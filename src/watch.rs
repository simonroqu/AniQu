//! Ansehen-Seite: Video, darunter Anime-Titel (öffnet die Anime-Seite), Episode und
//! Knöpfe für vorherige/nächste Episode. Der Player ist genau so hoch wie das Video.

use crate::show::Play;
use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{gdk, glib};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Player-Höhe: volle Breite im Seitenverhältnis, höchstens so hoch wie die Seite.
/// Im Vollbild immer die ganze Höhe.
fn player_height(width: i32, page_height: i32, aspect: f64, fullscreen: bool) -> i32 {
    if fullscreen || aspect <= 0.0 {
        return page_height;
    }
    ((width as f64 / aspect).round() as i32).min(page_height)
}

/// Rand um den Player (Hintergrundfarbe), im Vollbild keiner.
const PLAYER_MARGIN: i32 = 5;
/// Anteil der Fensterbreite, den der Player höchstens einnimmt; links/rechts bleibt Hintergrund.
const PLAYER_WIDTH: f64 = 0.85;
/// Unter dieser Fensterbreite nimmt der Player die volle Breite ein.
const FULL_WIDTH_BELOW: i32 = 1600;
/// Miniplayer unten rechts (Seite verlassen, Video läuft weiter): Breite und Abstand zum Fensterrand.
const MINI_WIDTH: i32 = 400;
const MINI_MARGIN: i32 = 16;

/// Seitenrand des Players: so viel, dass er mittig `PLAYER_WIDTH` der Breite einnimmt, mindestens
/// `PLAYER_MARGIN`. Im Vollbild kein Rand.
fn player_side(width: i32, fullscreen: bool) -> i32 {
    if fullscreen {
        return 0;
    }
    // Schmales Fenster: volle Breite (nur der kleine Rand).
    if width < FULL_WIDTH_BELOW {
        return PLAYER_MARGIN;
    }
    PLAYER_MARGIN.max((width as f64 * (1.0 - PLAYER_WIDTH) / 2.0).round() as i32)
}

pub struct WatchPage {
    pub widget: gtk::Overlay,
    /// Alles unter dem Video; wird im Vollbild ausgeblendet.
    pub info: gtk::Box,
    scroll: gtk::ScrolledWindow,
    player: gtk::Widget,
    title: gtk::Label,
    episode: gtk::Label,
    pub prev: gtk::Button,
    pub next: gtk::Button,
    /// Seitengröße, Seitenverhältnis des Videos und Vollbild: daraus folgt die Player-Höhe.
    size: Cell<(i32, i32)>,
    aspect: Cell<f64>,
    fullscreen: Cell<bool>,
    /// Miniplayer: nur das Video, klein unten rechts über der aktuellen Seite.
    mini: Cell<bool>,
    /// Was gerade läuft (für Zurück zur Anime-Seite, vorherige/nächste Episode).
    pub current: RefCell<Option<Play>>,
}

impl WatchPage {
    pub fn new(player: &impl IsA<gtk::Widget>, on_anime: impl Fn() + 'static) -> Rc<Self> {
        let title = gtk::Label::builder().xalign(0.0).wrap(true).css_classes(["title-3"]).build();
        let title_button = gtk::Button::builder()
            .child(&title)
            .css_classes(["flat", "channel-link"])
            .halign(gtk::Align::Start)
            .focusable(false)
            .tooltip_text("All episodes")
            .cursor(&gdk::Cursor::from_name("pointer", None).unwrap())
            .build();
        title_button.connect_clicked(move |_| on_anime());
        let episode = gtk::Label::builder().xalign(0.0).wrap(true).selectable(true).focusable(false).css_classes(["dim-label"]).build();
        let text_col = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(4).hexpand(true).build();
        text_col.append(&title_button);
        text_col.append(&episode);

        let nav_button = |icon: &str, tip: &str| {
            gtk::Button::builder().icon_name(icon).tooltip_text(tip).focusable(false).valign(gtk::Align::Center).css_classes(["flat"]).build()
        };
        let prev = nav_button("aniqu-back-symbolic", "Previous episode");
        let next = nav_button("aniqu-next-symbolic", "Next episode (n)");
        let row = gtk::Box::builder().spacing(8).build();
        row.append(&text_col);
        row.append(&prev);
        row.append(&next);

        let info = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(12)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();
        info.append(&row);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(player);
        content.append(&info);
        let scroll = gtk::ScrolledWindow::builder().child(&content).hscrollbar_policy(gtk::PolicyType::Never).build();
        crate::smooth::attach(&scroll);

        // Unsichtbare Messfläche über der ganzen Seite: liefert Breite und Höhe für die Player-Höhe.
        let sizer = gtk::DrawingArea::builder().can_target(false).build();
        // Liegt als Ebene über den Seiten (main.rs), daher eigener Hintergrund (außer als Miniplayer).
        let widget = gtk::Overlay::builder().child(&scroll).css_classes(["background"]).visible(false).build();
        widget.add_overlay(&sizer);

        let page = Rc::new(WatchPage {
            widget,
            info,
            scroll,
            player: player.clone().upcast(),
            title,
            episode,
            prev,
            next,
            size: Cell::new((0, 0)),
            aspect: Cell::new(16.0 / 9.0),
            fullscreen: Cell::new(false),
            mini: Cell::new(false),
            current: RefCell::default(),
        });

        sizer.connect_resize({
            let page = Rc::downgrade(&page);
            move |_, w, h| {
                if let Some(p) = page.upgrade() {
                    p.size.set((w, h));
                    // Nicht mitten in der Größenberechnung umbauen, sondern direkt danach.
                    glib::idle_add_local_once(move || p.fit_player());
                }
            }
        });
        page
    }

    fn fit_player(&self) {
        if self.mini.get() {
            let a = if self.aspect.get() > 0.0 { self.aspect.get() } else { 16.0 / 9.0 };
            let h = (MINI_WIDTH as f64 / a).round() as i32;
            self.player.set_margin_top(0);
            self.player.set_margin_start(0);
            self.player.set_margin_bottom(0);
            self.player.set_margin_end(0);
            self.player.add_css_class("rounded");
            self.player.set_size_request(-1, h);
            self.widget.set_size_request(MINI_WIDTH, h);
            return;
        }
        self.widget.set_size_request(-1, -1);
        let (w, h) = self.size.get();
        let fs = self.fullscreen.get();
        let m = if fs { 0 } else { PLAYER_MARGIN };
        let side = player_side(w, fs);
        self.player.set_margin_top(m);
        self.player.set_margin_bottom(m);
        self.player.set_margin_start(side);
        self.player.set_margin_end(side);
        // Text darunter bündig zum Player, 16 px eingerückt (bzw. 11 px mehr als der Player).
        self.info.set_margin_start(side - PLAYER_MARGIN + 16);
        self.info.set_margin_end(side - PLAYER_MARGIN + 16);
        // Abgerundete Ecken wie die Fensterecken (`--radius-window`), im Vollbild eckig.
        if fs { self.player.remove_css_class("rounded") } else { self.player.add_css_class("rounded") }
        if w > 0 && h > 0 {
            self.player.set_size_request(-1, player_height(w - 2 * side, h - 2 * m, self.aspect.get(), fs));
        }
    }

    /// Seitenverhältnis des laufenden Videos (vom Player gemeldet).
    pub fn set_aspect(&self, aspect: f64) {
        self.aspect.set(aspect);
        self.fit_player();
    }

    pub fn is_mini(&self) -> bool {
        self.mini.get()
    }

    /// Miniplayer an/aus: dieselbe Seite samt Player, nur klein unten rechts und ohne Infos
    /// (der Player wird nicht umgehängt, sonst müsste mpv seine Video-Fläche neu aufbauen).
    pub fn set_mini(&self, on: bool) {
        self.mini.set(on);
        let align = if on { gtk::Align::End } else { gtk::Align::Fill };
        self.widget.set_halign(align);
        self.widget.set_valign(align);
        let m = if on { MINI_MARGIN } else { 0 };
        self.widget.set_margin_end(m);
        self.widget.set_margin_bottom(m);
        if on {
            self.widget.remove_css_class("background");
            self.widget.add_css_class("mini");
            self.scroll.vadjustment().set_value(0.0);
        } else {
            self.widget.add_css_class("background");
            self.widget.remove_css_class("mini");
        }
        let v = if on { gtk::PolicyType::Never } else { gtk::PolicyType::Automatic };
        self.scroll.set_policy(gtk::PolicyType::Never, v);
        self.info.set_visible(!on && !self.fullscreen.get());
        self.fit_player();
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        self.fullscreen.set(fullscreen);
        self.info.set_visible(!fullscreen && !self.mini.get());
        self.scroll.vadjustment().set_value(0.0);
        self.fit_player();
    }

    pub fn show(&self, p: &Play) {
        let ep = p.episode();
        self.title.set_text(&p.anime.title);
        let lang = if p.dub { "Dub" } else { "Sub" };
        let name = if ep.title.is_empty() || ep.title == format!("Episode {}", ep.number) { String::new() } else { format!(" · {}", ep.title) };
        self.episode.set_text(&format!("Episode {} of {}{name} · {lang}", ep.number, p.episodes.len()));
        self.prev.set_sensitive(p.prev().is_some());
        self.next.set_sensitive(p.next().is_some());
        self.current.replace(Some(p.clone()));
        // Bis das neue Video sein Format meldet: 16:9 annehmen.
        self.aspect.set(16.0 / 9.0);
        self.fit_player();
        self.scroll.vadjustment().set_value(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_fits_video() {
        // 16:9 bei 1280 Breite: 720 hoch, Seite ist höher -> passt.
        assert_eq!(player_height(1280, 1400, 16.0 / 9.0, false), 720);
        // Breites Fenster, niedrige Seite: durch die Seitenhöhe begrenzt.
        assert_eq!(player_height(1920, 900, 16.0 / 9.0, false), 900);
        // Hochkant-Video (9:16): wäre riesig, also Seitenhöhe.
        assert_eq!(player_height(1280, 1400, 9.0 / 16.0, false), 1400);
        assert_eq!(player_height(1280, 1400, 16.0 / 9.0, true), 1400);
        // Seitenrand: unter 1600 px volle Breite (5 px), sonst 85 % mittig, Vollbild = 0.
        assert_eq!(player_side(1343, false), 5);
        assert_eq!(player_side(2560, false), 192);
        assert_eq!(player_side(2560, true), 0);
    }
}
