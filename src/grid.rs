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
/// So viele Trending-Animes zeigt hianime; auf ganze Reihen aufgerundet (6 Spalten → 12, 5 → 15).
const TRENDING_SHOWN: usize = 12;

/// Wie viele von `available` Trending-Animes zeigen: `TRENDING_SHOWN` auf volle Reihen aufgerundet;
/// reicht der Vorrat nicht, nur die vollen Reihen (mindestens eine, auch wenn unvollständig).
fn trending_count(available: usize, cols: usize) -> usize {
    let cols = cols.max(1);
    let want = TRENDING_SHOWN.div_ceil(cols) * cols;
    if available >= want { want } else { (available / cols * cols).max(available.min(cols)) }
}

/// Abstand oben bis zur ersten Karte (unter der schwebenden Navigation).
const TOP_MARGIN: i32 = 56;

/// Mit Strg+Mausrad wählbare Spaltenzahl (Poster: weniger als 4 wären riesig).
const MIN_WANT: usize = 4;
const MAX_WANT: usize = 12;
/// Gewählte Spalten werden nur so weit gezeigt, wie Karten dieser Breite noch passen.
const MIN_CARD_WIDTH: i32 = 120;

/// Spalten für eine Breite: ohne Wahl so viele Karten wie passen, höchstens `MAX_COLUMNS`;
/// mit Strg+Mausrad gewählt (`want`) diese Zahl, solange Karten ≥ `MIN_CARD_WIDTH` passen.
fn columns(width: i32, want: Option<usize>) -> usize {
    let fit = |card: i32| ((width + COLUMN_SPACING) / (card + COLUMN_SPACING + 8)).max(1) as usize;
    match want {
        Some(n) => n.min(fit(MIN_CARD_WIDTH)),
        None => fit(CARD_WIDTH).min(MAX_COLUMNS as usize),
    }
}

/// Eine Spalte mehr (`dy` < 0, Mausrad hoch) oder weniger, in `MIN_WANT..=MAX_WANT`.
fn step_columns(current: usize, dy: f64) -> usize {
    let n = if dy < 0.0 { current + 1 } else { current.saturating_sub(1) };
    n.clamp(MIN_WANT, MAX_WANT)
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
    // Unsichtbar bis zum Laden, dann einblenden (style.css `.fade`).
    pic.add_css_class("fade");
    pic.remove_css_class("loaded");
    let pic = pic.downgrade();
    glib::spawn_future_local(async move {
        let Ok(bytes) = bg(anime::poster(url)).await else { return };
        let Ok(tex) = gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes)) else { return };
        if let Some(pic) = pic.upgrade() {
            pic.set_paintable(Some(&tex));
            pic.add_css_class("loaded");
        }
    });
}

/// Lade-Platzhalter in Form einer Karte: Poster, Titel, Infos (style.css `.skeleton`). Gibt auch den
/// Poster-Platzhalter zurück: seine Höhe folgt wie bei echten Karten der Spaltenbreite (`fit`).
fn ghost_card(width: i32) -> (gtk::Box, gtk::Box) {
    let b = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(8).build();
    let poster = gtk::Box::builder().height_request(poster_height(width)).css_classes(["skeleton"]).build();
    b.append(&poster);
    b.append(&part(4, 5, 16));
    b.append(&part(1, 2, 12));
    (b, poster)
}

/// Grauer Balken über `k` von `n` gleich breiten Teilen der Zeile – ohne feste Pixelbreite,
/// die dem Fenster eine Mindestbreite aufzwänge.
pub fn part(k: i32, n: i32, height: i32) -> gtk::Grid {
    let g = gtk::Grid::builder().column_homogeneous(true).build();
    g.attach(&gtk::Box::builder().height_request(height).css_classes(["skeleton"]).build(), 0, 0, k, 1);
    // Jede freie Spalte braucht ein Kind, sonst zählt sie bei gleich breiten Spalten nicht mit.
    for i in k..n {
        g.attach(&gtk::Box::new(gtk::Orientation::Horizontal, 0), i, 0, 1, 1);
    }
    g
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
    // Grauer Platzhalter in Bildgröße, bis das Bild da ist; schneidet auch den Hover-Zoom ab.
    let thumb = gtk::Overlay::builder().child(&pic).overflow(gtk::Overflow::Hidden).css_classes(["thumb-box"]).build();

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
    card_menu(&b, &a.id, matches!(item, Item::Resume(_)));
    b.append(&thumb);
    b.append(&label(&a.title, &["heading"]));
    if !meta.is_empty() {
        b.append(&label(&meta, &["dim-label", "caption"]));
    }
    (b, pic)
}

/// Rechtsklick: Watchlist an/aus, bei „Continue watching“ zusätzlich „Watched“.
/// Die Aktionen (`card.*`) hängen am Raster, siehe `Grid::new`.
fn card_menu(card: &gtk::Box, id: &str, resume: bool) {
    let click = gtk::GestureClick::builder().button(3).build();
    let id = id.to_string();
    click.connect_pressed(move |g, _, x, y| {
        let Some(card) = g.widget() else { return };
        g.set_state(gtk::EventSequenceState::Claimed);
        let menu = gtk::gio::Menu::new();
        let label = if crate::lists::on_watchlist(&id) { "Remove from Watchlist" } else { "Add to Watchlist" };
        menu.append(Some(label), Some(&gtk::gio::Action::print_detailed_name("card.watchlist", Some(&id.to_variant()))));
        if resume {
            menu.append(Some("Watched"), Some(&gtk::gio::Action::print_detailed_name("card.watched", Some(&id.to_variant()))));
        }
        let pop = gtk::PopoverMenu::from_model(Some(&menu));
        pop.set_parent(&card);
        pop.set_has_arrow(false);
        pop.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        // Nach dem Schließen wieder vom Karten-Widget lösen.
        pop.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        pop.popup();
    });
    card.add_controller(click);
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
    watchlist: Rc<Section>,
    results: Rc<Section>,
    /// Trending ungefiltert; angezeigt ohne Animes auf der Watchlist (`fill_shelf`).
    trending_all: RefCell<Vec<Anime>>,
    status: gtk::Label,
    /// Graue Platzhalter-Karten beim Laden; ihre Poster-Höhen folgen der Spaltenbreite.
    ghost: gtk::FlowBox,
    ghost_posters: RefCell<Vec<gtk::Box>>,
    /// `None` = Startseite, sonst die angezeigte Suche (für Reload).
    query: RefCell<Option<String>>,
    column_width: Cell<i32>,
    /// Zuletzt gemessene Breite (ohne Seitenränder) und aktuelle Spaltenzahl.
    width: Cell<i32>,
    columns: Cell<usize>,
    /// Mit Strg+Mausrad gewählte Spaltenzahl, gespeichert in `~/.config/aniqu/columns`.
    want: Cell<Option<usize>>,
    /// Wird bei jedem neuen Inhalt erhöht, damit verspätete Antworten alter Suchen verworfen werden.
    generation: Cell<u64>,
}

impl Grid {
    pub fn new(on_open: impl Fn(Anime) + 'static, on_resume: impl Fn(Resume) + 'static) -> Rc<Self> {
        let trending = Section::new("Trending");
        let shelf = Section::new("Continue watching");
        let watchlist = Section::new("Watchlist");
        let results = Section::new("");
        // Laden: graue Platzhalter-Karten. Das Label zeigt nur Fehler oder „keine Ergebnisse“.
        let status = gtk::Label::builder().visible(false).margin_top(48).wrap(true).build();
        let ghost = gtk::FlowBox::builder()
            .homogeneous(true)
            .selection_mode(gtk::SelectionMode::None)
            .row_spacing(24)
            .column_spacing(COLUMN_SPACING as u32)
            .valign(gtk::Align::Start)
            .can_target(false)
            .visible(false)
            .build();

        // Unsichtbare Messfläche über der ganzen Seite: meldet die Fensterbreite, daraus folgt die Spaltenzahl.
        let sizer = gtk::DrawingArea::builder().can_target(false).build();
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .margin_top(TOP_MARGIN)
            .margin_start(16)
            .margin_end(16)
            .margin_bottom(32)
            .build();
        content.append(&ghost);
        content.append(&status);
        // „Continue watching“ ganz oben, dann Watchlist, darunter Trending.
        for s in [&shelf, &watchlist, &trending, &results] {
            content.append(&s.title);
            content.append(&s.flow);
        }
        let scroll = gtk::ScrolledWindow::builder().child(&content).hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).build();
        crate::smooth::attach(&scroll);
        let widget = gtk::Overlay::builder().child(&scroll).build();
        widget.add_overlay(&sizer);

        let grid = Rc::new(Grid {
            widget,
            scroll,
            trending,
            shelf,
            watchlist,
            results,
            trending_all: RefCell::default(),
            status,
            ghost,
            ghost_posters: RefCell::default(),
            query: RefCell::default(),
            column_width: Cell::new(CARD_WIDTH),
            width: Cell::new(0),
            columns: Cell::new(0),
            want: Cell::new(
                std::fs::read_to_string(crate::controls::config_file("columns")).ok().and_then(|s| s.trim().parse().ok()),
            ),
            generation: Cell::new(0),
        });

        let (on_open, on_resume): (Rc<dyn Fn(Anime)>, Rc<dyn Fn(Resume)>) = (Rc::new(on_open), Rc::new(on_resume));
        for s in [&grid.trending, &grid.shelf, &grid.watchlist, &grid.results] {
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
                if let Some(grid) = grid.upgrade() {
                    // Abzüglich der Seitenränder des Inhalts (je 16 px).
                    grid.width.set(page_width - 32);
                    grid.fit();
                }
            }
        });

        // Strg+Mausrad: eine Spalte mehr/weniger (vor dem weichen Scrollen der Seite).
        let zoom_ctl = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        zoom_ctl.set_propagation_phase(gtk::PropagationPhase::Capture);
        zoom_ctl.connect_scroll({
            let grid = Rc::downgrade(&grid);
            move |c, _, dy| {
                let Some(grid) = grid.upgrade() else { return glib::Propagation::Proceed };
                if !c.current_event_state().contains(gdk::ModifierType::CONTROL_MASK) {
                    return glib::Propagation::Proceed;
                }
                // ponytail: Touchpad (Pixel statt Rasten) wird nur geschluckt; aufsummieren, falls gewünscht.
                if c.unit() == gdk::ScrollUnit::Wheel && dy != 0.0 {
                    let n = step_columns(grid.columns.get(), dy);
                    grid.want.set(Some(n));
                    let f = crate::controls::config_file("columns");
                    let _ = std::fs::create_dir_all(f.parent().unwrap());
                    let _ = std::fs::write(f, n.to_string());
                    grid.fit();
                }
                glib::Propagation::Stop
            }
        });
        grid.widget.add_controller(zoom_ctl);

        // Aktionen für das Karten-Menü (`card_menu`); Parameter = Anime-ID.
        let actions = gtk::gio::SimpleActionGroup::new();
        let add = |name: &str, f: fn(&Rc<Grid>, &str)| {
            let a = gtk::gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
            let grid = Rc::downgrade(&grid);
            a.connect_activate(move |_, p| {
                if let (Some(g), Some(id)) = (grid.upgrade(), p.and_then(|p| p.get::<String>())) {
                    f(&g, &id);
                }
            });
            actions.add_action(&a);
        };
        add("watchlist", |g, id| {
            if let Some(a) = g.find(id) {
                crate::lists::toggle_watchlist(&a);
                if g.is_home() {
                    g.fill_shelf();
                }
            }
        });
        add("watched", |g, id| {
            progress::hide(id);
            g.fill_shelf();
        });
        grid.widget.insert_action_group("card", Some(&actions));

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

    /// Spaltenzahl und Posterhöhen aus der gemessenen Breite und der Spaltenwahl.
    fn fit(self: &Rc<Self>) {
        let width = self.width.get();
        if width <= 0 {
            return;
        }
        let cols = columns(width, self.want.get());
        if cols != self.columns.get() {
            self.columns.set(cols);
            // Trending füllt immer ganze Reihen: bei anderer Spaltenzahl neu zuschneiden.
            if self.is_home() {
                self.fill_trending();
            }
        }
        let col_width = (width - (cols as i32 - 1) * COLUMN_SPACING) / cols as i32;
        if (col_width - self.column_width.get()).abs() > 4 {
            self.column_width.set(col_width);
            for s in [&self.trending, &self.shelf, &self.watchlist, &self.results] {
                for pic in s.pics.borrow().iter() {
                    pic.set_height_request(poster_height(col_width));
                }
            }
            for p in self.ghost_posters.borrow().iter() {
                p.set_height_request(poster_height(col_width));
            }
        }
        // Feste Spaltenzahl (FlowBox würde sonst nach der natürlichen Bildbreite umbrechen).
        // Nicht mitten in der Größenberechnung umbauen, sondern direkt danach.
        let grid = self.clone();
        glib::idle_add_local_once(move || {
            for flow in [&grid.trending.flow, &grid.shelf.flow, &grid.watchlist.flow, &grid.results.flow, &grid.ghost] {
                flow.set_min_children_per_line(cols as u32);
                flow.set_max_children_per_line(cols as u32);
            }
        });
    }

    fn fill_shelf(&self) {
        let items: Vec<Item> = progress::continue_watching(SHELF_MAX).into_iter().map(Item::Resume).collect();
        let marked: Vec<Item> = crate::lists::watchlist().into_iter().map(Item::Anime).collect();
        // Abstand zum Abschnitt darüber nur, wenn einer darüber steht.
        self.watchlist.title.set_margin_top(if items.is_empty() { 0 } else { 32 });
        self.trending.title.set_margin_top(if items.is_empty() && marked.is_empty() { 0 } else { 32 });
        self.shelf.set(items, self.column_width.get());
        self.watchlist.set(marked, self.column_width.get());
        self.fill_trending();
    }

    /// Trending ohne Watchlist-Animes, aufgefüllt aus der Reserve, immer ganze Reihen.
    fn fill_trending(&self) {
        let list: Vec<Anime> = self.trending_all.borrow().iter().filter(|a| !crate::lists::on_watchlist(&a.id)).cloned().collect();
        let n = trending_count(list.len(), self.columns.get());
        self.trending.set(list.into_iter().take(n).map(Item::Anime).collect(), self.column_width.get());
    }

    /// Angezeigter Anime mit dieser ID (für das Karten-Menü).
    fn find(&self, id: &str) -> Option<Anime> {
        [&self.shelf, &self.watchlist, &self.trending, &self.results].iter().find_map(|s| {
            s.items.borrow().iter().find_map(|i| match i {
                Item::Anime(a) if a.id == id => Some(a.clone()),
                Item::Resume(r) if r.anime.id == id => Some(r.anime.clone()),
                _ => None,
            })
        })
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

    /// Platzhalter-Karten zeigen (an) oder entfernen (aus). So viele Reihen, dass die sichtbare Höhe
    /// gefüllt ist; beim App-Start (noch ohne Größe) 4 Reihen bei 6 Spalten.
    fn set_ghost(&self, on: bool) {
        self.ghost.remove_all();
        self.ghost_posters.borrow_mut().clear();
        self.ghost.set_visible(on);
        if !on {
            return;
        }
        let cols = if self.columns.get() > 0 { self.columns.get() } else { 6 };
        let width = self.column_width.get();
        // ponytail: ~60 px Text unter dem Poster + Reihenabstand geschätzt.
        let h = self.scroll.height();
        let rows = if h > 0 { (h / (poster_height(width) + 60).max(1)) as usize + 1 } else { 4 };
        self.ghost.set_min_children_per_line(cols as u32);
        self.ghost.set_max_children_per_line(cols as u32);
        for _ in 0..rows.max(1) * cols {
            let (card, poster) = ghost_card(width);
            self.ghost.append(&card);
            self.ghost_posters.borrow_mut().push(poster);
        }
    }

    fn message(&self, text: &str) {
        self.set_ghost(false);
        self.status.set_text(text);
        self.status.set_visible(true);
    }

    /// Startseite: gespeicherte Trending-Liste sofort, die neue ersetzt sie; darunter „Continue watching“.
    pub fn show_home(self: &Rc<Self>) {
        let generation = self.begin(None);
        let w = self.column_width.get();
        self.results.set(vec![], w);
        let saved = anime::saved_trending();
        let has_saved = !saved.is_empty();
        // Bisheriges bleibt beim Reload stehen, bis das Neue da ist (Lade-Kreis in der Bubble).
        if has_saved {
            self.trending_all.replace(saved);
        }
        self.fill_shelf();
        self.check_new_episodes();
        self.set_ghost(self.trending_all.borrow().is_empty());
        let grid = self.clone();
        let busy = crate::Busy::new();
        glib::spawn_future_local(async move {
            let res = bg(anime::trending()).await;
            drop(busy);
            if grid.generation.get() != generation {
                return;
            }
            grid.set_ghost(false);
            match res {
                Ok(list) => {
                    anime::save_trending(&list);
                    grid.trending_all.replace(list);
                    grid.fill_shelf();
                }
                Err(e) if grid.trending_all.borrow().is_empty() => grid.message(&format!("Error: {e}")),
                Err(e) => eprintln!("Trending nicht geladen, zeige gespeicherte Liste: {e}"),
            }
        });
    }

    pub fn show_search(self: &Rc<Self>, query: String) {
        let generation = self.begin(Some(query.clone()));
        let w = self.column_width.get();
        for s in [&self.trending, &self.shelf, &self.watchlist, &self.results] {
            s.set(vec![], w);
        }
        self.set_ghost(true);
        let grid = self.clone();
        let busy = crate::Busy::new();
        glib::spawn_future_local(async move {
            let res = bg(anime::search(query)).await;
            drop(busy);
            if grid.generation.get() != generation {
                return;
            }
            grid.set_ghost(false);
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
        assert_eq!(columns(150, None), 1);
        assert_eq!(columns(1248, None), 6);
        assert_eq!(columns(1888, None), 9);
        assert_eq!(columns(2528, None), 12);
        // Gewählt: so viele, wie 120-px-Karten passen.
        assert_eq!(columns(2528, Some(4)), 4);
        assert_eq!(columns(1248, Some(12)), 8);
        assert_eq!(step_columns(6, -1.0), 7);
        assert_eq!(step_columns(12, -1.0), 12);
        assert_eq!(step_columns(4, 1.0), 4);
        assert_eq!(poster_height(200), 300);
        // Trending: 12 auf volle Reihen, Reserve reicht nicht → nur volle Reihen.
        assert_eq!(trending_count(40, 6), 12);
        assert_eq!(trending_count(40, 5), 15);
        assert_eq!(trending_count(40, 12), 12);
        assert_eq!(trending_count(11, 6), 6);
        assert_eq!(trending_count(4, 6), 4);
    }
}
