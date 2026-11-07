//! Eigene Bedienleiste über dem Video: Fortschritt (mit Kapiteln), Play/Pause, Lautstärke,
//! Einstellungen (Qualität, Geschwindigkeit), Vollbild.
//! Bubble-Design: Timeline ohne Hintergrund, jede Knopfgruppe in einer eigenen Glas-Pille.
//! Erscheint nur bei Mausbewegung über dem Player und blendet sich nach kurzer Zeit wieder aus.

use crate::grid::fmt_duration;
use gtk4 as gtk;
use gtk::glib;
use gtk::prelude::*;
use libmpv2::Mpv;
use crate::aniskip::Segment;
use libmpv2::events::PropertyData;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

const HIDE_AFTER: Duration = Duration::from_millis(2000);
/// Beim Ziehen an der Timeline: so lange still halten, bevor gesprungen wird.
const SEEK_DELAY: Duration = Duration::from_millis(150);

/// Wählbare Qualitäten (maximale Bildhöhe). Gibt es die gewählte nicht, nimmt der Player die nächstkleinere.
const QUALITIES: [(u32, &str); 4] = [(1080, "1080p"), (720, "720p"), (480, "480p"), (360, "360p")];
const DEFAULT_QUALITY: u32 = 1080;
pub const SPEEDS: [f64; 8] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];

/// Datei in `~/.config/aniqu/` (Qualität, Lautstärke).
fn config_file(name: &str) -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    base.join("aniqu").join(name)
}

fn quality_file() -> PathBuf {
    config_file("quality")
}

/// Zuletzt eingestellte Lautstärke (0–100), bleibt über Neustarts erhalten.
pub fn saved_volume() -> f64 {
    std::fs::read_to_string(config_file("volume"))
        .ok()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .map_or(100.0, |v| v.clamp(0.0, 100.0))
}

fn save_volume(v: f64) {
    let f = config_file("volume");
    let _ = std::fs::create_dir_all(f.parent().unwrap());
    let _ = std::fs::write(f, format!("{v:.0}"));
}

/// Zuletzt gewählte Qualität (bleibt über Neustarts erhalten).
pub fn saved_quality() -> u32 {
    std::fs::read_to_string(quality_file())
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .filter(|q| QUALITIES.iter().any(|(h, _)| h == q))
        .unwrap_or(DEFAULT_QUALITY)
}

pub fn speed_label(s: f64) -> String {
    if s == 1.0 { "Normal".into() } else { format!("{s}×") }
}

/// Kapitel, in dem `pos` liegt (Kapitel sind nach Startzeit sortiert).
fn chapter_at(chapters: &[(f64, String)], pos: f64) -> Option<&str> {
    chapters.iter().rev().find(|(start, _)| *start <= pos).map(|(_, t)| t.as_str())
}

pub struct Controls {
    pub widget: gtk::Revealer,
    pub fullscreen: gtk::Button,
    mpv: &'static Mpv,
    play: gtk::Button,
    seek: gtk::Scale,
    time: gtk::Label,
    vol: gtk::Scale,
    mute: gtk::Button,
    settings: gtk::MenuButton,
    speed_buttons: Vec<(f64, gtk::CheckButton)>,
    /// Bekommt beim Ausblenden einen unsichtbaren Mauszeiger.
    cursor_target: gtk::Widget,
    hide_timer: RefCell<Option<glib::SourceId>>,
    chapters: Rc<RefCell<Vec<(f64, String)>>>,
    /// Opening/Ending (ani-skip), als dezente Bereiche über dem Fortschrittsbalken gezeichnet.
    segments: Rc<RefCell<Vec<Segment>>>,
    /// Bis hierhin (Sekunden) ist das Video vorgeladen; wird hell in der Timeline gezeigt.
    cached_until: Rc<Cell<f64>>,
    /// Wird mit der neuen Höhe aufgerufen, wenn die Qualität geändert wird (Player lädt neu).
    on_quality: Rc<RefCell<Option<Box<dyn Fn(u32)>>>>,
    segment_layer: gtk::DrawingArea,
    pos: Cell<f64>,
    dur: Cell<f64>,
    speed: Cell<f64>,
    live: Cell<bool>,
    paused: Cell<bool>,
}

fn icon_button(icon: &str, tip: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tip)
        .css_classes(["flat"])
        // Kein Fokus, sonst würde die Leertaste den zuletzt geklickten Button auslösen.
        .focusable(false)
        .build()
}

fn slider(max: f64) -> gtk::Scale {
    let s = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, max, 1.0);
    s.set_draw_value(false);
    s.set_focusable(false);
    s
}

/// Spalte im Einstellungsmenü: Überschrift und Auswahlknöpfe (nur einer aktiv).
fn radio_column(title: &str, labels: &[String]) -> (gtk::Box, Vec<gtk::CheckButton>) {
    let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
    col.append(&gtk::Label::builder().label(title).xalign(0.0).margin_bottom(4).css_classes(["heading"]).build());
    let mut buttons: Vec<gtk::CheckButton> = Vec::new();
    for l in labels {
        let b = gtk::CheckButton::builder().label(l).focusable(false).build();
        if let Some(first) = buttons.first() {
            b.set_group(Some(first));
        }
        col.append(&b);
        buttons.push(b);
    }
    (col, buttons)
}

impl Controls {
    pub fn new(mpv: &'static Mpv, cursor_target: &impl IsA<gtk::Widget>) -> Rc<Self> {
        let play = icon_button("aniqu-pause-symbolic", "Pause (space)");
        play.connect_clicked(move |_| {
            let _ = mpv.command("cycle", &["pause"]);
        });

        let seek = slider(1.0);
        seek.set_hexpand(true);

        let mute = icon_button("aniqu-volume-symbolic", "Mute (m)");
        mute.connect_clicked(move |_| {
            let _ = mpv.command("cycle", &["mute"]);
        });
        let vol = slider(100.0);
        vol.set_width_request(90);
        vol.set_margin_end(8);
        vol.connect_change_value(move |_, _, v| {
            let _ = mpv.set_property("volume", v.clamp(0.0, 100.0));
            glib::Propagation::Proceed
        });

        let time = gtk::Label::builder()
            .css_classes(["numeric"])
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        let fullscreen = icon_button("aniqu-fullscreen-symbolic", "Full screen (f)");

        // Timeline: change-value kommt nur von der Maus, nicht von set_value (keine Rückkopplung).
        // Beim Ziehen nicht bei jeder Bewegung springen (jeder Sprung lädt neu), sondern erst,
        // wenn die Maus kurz still steht oder losgelassen wird. Die Zeit folgt sofort.
        let pending_seek: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
        seek.connect_change_value({
            let time = time.downgrade();
            move |s, _, v| {
                if let Some(t) = time.upgrade() {
                    t.set_text(&format!("{} / {}", fmt_duration(v.max(0.0) as u32), fmt_duration(s.adjustment().upper() as u32)));
                }
                if let Some(id) = pending_seek.take() {
                    id.remove();
                }
                let pending = pending_seek.clone();
                let id = glib::timeout_add_local_once(SEEK_DELAY, move || {
                    pending.take();
                    let _ = mpv.command("seek", &[&v.to_string(), "absolute"]);
                });
                pending_seek.replace(Some(id));
                glib::Propagation::Proceed
            }
        });

        // Einstellungen: Qualität und Geschwindigkeit nebeneinander.
        let (quality_col, quality_buttons) =
            radio_column("Quality", &QUALITIES.iter().map(|(_, l)| l.to_string()).collect::<Vec<_>>());
        let (speed_col, speed_buttons) = radio_column("Playback speed", &SPEEDS.map(speed_label));
        let menu = gtk::Box::builder().spacing(24).margin_top(8).margin_bottom(8).margin_start(8).margin_end(8).build();
        menu.append(&quality_col);
        menu.append(&speed_col);
        let settings = gtk::MenuButton::builder()
            .icon_name("aniqu-settings-symbolic")
            .tooltip_text("Settings")
            .direction(gtk::ArrowType::Up)
            .popover(&gtk::Popover::builder().child(&menu).build())
            .focusable(false)
            .css_classes(["flat"])
            .build();

        let saved = saved_quality();
        let on_quality: Rc<RefCell<Option<Box<dyn Fn(u32)>>>> = Rc::default();
        for ((height, _), b) in QUALITIES.iter().zip(&quality_buttons) {
            b.set_active(*height == saved);
            let (height, on_quality) = (*height, on_quality.clone());
            b.connect_toggled(move |b| {
                if !b.is_active() {
                    return;
                }
                let _ = std::fs::create_dir_all(quality_file().parent().unwrap());
                let _ = std::fs::write(quality_file(), height.to_string());
                // Neue Qualität = andere Stufe des Streams: der Player lädt sie an derselben Stelle.
                if let Some(f) = on_quality.borrow().as_ref() {
                    f(height);
                }
            });
        }
        let speed_buttons: Vec<(f64, gtk::CheckButton)> = SPEEDS.into_iter().zip(speed_buttons).collect();
        for (speed, b) in &speed_buttons {
            b.set_active(*speed == 1.0);
            let speed = *speed;
            b.connect_toggled(move |b| {
                if b.is_active() {
                    let _ = mpv.set_property("speed", speed);
                }
            });
        }

        // Jede Gruppe in einer eigenen Bubble.
        let bubble = |widgets: &[&gtk::Widget]| {
            let b = gtk::Box::builder().spacing(2).valign(gtk::Align::Center).css_classes(["bubble"]).build();
            for w in widgets {
                b.append(*w);
            }
            b
        };
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.append(&bubble(&[play.upcast_ref()]));
        buttons.append(&bubble(&[mute.upcast_ref(), vol.upcast_ref()]));
        buttons.append(&bubble(&[time.upcast_ref()]));
        buttons.append(&gtk::Box::builder().hexpand(true).build());
        // Untertitel durchschalten (wie Taste c); die Sprache blendet der Player ein.
        let subtitles = icon_button("aniqu-subtitles-symbolic", "Subtitles (c)");
        subtitles.connect_clicked(move |_| {
            let _ = mpv.command("cycle", &["sub"]);
        });
        buttons.append(&bubble(&[subtitles.upcast_ref(), settings.upcast_ref(), fullscreen.upcast_ref()]));

        // Vorschau über der Timeline (Zeit, ggf. Kapitel), folgt der Maus. Liegt in der Leiste über dem Balken;
        // unsichtbar nimmt sie keinen Platz ein, sichtbar wächst die (unten verankerte) Leiste nach oben.
        let preview_time = gtk::Label::builder().css_classes(["osd", "pill", "numeric"]).halign(gtk::Align::Center).max_width_chars(40).ellipsize(gtk::pango::EllipsizeMode::End).build();
        let preview = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .halign(gtk::Align::Start)
            .can_target(false)
            .visible(false)
            .build();
        preview.append(&preview_time);
        let chapters: Rc<RefCell<Vec<(f64, String)>>> = Rc::default();

        let bar = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .margin_start(16)
            .margin_end(16)
            .margin_bottom(14)
            .css_classes(["player-controls"])
            .build();
        seek.add_css_class("timeline");

        // Opening/Ending-Bereiche: eigene Zeichenfläche über dem Balken, genau über dessen Schiene.
        let segments: Rc<RefCell<Vec<Segment>>> = Rc::default();
        let cached_until: Rc<Cell<f64>> = Rc::default();
        let segment_layer = gtk::DrawingArea::builder().can_target(false).build();
        segment_layer.set_draw_func({
            let (segments, cached_until, seek) = (segments.clone(), cached_until.clone(), seek.downgrade());
            move |area, cr, _, _| {
                let Some(seek) = seek.upgrade() else { return };
                let segs = segments.borrow();
                let dur = seek.adjustment().upper();
                // Die Schiene ("trough") ist ein internes Kind des Reglers; ihre Lage bestimmt, wo gezeichnet wird.
                let mut child = seek.first_child();
                while let Some(c) = child.as_ref().filter(|c| c.css_name() != "trough") {
                    child = c.next_sibling();
                }
                let (Some(trough), true) = (child, dur > 1.0) else { return };
                let Some(r) = trough.compute_bounds(area) else { return };
                let (h, y) = (r.height() as f64, r.y() as f64);
                let x_at = |t: f64| r.x() as f64 + r.width() as f64 * (t / dur).clamp(0.0, 1.0);
                // Vorgeladener Bereich (aktuelle Stelle bis Pufferende) heller, wie bei YouTube:
                // dorthin springen geht ohne Warten.
                let (pos, until) = (seek.value(), cached_until.get());
                if until > pos {
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.3);
                    cr.rectangle(x_at(pos), y, x_at(until) - x_at(pos), h);
                    let _ = cr.fill();
                }
                // Dezent in der Schriftfarbe des Themes, keine Signalfarben.
                let c = seek.color();
                for s in segs.iter() {
                    let x0 = r.x() as f64 + r.width() as f64 * (s.start / dur).clamp(0.0, 1.0);
                    let x1 = r.x() as f64 + r.width() as f64 * (s.end / dur).clamp(0.0, 1.0);
                    cr.set_source_rgba(c.red().into(), c.green().into(), c.blue().into(), 0.45);
                    cr.rectangle(x0, y, (x1 - x0).max(2.0), h);
                    let _ = cr.fill();
                }
            }
        });
        let seek_row = gtk::Overlay::builder().child(&seek).build();
        seek_row.add_overlay(&segment_layer);
        bar.append(&preview);
        bar.append(&seek_row);

        let hover = gtk::EventControllerMotion::new();
        hover.connect_motion({
            let (preview, label, seek, chapters) = (preview.clone(), preview_time.clone(), seek.downgrade(), chapters.clone());
            move |_, x, _| {
                let Some(seek) = seek.upgrade() else { return };
                let dur = seek.adjustment().upper();
                // Zeit unter der Maus: Lage der Schiene ("trough") im Regler.
                let mut child = seek.first_child();
                while let Some(c) = child.as_ref().filter(|c| c.css_name() != "trough") {
                    child = c.next_sibling();
                }
                let (Some(trough), true) = (child, dur > 1.0) else { return };
                let Some(r) = trough.compute_bounds(&seek) else { return };
                let t = ((x - r.x() as f64) / r.width() as f64).clamp(0.0, 1.0) * dur;
                // Zeit und – falls vorhanden – Kapitel an dieser Stelle.
                match chapter_at(&chapters.borrow(), t).filter(|c| !c.is_empty()) {
                    Some(c) => label.set_text(&format!("{} · {c}", fmt_duration(t as u32))),
                    None => label.set_text(&fmt_duration(t as u32)),
                }
                // Mittig über der Maus, aber nicht über den Rand hinaus.
                let w = preview.width().max(60) as f64;
                let max = (seek.width() as f64 - w).max(0.0);
                preview.set_margin_start((x - w / 2.0).clamp(0.0, max) as i32);
                preview.set_visible(true);
            }
        });
        hover.connect_leave({
            let preview = preview.clone();
            move |_| preview.set_visible(false)
        });
        seek.add_controller(hover);
        bar.append(&buttons);

        let widget = gtk::Revealer::builder()
            .child(&bar)
            .transition_type(gtk::RevealerTransitionType::Crossfade)
            .reveal_child(true)
            .valign(gtk::Align::End)
            .build();

        Rc::new(Controls {
            widget,
            fullscreen,
            mpv,
            play,
            seek,
            time,
            vol,
            mute,
            settings,
            speed_buttons,
            cursor_target: cursor_target.clone().upcast(),
            hide_timer: RefCell::default(),
            chapters,
            segments,
            segment_layer,
            cached_until,
            on_quality,
            pos: Cell::new(0.0),
            dur: Cell::new(0.0),
            speed: Cell::new(1.0),
            live: Cell::new(false),
            paused: Cell::new(false),
        })
    }

    /// Leiste zeigen und das Ausblenden neu starten (bei Mausbewegung, Taste, Pause …).
    pub fn poke(self: &Rc<Self>) {
        self.widget.set_reveal_child(true);
        self.cursor_target.set_cursor(None);
        self.cursor_target.remove_css_class("idle");
        if let Some(t) = self.hide_timer.take() {
            t.remove();
        }
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(HIDE_AFTER, move || {
            let Some(c) = weak.upgrade() else { return };
            c.hide_timer.take(); // Timer ist abgelaufen, nicht mehr entfernen.
            // Maus ruht: Hover-Hervorhebung aus (style.css `.idle`), auch wenn die Leiste sichtbar bleibt.
            c.cursor_target.add_css_class("idle");
            // Pausiert oder bei offenem Einstellungsmenü sichtbar bleiben.
            if !c.paused.get() && !c.settings.is_active() {
                c.widget.set_reveal_child(false);
                c.cursor_target.set_cursor_from_name(Some("none"));
            }
        });
        self.hide_timer.replace(Some(id));
    }

    /// Sofort ausblenden (Maus hat den Player verlassen). Bei offenem Einstellungsmenü nicht.
    pub fn hide(&self) {
        if self.settings.is_active() {
            return;
        }
        if let Some(t) = self.hide_timer.take() {
            t.remove();
        }
        self.widget.set_reveal_child(false);
    }

    /// Neues Video: Anzeige zurücksetzen. Bei Livestreams gibt es keinen Fortschrittsbalken.
    pub fn reset(self: &Rc<Self>, live: bool) {
        self.live.set(live);
        self.pos.set(0.0);
        self.dur.set(0.0);
        self.chapters.borrow_mut().clear();
        self.cached_until.set(0.0);
        self.set_segments(vec![]);
        self.seek.clear_marks();
        self.seek.set_visible(!live);
        self.seek.set_value(0.0);
        self.update_time();
        self.poke();
    }

    pub fn connect_quality(&self, f: impl Fn(u32) + 'static) {
        self.on_quality.replace(Some(Box::new(f)));
    }

    pub fn set_segments(&self, segments: Vec<Segment>) {
        self.segments.replace(segments);
        self.segment_layer.queue_draw();
    }

    /// Kapitel aus mpv übernehmen (falls der Stream welche hat) und im Balken markieren.
    pub fn load_chapters(&self) {
        let count = self.mpv.get_property::<i64>("chapter-list/count").unwrap_or(0);
        let chapters: Vec<(f64, String)> = (0..count)
            .filter_map(|i| {
                let start = self.mpv.get_property::<f64>(&format!("chapter-list/{i}/time")).ok()?;
                let title = self.mpv.get_property::<String>(&format!("chapter-list/{i}/title")).unwrap_or_default();
                Some((start, title))
            })
            .collect();
        self.seek.clear_marks();
        for (start, _) in chapters.iter().filter(|(s, _)| *s > 0.0) {
            self.seek.add_mark(*start, gtk::PositionType::Bottom, None);
        }
        self.chapters.replace(chapters);
        self.update_time();
    }

    /// Geschwindigkeit eine Stufe schneller (+1) oder langsamer (-1), wie `>` / `<` bei YouTube.
    pub fn step_speed(&self, dir: i32) {
        let i = SPEEDS.iter().position(|s| *s == self.speed.get()).unwrap_or(3) as i32;
        let next = SPEEDS[(i + dir).clamp(0, SPEEDS.len() as i32 - 1) as usize];
        let _ = self.mpv.set_property("speed", next);
    }

    /// Wird für jede beobachtete mpv-Eigenschaft aufgerufen, wenn sie sich ändert.
    pub fn update(self: &Rc<Self>, name: &str, data: PropertyData) {
        match (name, data) {
            ("time-pos", PropertyData::Double(t)) => {
                self.pos.set(t);
                // Während der Nutzer den Regler zieht, nicht dazwischenfunken.
                if !self.seek.state_flags().contains(gtk::StateFlags::ACTIVE) {
                    self.seek.set_value(t);
                }
                self.update_time();
            }
            ("demuxer-cache-time", PropertyData::Double(t)) => {
                self.cached_until.set(t);
                self.segment_layer.queue_draw();
            }
            ("duration", PropertyData::Double(d)) => {
                self.dur.set(d);
                self.seek.set_range(0.0, d.max(1.0));
                self.segment_layer.queue_draw();
                self.update_time();
            }
            ("pause", PropertyData::Flag(p)) => {
                self.paused.set(p);
                let (icon, tip) = if p {
                    ("aniqu-play-symbolic", "Play (space)")
                } else {
                    ("aniqu-pause-symbolic", "Pause (space)")
                };
                self.play.set_icon_name(icon);
                self.play.set_tooltip_text(Some(tip));
                self.poke();
            }
            ("speed", PropertyData::Double(s)) => {
                self.speed.set(s);
                if let Some((_, b)) = self.speed_buttons.iter().find(|(v, _)| *v == s) {
                    b.set_active(true);
                }
                self.update_time();
                self.poke();
            }
            // Jede Änderung (Regler, Pfeiltasten) merken.
            ("volume", PropertyData::Double(v)) => {
                self.vol.set_value(v);
                save_volume(v);
            }
            ("mute", PropertyData::Flag(m)) => self.mute.set_icon_name(if m { "aniqu-mute-symbolic" } else { "aniqu-volume-symbolic" }),
            _ => {}
        }
    }

    fn update_time(&self) {
        // Während an der Timeline gezogen wird, zeigt die Zeit das Ziel (siehe change-value).
        if self.seek.state_flags().contains(gtk::StateFlags::ACTIVE) {
            return;
        }
        let mut parts = vec![if self.live.get() {
            "● LIVE".to_string()
        } else {
            format!("{} / {}", fmt_duration(self.pos.get() as u32), fmt_duration(self.dur.get() as u32))
        }];
        if let Some(c) = chapter_at(&self.chapters.borrow(), self.pos.get()).filter(|c| !c.is_empty()) {
            parts.push(c.to_string());
        }
        if self.speed.get() != 1.0 {
            parts.push(speed_label(self.speed.get()));
        }
        let t = parts.join(" · ");
        if self.time.text() != t {
            self.time.set_text(&t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapters_speed_quality() {
        let ch = vec![(0.0, "Intro".to_string()), (60.0, "Teil 1".to_string()), (300.0, "Ende".to_string())];
        assert_eq!(chapter_at(&ch, 10.0), Some("Intro"));
        assert_eq!(chapter_at(&ch, 60.0), Some("Teil 1"));
        assert_eq!(chapter_at(&ch, 999.0), Some("Ende"));
        assert_eq!(chapter_at(&[], 5.0), None);
        assert_eq!(speed_label(1.5), "1.5×");
        assert_eq!(speed_label(1.0), "Normal");
    }
}
