//! Anime-Seite: Poster, Titel, Infos, Beschreibung, Umschalter Sub/Dub und alle Episoden.

use crate::anime::{self, Anime, Details, Episode};
use crate::bg;
use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{gdk, glib};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// So viele Zeilen Beschreibung sind eingeklappt sichtbar.
const DESCRIPTION_LINES: i32 = 4;
const SHOW_MORE: &str = "Show more";
const SHOW_LESS: &str = "Show less";

/// Eine Episode abspielen: mit der ganzen Liste, damit „nächste Episode“ ohne neue Anfrage geht.
#[derive(Clone)]
pub struct Play {
    pub anime: Anime,
    pub episodes: Rc<Vec<Episode>>,
    pub index: usize,
    pub dub: bool,
}

impl Play {
    pub fn episode(&self) -> &Episode {
        &self.episodes[self.index]
    }

    /// Die nächste Episode (falls es eine gibt und sie in der gewählten Sprache verfügbar ist).
    pub fn next(&self) -> Option<Play> {
        let next = Play { index: self.index + 1, ..self.clone() };
        (next.index < self.episodes.len() && available(&next.anime, next.episode().number, next.dub)).then_some(next)
    }

    pub fn prev(&self) -> Option<Play> {
        (self.index > 0).then(|| Play { index: self.index - 1, ..self.clone() })
    }
}

/// Gibt es die Episode synchronisiert? hianime zählt nur, wie viele (die ersten n).
fn available(a: &Anime, number: u32, dub: bool) -> bool {
    !dub || a.dub.is_none_or(|n| number <= n)
}

fn label(classes: &[&str]) -> gtk::Label {
    gtk::Label::builder().xalign(0.0).wrap(true).selectable(true).focusable(false).css_classes(classes.to_vec()).build()
}

pub struct ShowPage {
    pub widget: gtk::ScrolledWindow,
    poster: gtk::Picture,
    title: gtk::Label,
    meta: gtk::Label,
    description: gtk::Label,
    more: gtk::Button,
    sub_tab: gtk::ToggleButton,
    dub_tab: gtk::ToggleButton,
    /// Auf die Watchlist setzen/entfernen (Lesezeichen: Kontur = nicht drauf, gefüllt = drauf).
    watchlist: gtk::Button,
    flow: gtk::FlowBox,
    /// Lade-Platzhalter (style.css `.skeleton`): Infos + Beschreibung, Episoden-Kacheln, Poster-Fläche.
    ghost_text: gtk::Box,
    ghost_flow: gtk::FlowBox,
    poster_box: gtk::Box,
    status: gtk::Label,
    anime: RefCell<Anime>,
    episodes: RefCell<Rc<Vec<Episode>>>,
    dub: Cell<bool>,
    generation: Cell<u64>,
    on_play: Rc<dyn Fn(Play)>,
}

impl ShowPage {
    pub fn new(on_play: impl Fn(Play) + 'static) -> Rc<Self> {
        let poster = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .width_request(200)
            .height_request(300)
            .valign(gtk::Align::Start)
            .css_classes(["thumb"])
            .build();
        let title = label(&["title-1"]);
        let meta = label(&["dim-label"]);
        let description = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .lines(DESCRIPTION_LINES)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(90)
            .selectable(true)
            .focusable(false)
            .build();
        let more = gtk::Button::builder().label(SHOW_MORE).halign(gtk::Align::Start).focusable(false).visible(false).css_classes(["flat"]).build();
        let sub_tab = gtk::ToggleButton::builder().label("Sub").active(true).focusable(false).build();
        let dub_tab = gtk::ToggleButton::builder().label("Dub").group(&sub_tab).focusable(false).build();
        let tabs = gtk::Box::builder().spacing(6).margin_top(8).css_classes(["tabs"]).build();
        tabs.append(&sub_tab);
        tabs.append(&dub_tab);
        let watchlist = gtk::Button::builder().icon_name("qu-watchlist-symbolic").focusable(false).valign(gtk::Align::Center).css_classes(["flat"]).build();
        tabs.append(&watchlist);

        // Platzhalter für Infos und Beschreibung, bis die Anime-Seite gelesen ist (Titel ist schon bekannt).
        let ghost_text = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(8).visible(false).build();
        for (k, n, h) in [(2, 5, 14), (1, 4, 14)] {
            ghost_text.append(&qu_core::cards::part(k, n, h));
        }
        let desc = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(8).margin_top(8).build();
        for (k, n) in [(1, 1), (1, 1), (3, 5)] {
            desc.append(&qu_core::cards::part(k, n, 14));
        }
        ghost_text.append(&desc);

        let text_col = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(8).hexpand(true).build();
        for w in [title.upcast_ref::<gtk::Widget>(), ghost_text.upcast_ref(), meta.upcast_ref(), description.upcast_ref(), more.upcast_ref(), tabs.upcast_ref()] {
            text_col.append(w);
        }
        // Graue Fläche hinter dem Poster, bis es geladen ist; glänzt, solange die Seite lädt.
        let poster_box = gtk::Box::builder().valign(gtk::Align::Start).overflow(gtk::Overflow::Hidden).css_classes(["thumb-box"]).build();
        poster_box.append(&poster);
        let header = gtk::Box::builder().spacing(24).build();
        header.append(&poster_box);
        header.append(&text_col);

        let flow = gtk::FlowBox::builder()
            .homogeneous(true)
            .selection_mode(gtk::SelectionMode::None)
            .activate_on_single_click(true)
            .max_children_per_line(12)
            .row_spacing(8)
            .column_spacing(8)
            .valign(gtk::Align::Start)
            .build();
        // Platzhalter-Kacheln in Form der Episoden (gleiche Breite wie `fill_episodes`).
        let ghost_flow = gtk::FlowBox::builder()
            .homogeneous(true)
            .selection_mode(gtk::SelectionMode::None)
            .max_children_per_line(12)
            .row_spacing(8)
            .column_spacing(8)
            .valign(gtk::Align::Start)
            .can_target(false)
            .visible(false)
            .build();
        for _ in 0..12 {
            let tile = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(8).width_request(200).css_classes(["episode"]).build();
            tile.append(&qu_core::cards::part(1, 2, 16));
            tile.append(&qu_core::cards::part(3, 4, 12));
            ghost_flow.append(&tile);
        }
        let status = gtk::Label::builder().visible(false).margin_top(32).wrap(true).build();

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            // Platz für die schwebende Navigation oben links.
            .margin_top(68)
            .margin_start(16)
            .margin_end(16)
            .margin_bottom(32)
            .build();
        content.append(&header);
        content.append(&ghost_flow);
        content.append(&status);
        content.append(&flow);
        let widget = gtk::ScrolledWindow::builder().child(&content).hscrollbar_policy(gtk::PolicyType::Never).build();
        qu_core::smooth::attach(&widget);

        let page = Rc::new(ShowPage {
            widget,
            poster,
            title,
            meta,
            description,
            more,
            sub_tab,
            dub_tab,
            watchlist,
            flow,
            ghost_text,
            ghost_flow,
            poster_box,
            status,
            anime: RefCell::default(),
            episodes: RefCell::default(),
            dub: Cell::new(false),
            generation: Cell::new(0),
            on_play: Rc::new(on_play),
        });

        page.more.connect_clicked({
            let page = Rc::downgrade(&page);
            move |b| {
                let Some(p) = page.upgrade() else { return };
                let expand = b.label().as_deref() == Some(SHOW_MORE);
                p.description.set_lines(if expand { -1 } else { DESCRIPTION_LINES });
                p.description.set_ellipsize(if expand { gtk::pango::EllipsizeMode::None } else { gtk::pango::EllipsizeMode::End });
                b.set_label(if expand { SHOW_LESS } else { SHOW_MORE });
            }
        });
        page.watchlist.connect_clicked({
            let page = Rc::downgrade(&page);
            move |_| {
                if let Some(p) = page.upgrade() {
                    let on = crate::lists::toggle_watchlist(&p.anime.borrow());
                    p.show_watchlist(on);
                }
            }
        });
        // `clicked` kommt nur vom Nutzer, nicht von set_active in `open`.
        for (tab, dub) in [(&page.sub_tab, false), (&page.dub_tab, true)] {
            let page = Rc::downgrade(&page);
            tab.connect_clicked(move |t| {
                if let (true, Some(p)) = (t.is_active(), page.upgrade()) {
                    p.dub.set(dub);
                    crate::lists::set_dub(&p.anime.borrow().id, dub);
                    p.fill_episodes();
                }
            });
        }
        page.flow.connect_child_activated({
            let page = Rc::downgrade(&page);
            move |_, child| {
                let Some(p) = page.upgrade() else { return };
                let play = Play {
                    anime: p.anime.borrow().clone(),
                    episodes: p.episodes.borrow().clone(),
                    index: child.index() as usize,
                    dub: p.dub.get(),
                };
                (p.on_play)(play);
            }
        });
        // Zurück von einer Episode: Fortschritt auffrischen.
        page.widget.connect_map({
            let page = Rc::downgrade(&page);
            move |_| {
                if let Some(p) = page.upgrade() {
                    p.fill_episodes();
                }
            }
        });
        page
    }

    /// Anime anzeigen: Bekanntes sofort, Details und Episoden im Hintergrund.
    pub fn open(self: &Rc<Self>, a: Anime) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        let same = self.anime.borrow().id == a.id;
        self.dub.set(crate::lists::dub(&a.id));
        self.sub_tab.set_active(!self.dub.get());
        self.dub_tab.set_active(self.dub.get());
        if !same {
            self.widget.vadjustment().set_value(0.0);
            self.episodes.replace(Rc::default());
            self.poster.set_paintable(None::<&gdk::Paintable>);
            self.description.set_text("");
            self.more.set_visible(false);
            self.set_anime(&a);
            self.fill_episodes();
            self.set_loading(true);
        }
        self.status.set_visible(false);
        self.show_watchlist(crate::lists::on_watchlist(&a.id));
        self.anime.replace(a.clone());

        let page = self.clone();
        let busy = crate::Busy::new();
        glib::spawn_future_local(async move {
            let id = a.id.clone();
            // Beide Anfragen gleichzeitig.
            let (details, episodes) = bg(async move {
                let d = tokio::spawn(anime::details(id.clone()));
                let e = anime::episodes(id).await;
                (d.await.unwrap_or_else(|e| Err(e.to_string())), e)
            })
            .await;
            drop(busy);
            if page.generation.get() != generation {
                return;
            }
            page.set_loading(false);
            match details {
                Ok(d) => page.set_details(d),
                Err(e) => eprintln!("Anime-Seite nicht gelesen: {e}"),
            }
            match episodes {
                Ok(list) if list.is_empty() => page.message("No episodes yet"),
                Ok(list) => {
                    page.episodes.replace(Rc::new(list));
                    page.fill_episodes();
                }
                Err(e) if page.episodes.borrow().is_empty() => page.message(&format!("Error: {e}")),
                Err(e) => eprintln!("Episoden nicht neu geladen: {e}"),
            }
        });
    }

    /// Platzhalter an/aus. Die Episoden-Platzhalter nur, solange noch keine Episoden da sind.
    fn set_loading(&self, on: bool) {
        self.ghost_text.set_visible(on);
        self.ghost_flow.set_visible(on && self.episodes.borrow().is_empty());
        if on { self.poster_box.add_css_class("skeleton") } else { self.poster_box.remove_css_class("skeleton") }
    }

    fn show_watchlist(&self, on: bool) {
        self.watchlist.set_icon_name(if on { "qu-watchlisted-symbolic" } else { "qu-watchlist-symbolic" });
        self.watchlist.set_tooltip_text(Some(if on { "Remove from Watchlist" } else { "Add to Watchlist" }));
    }

    pub fn reload(self: &Rc<Self>) {
        let a = self.anime.borrow().clone();
        self.open(a);
    }

    fn message(&self, text: &str) {
        self.status.set_text(text);
        self.status.set_visible(true);
    }

    fn set_anime(&self, a: &Anime) {
        self.title.set_text(&a.title);
        if self.poster.paintable().is_none() {
            crate::grid::load_poster(&self.poster, a.poster.clone());
        }
        self.dub_tab.set_visible(a.dub.is_some_and(|n| n > 0));
        self.sub_tab.set_visible(self.dub_tab.is_visible());
    }

    fn set_details(&self, d: Details) {
        // Bekanntes (z. B. aus der Suche) behalten, wenn die Seite etwas nicht liefert.
        let old = self.anime.borrow().clone();
        let a = Anime {
            poster: d.anime.poster.or(old.poster),
            sub: d.anime.sub.or(old.sub),
            dub: d.anime.dub.or(old.dub),
            kind: d.anime.kind.or(old.kind),
            duration: d.anime.duration.or(old.duration),
            title: if d.anime.title.is_empty() { old.title } else { d.anime.title },
            id: old.id,
        };
        self.set_anime(&a);
        let meta: Vec<String> = [a.kind.clone(), a.duration.clone()].into_iter().flatten().chain(d.info).collect();
        let mut lines = vec![meta.join(" · ")];
        if !d.genres.is_empty() {
            lines.push(d.genres.join(", "));
        }
        self.meta.set_text(&lines.join("\n"));
        self.description.set_text(&d.description);
        self.more.set_visible(d.description.len() > 300);
        self.anime.replace(a);
    }

    /// Episoden-Kacheln: Nummer, Titel, Fortschritt. Nicht synchronisierte Episoden im Dub-Modus ausgegraut.
    fn fill_episodes(&self) {
        self.flow.remove_all();
        let a = self.anime.borrow().clone();
        for ep in self.episodes.borrow().iter() {
            let tile = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(2).width_request(200).css_classes(["episode"]).build();
            tile.append(&gtk::Label::builder().label(format!("Episode {}", ep.number)).xalign(0.0).css_classes(["heading"]).build());
            let title = gtk::Label::builder()
                .label(&ep.title)
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(1)
                .hexpand(true)
                .css_classes(["dim-label", "caption"])
                .build();
            tile.append(&title);
            let entry = crate::progress::get(&a.id, ep.number);
            let bar = gtk::ProgressBar::builder().fraction(entry.as_ref().map_or(0.0, |e| e.fraction())).margin_top(4).css_classes(["watched"]).build();
            bar.set_opacity(if entry.is_some() { 1.0 } else { 0.0 });
            tile.append(&bar);
            qu_core::fly::register(&ep.id, &tile);
            if entry.is_some_and(|e| e.finished()) {
                tile.set_opacity(0.5);
            }
            let ok = available(&a, ep.number, self.dub.get());
            let child = gtk::FlowBoxChild::builder().child(&tile).sensitive(ok).build();
            if ok {
                child.set_cursor_from_name(Some("pointer"));
            }
            self.flow.append(&child);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_respects_dub() {
        let eps: Rc<Vec<Episode>> =
            Rc::new((1..=3).map(|n| Episode { id: n.to_string(), number: n, title: String::new() }).collect());
        let a = Anime { dub: Some(2), ..Default::default() };
        let p = Play { anime: a, episodes: eps, index: 0, dub: true };
        let n = p.next().expect("Episode 2 gibt es synchronisiert");
        assert_eq!(n.episode().number, 2);
        assert!(n.next().is_none(), "Episode 3 noch nicht synchronisiert");
        assert_eq!(Play { dub: false, ..n.clone() }.next().map(|p| p.index), Some(2));
        assert_eq!(n.prev().map(|p| p.index), Some(0));
    }
}
