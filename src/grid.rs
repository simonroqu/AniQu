//! Raster aus Anime-Karten (Poster). Startseite („Continue watching“ + Trending) und Suche.

use crate::anime::{self, Anime};
use crate::bg;
use crate::progress::{self, Resume};
use gtk4 as gtk;
use gtk::prelude::*;
use gtk::{gdk, glib};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub fn fmt_duration(s: u32) -> String {
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// Kleinste Kartenbreite plus Abstand, um aus der verfügbaren Breite die Spaltenzahl zu berechnen.
/// Die Karten wachsen dann mit, bis die Reihe gefüllt ist.
const CARD_WIDTH: i32 = 170;
/// Höchstens so viele Karten pro Reihe. Hoch genug, dass Poster im Vollbild nicht riesig werden
/// (Kartenbreite bleibt so etwa zwischen 170 und 250 px).
const MAX_COLUMNS: i32 = 12;
const COLUMN_SPACING: i32 = 16;
/// Höchstens so viele Animes in „Continue watching“.
const SHELF_MAX: usize = 30;
/// So viele Animes, bei denen man auf dem Stand ist, werden auf neue Folgen geprüft (je 1 Anfrage).
const NEW_EPISODE_CHECKS: usize = 10;
/// Neue Folgen höchstens so oft prüfen (Startseite wird oft geöffnet).
const NEW_EPISODE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(10 * 60);

thread_local! {
    static LAST_CHECK: Cell<Option<std::time::Instant>> = const { Cell::new(None) };
}
/// Abstand oben bis zur ersten Karte (unter der schwebenden Navigation).
const TOP_MARGIN: i32 = 56;

/// Spalten für eine Breite: so viele Karten wie passen, höchstens `MAX_COLUMNS`.
fn columns(width: i32) -> usize {
    ((width + COLUMN_SPACING) / (CARD_WIDTH + COLUMN_SPACING + 8)).clamp(1, MAX_COLUMNS) as usize
}

/// Poster-Höhe (Hochformat 2:3) zur Kartenbreite.
fn poster_height(width: i32) -> i32 {
    width * 3 / 2
}

fn label(text: &str, classes: &[&str]) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .lines(2)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(1) // Breite kommt von der Karte, nicht vom Text
        .hexpand(true)
        .css_classes(classes.to_vec())
        .build()
}

/// Poster laden (aus dem Cache oder aus dem Netz) und anzeigen.
pub fn load_poster(pic: &gtk::Picture, url: Option<String>) {
    let Some(url) = url else { return };
    let pic = pic.downgrade();
    glib::spawn_future_local(async move {
        let Ok(bytes) = bg(anime::poster(url)).await else { return };
        let Ok(tex) = gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes)) else { return };
        if let Some(pic) = pic.upgrade() {
            pic.set_paintable(Some(&tex));
        }
    });
}

/// Was eine Karte beim Anklicken öffnet.
#[derive(Clone)]
enum Item {
    Anime(Anime),
    Resume(Resume),
}

fn card(item: &Item, width: i32) -> (gtk::Box, gtk::Picture) {
    let (a, meta, fraction) = match item {
        Item::Anime(a) => {
            let meta: Vec<String> = [a.kind.clone(), a.duration.clone()].into_iter().flatten().collect();
            (a, meta.join(" · "), None)
        }
        Item::Resume(r) => (&r.anime, format!("Episode {}", r.episode), Some(r.fraction)),
    };
    let pic = gtk::Picture::builder()
        .content_fit(gtk::ContentFit::Cover)
        .can_shrink(true)
        .height_request(poster_height(width))
        .css_classes(["thumb"])
        .build();
    load_poster(&pic, a.poster.clone());
    let thumb = gtk::Overlay::builder().child(&pic).build();

    // Anzahl Episoden mit Untertiteln / Synchronisation, unten links.
    let badges = gtk::Box::builder().spacing(4).halign(gtk::Align::Start).valign(gtk::Align::End).margin_start(6).margin_bottom(6).build();
    for (n, tag) in [(a.sub, "SUB"), (a.dub, "DUB")] {
        if let Some(n) = n.filter(|n| *n > 0) {
            badges.append(&gtk::Label::builder().label(format!("{tag} {n}")).css_classes(["osd", "badge"]).build());
        }
    }
    thumb.add_overlay(&badges);
    if let Some(f) = fraction.filter(|f| *f > 0.0) {
        let bar = gtk::ProgressBar::builder().valign(gtk::Align::End).fraction(f).css_classes(["watched"]).build();
        thumb.add_overlay(&bar);
    }

    // Keine Mindestbreite: die Spaltenzahl folgt aus `columns`, die Karte füllt ihre Spalte.
    let b = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(4).build();
    b.append(&thumb);
    b.append(&label(&a.title, &["heading"]));
    if !meta.is_empty() {
        b.append(&label(&meta, &["dim-label", "caption"]));
    }
    (b, pic)
}

/// Überschrift plus Karten-Reihe(n).
struct Section {
    title: gtk::Label,
    flow: gtk::FlowBox,
    items: RefCell<Vec<Item>>,
    pics: RefCell<Vec<gtk::Picture>>,
}

impl Section {
    fn new(title: &str) -> Rc<Self> {
        let flow = gtk::FlowBox::builder()
            .homogeneous(true)
            .selection_mode(gtk::SelectionMode::None)
            .activate_on_single_click(true)
            .row_spacing(24)
            .column_spacing(COLUMN_SPACING as u32)
            .valign(gtk::Align::Start)
            .build();
        let title = gtk::Label::builder().label(title).xalign(0.0).margin_bottom(12).css_classes(["title-3"]).visible(!title.is_empty()).build();
        Rc::new(Section { title, flow, items: RefCell::default(), pics: RefCell::default() })
    }

    fn set(&self, items: Vec<Item>, width: i32) {
        self.flow.remove_all();
        self.pics.borrow_mut().clear();
        for item in &items {
            let (card, pic) = card(item, width);
            self.pics.borrow_mut().push(pic);
            let child = gtk::FlowBoxChild::builder().child(&card).cursor(&gdk::Cursor::from_name("pointer", None).unwrap()).build();
            self.flow.append(&child);
        }
        let empty = items.is_empty();
        self.items.replace(items);
        self.flow.set_visible(!empty);
        if !self.title.label().is_empty() {
            self.title.set_visible(!empty);
        }
    }
}

pub struct Grid {
    pub widget: gtk::Overlay,
    scroll: gtk::ScrolledWindow,
    trending: Rc<Section>,
    shelf: Rc<Section>,
    results: Rc<Section>,
    status: gtk::Label,
    spinner: gtk::Image,
    /// `None` = Startseite, sonst die angezeigte Suche (für Reload).
    query: RefCell<Option<String>>,
    column_width: Cell<i32>,
    /// Wird bei jedem neuen Inhalt erhöht, damit verspätete Antworten alter Suchen verworfen werden.
    generation: Cell<u64>,
}

impl Grid {
    pub fn new(on_open: impl Fn(Anime) + 'static, on_resume: impl Fn(Resume) + 'static) -> Rc<Self> {
        let trending = Section::new("Trending");
        let shelf = Section::new("Continue watching");
        let results = Section::new("");
        // Laden: nur ein Spinner. Das Label zeigt nur Fehler oder „keine Ergebnisse“.
        let status = gtk::Label::builder().visible(false).margin_top(48).wrap(true).build();
        let spinner = crate::icons::spinner(32).margin_top(48).visible(false).build();

        // Unsichtbare Messfläche über der ganzen Seite: meldet die Fensterbreite, daraus folgt die Spaltenzahl.
        let sizer = gtk::DrawingArea::builder().can_target(false).build();
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .margin_top(TOP_MARGIN)
            .margin_start(16)
            .margin_end(16)
            .margin_bottom(32)
            .build();
        content.append(&spinner);
        content.append(&status);
        // „Continue watching“ ganz oben, darunter Trending.
        for s in [&shelf, &trending, &results] {
            content.append(&s.title);
            content.append(&s.flow);
        }
        let scroll = gtk::ScrolledWindow::builder().child(&content).hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).build();
        let widget = gtk::Overlay::builder().child(&scroll).build();
        widget.add_overlay(&sizer);

        let grid = Rc::new(Grid {
            widget,
            scroll,
            trending,
            shelf,
            results,
            status,
            spinner,
            query: RefCell::default(),
            column_width: Cell::new(CARD_WIDTH),
            generation: Cell::new(0),
        });

        let (on_open, on_resume): (Rc<dyn Fn(Anime)>, Rc<dyn Fn(Resume)>) = (Rc::new(on_open), Rc::new(on_resume));
        for s in [&grid.trending, &grid.shelf, &grid.results] {
            let (s2, on_open, on_resume) = (Rc::downgrade(s), on_open.clone(), on_resume.clone());
            s.flow.connect_child_activated(move |_, child| {
                let Some(s) = s2.upgrade() else { return };
                let item = s.items.borrow().get(child.index() as usize).cloned();
                match item {
                    Some(Item::Anime(a)) => on_open(a),
                    Some(Item::Resume(r)) => on_resume(r),
                    None => {}
                }
            });
        }

        sizer.connect_resize({
            let grid = Rc::downgrade(&grid);
            move |_, page_width, _| {
                // Abzüglich der Seitenränder des Inhalts (je 16 px).
                let width = page_width - 32;
                let cols = columns(width);
                let Some(grid) = grid.upgrade() else { return };
                let col_width = (width - (cols as i32 - 1) * COLUMN_SPACING) / cols as i32;
                if (col_width - grid.column_width.get()).abs() > 4 {
                    grid.column_width.set(col_width);
                    for s in [&grid.trending, &grid.shelf, &grid.results] {
                        for pic in s.pics.borrow().iter() {
                            pic.set_height_request(poster_height(col_width));
                        }
                    }
                }
                // Feste Spaltenzahl (FlowBox würde sonst nach der natürlichen Bildbreite umbrechen).
                // Nicht mitten in der Größenberechnung umbauen, sondern direkt danach.
                glib::idle_add_local_once(move || {
                    for s in [&grid.trending, &grid.shelf, &grid.results] {
                        s.flow.set_min_children_per_line(cols as u32);
                        s.flow.set_max_children_per_line(cols as u32);
                    }
                });
            }
        });

        // Zurück auf der Startseite (z. B. nach einer Episode): „Continue watching“ auffrischen.
        grid.widget.connect_map({
            let grid = Rc::downgrade(&grid);
            move |_| {
                if let Some(grid) = grid.upgrade().filter(|g| g.query.borrow().is_none()) {
                    let adj = grid.scroll.vadjustment();
                    let pos = adj.value();
                    grid.fill_shelf();
                    glib::idle_add_local_once(move || adj.set_value(pos));
                }
            }
        });
        grid
    }

    fn fill_shelf(&self) {
        let items: Vec<Item> = progress::continue_watching(SHELF_MAX).into_iter().map(Item::Resume).collect();
        // Abstand zu Trending nur, wenn darüber „Continue watching“ steht.
        self.trending.title.set_margin_top(if items.is_empty() { 0 } else { 32 });
        self.shelf.set(items, self.column_width.get());
    }

    /// Laufende Animes, bei denen man auf dem Stand ist: neue Folge draußen? Dann kommt die nächste
    /// Episode in „Continue watching“. Im Hintergrund, höchstens alle 10 Minuten.
    fn check_new_episodes(self: &Rc<Self>) {
        if LAST_CHECK.with(|c| c.get()).is_some_and(|t| t.elapsed() < NEW_EPISODE_INTERVAL) {
            return;
        }
        let list = progress::caught_up(NEW_EPISODE_CHECKS);
        if list.is_empty() {
            return;
        }
        LAST_CHECK.with(|c| c.set(Some(std::time::Instant::now())));
        let grid = Rc::downgrade(self);
        let busy = crate::Busy::new();
        glib::spawn_future_local(async move {
            let found = bg(async move {
                // Alle gleichzeitig; Fehler (offline, Seite geändert) heißt einfach: keine neue Folge.
                let tasks: Vec<_> = list
                    .into_iter()
                    .map(|(id, total)| tokio::spawn(async move { (anime::details(id.clone()).await, id, total) }))
                    .collect();
                let mut found = Vec::new();
                for t in tasks {
                    let Ok((Ok(d), id, total)) = t.await else { continue };
                    // Verfügbar in der Sprache, die man schaut (Dub kommt oft später).
                    let now = if crate::lists::dub(&id) { d.anime.dub } else { d.anime.sub };
                    if let Some(n) = now.filter(|n| *n > total) {
                        found.push((id, n));
                    }
                }
                found
            })
            .await;
            drop(busy);
            for (id, n) in &found {
                progress::set_total(id, *n);
            }
            if let Some(g) = grid.upgrade().filter(|g| !found.is_empty() && g.is_home()) {
                g.fill_shelf();
            }
        });
    }

    fn begin(&self, query: Option<String>) -> u64 {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        self.query.replace(query);
        self.status.set_visible(false);
        self.scroll.vadjustment().set_value(0.0);
        generation
    }

    fn message(&self, text: &str) {
        self.spinner.set_visible(false);
        self.status.set_text(text);
        self.status.set_visible(true);
    }

    /// Startseite: gespeicherte Trending-Liste sofort, die neue ersetzt sie; darunter „Continue watching“.
    pub fn show_home(self: &Rc<Self>) {
        let generation = self.begin(None);
        let w = self.column_width.get();
        self.results.set(vec![], w);
        self.fill_shelf();
        self.check_new_episodes();
        let saved: Vec<Item> = anime::saved_trending().into_iter().map(Item::Anime).collect();
        let has_saved = !saved.is_empty();
        // Bisheriges bleibt beim Reload stehen, bis das Neue da ist (Lade-Kreis in der Bubble).
        if has_saved || self.trending.items.borrow().is_empty() {
            self.trending.set(saved, w);
        }
        self.spinner.set_visible(!has_saved && self.trending.items.borrow().is_empty());
        let grid = self.clone();
        let busy = crate::Busy::new();
        glib::spawn_future_local(async move {
            let res = bg(anime::trending()).await;
            drop(busy);
            if grid.generation.get() != generation {
                return;
            }
            grid.spinner.set_visible(false);
            match res {
                Ok(list) => {
                    anime::save_trending(&list);
                    let items = list.into_iter().map(Item::Anime).collect();
                    grid.trending.set(items, grid.column_width.get());
                }
                Err(e) if grid.trending.items.borrow().is_empty() => grid.message(&format!("Error: {e}")),
                Err(e) => eprintln!("Trending nicht geladen, zeige gespeicherte Liste: {e}"),
            }
        });
    }

    pub fn show_search(self: &Rc<Self>, query: String) {
        let generation = self.begin(Some(query.clone()));
        let w = self.column_width.get();
        for s in [&self.trending, &self.shelf, &self.results] {
            s.set(vec![], w);
        }
        self.spinner.set_visible(true);
        let grid = self.clone();
        let busy = crate::Busy::new();
        glib::spawn_future_local(async move {
            let res = bg(anime::search(query)).await;
            drop(busy);
            if grid.generation.get() != generation {
                return;
            }
            grid.spinner.set_visible(false);
            match res {
                Ok(list) if list.is_empty() => grid.message("No results"),
                Ok(list) => grid.results.set(list.into_iter().map(Item::Anime).collect(), grid.column_width.get()),
                Err(e) => grid.message(&format!("Error: {e}")),
            }
        });
    }

    /// Zeigt das Raster die Startseite (nicht Suchergebnisse)?
    pub fn is_home(&self) -> bool {
        self.query.borrow().is_none()
    }

    /// Aktuelle Seite neu laden.
    pub fn reload(self: &Rc<Self>) {
        let query = self.query.borrow().clone();
        match query {
            Some(q) => self.show_search(q),
            None => self.show_home(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_columns() {
        assert_eq!(fmt_duration(59), "0:59");
        assert_eq!(fmt_duration(3725), "1:02:05");
        // Spalten: schmal 1, Standardfenster (1280) 6, Vollbild 1920 → 9, 2560 → 13 → höchstens 12.
        assert_eq!(columns(150), 1);
        assert_eq!(columns(1248), 6);
        assert_eq!(columns(1888), 9);
        assert_eq!(columns(2528), 12);
        assert_eq!(poster_height(200), 300);
    }
}
