//! aniqu-core für Kotlin (UniFFI). Nur Durchreichen: eigene Typen als `Record`, Netz-Aufrufe laufen auf
//! qu-cores tokio (`bg`), Kotlin wartet per `suspend`. Zuerst `init` aufrufen (Ordner, qu.toml).

use aniqu_core::{anime, aniskip, lists, progress};

uniffi::setup_scaffolding!();

#[derive(Debug, uniffi::Error)]
pub enum AniError {
    Failed { msg: String },
}

impl std::fmt::Display for AniError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let AniError::Failed { msg } = self;
        f.write_str(msg)
    }
}

impl From<String> for AniError {
    fn from(msg: String) -> Self {
        AniError::Failed { msg }
    }
}

#[derive(uniffi::Record)]
pub struct Anime {
    pub id: String,
    pub title: String,
    pub poster: Option<String>,
    pub sub: Option<u32>,
    pub dub: Option<u32>,
    pub kind: Option<String>,
    pub duration: Option<String>,
}

impl From<anime::Anime> for Anime {
    fn from(a: anime::Anime) -> Self {
        Anime { id: a.id, title: a.title, poster: a.poster, sub: a.sub, dub: a.dub, kind: a.kind, duration: a.duration }
    }
}

impl From<Anime> for anime::Anime {
    fn from(a: Anime) -> Self {
        anime::Anime { id: a.id, title: a.title, poster: a.poster, sub: a.sub, dub: a.dub, kind: a.kind, duration: a.duration }
    }
}

#[derive(uniffi::Record)]
pub struct Details {
    pub anime: Anime,
    pub description: String,
    pub info: Vec<String>,
    pub genres: Vec<String>,
}

#[derive(uniffi::Record)]
pub struct Episode {
    pub id: String,
    pub number: u32,
    pub title: String,
}

#[derive(uniffi::Record)]
pub struct Variant {
    /// Bildhöhe, 0 = unbekannt.
    pub height: u32,
    pub url: String,
}

#[derive(uniffi::Record)]
pub struct Stream {
    /// Beste zuerst.
    pub variants: Vec<Variant>,
    /// Als Referer-Header mitschicken.
    pub referer: String,
    pub user_agent: String,
    /// Lokale VTT-Dateien, Dateiname = Spur-Titel („English.vtt“), Standard zuerst.
    pub subtitles: Vec<String>,
    pub mal_id: Option<String>,
}

#[derive(uniffi::Record)]
pub struct Resume {
    pub anime: Anime,
    pub episode: u32,
    pub fraction: f64,
}

#[derive(uniffi::Record)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    /// „intro“, „ending“ …
    pub label: String,
}

fn list(v: Vec<anime::Anime>) -> Vec<Anime> {
    v.into_iter().map(Anime::from).collect()
}

/// Einmal beim Start: Ordner der App (`filesDir`, `cacheDir`) und Einstellungen aus qu.toml.
#[uniffi::export]
pub fn init(files_dir: String, cache_dir: String) {
    // Untertitel landen in temp_dir(); Androids Standard (/data/local/tmp) darf die App nicht beschreiben.
    // SAFETY: einmal beim Start, bevor Threads (tokio) laufen.
    unsafe { std::env::set_var("TMPDIR", &cache_dir) };
    qu_core::app::set_dirs(files_dir.into(), cache_dir.into());
    qu_core::app::init("aniqu");
    qu_core::config::load(include_str!("../../qu.toml"));
}

#[uniffi::export]
pub async fn trending() -> Result<Vec<Anime>, AniError> {
    let v = qu_core::bg(anime::trending()).await?;
    anime::save_trending(&v);
    Ok(list(v))
}

/// Letzte Trending-Liste (sofort, ohne Netz).
#[uniffi::export]
pub fn saved_trending() -> Vec<Anime> {
    list(anime::saved_trending())
}

#[uniffi::export]
pub async fn search(query: String) -> Result<Vec<Anime>, AniError> {
    Ok(list(qu_core::bg(anime::search(query)).await?))
}

#[uniffi::export]
pub async fn details(id: String) -> Result<Details, AniError> {
    let d = qu_core::bg(anime::details(id)).await?;
    Ok(Details { anime: d.anime.into(), description: d.description, info: d.info, genres: d.genres })
}

#[uniffi::export]
pub async fn episodes(id: String) -> Result<Vec<Episode>, AniError> {
    let v = qu_core::bg(anime::episodes(id)).await?;
    Ok(v.into_iter().map(|e| Episode { id: e.id, number: e.number, title: e.title }).collect())
}

/// `dub`: synchronisiert statt Untertitel.
#[uniffi::export]
pub async fn stream(episode_id: String, dub: bool) -> Result<Stream, AniError> {
    let s = qu_core::bg(anime::stream(episode_id, if dub { "dub" } else { "sub" }.into())).await?;
    Ok(Stream {
        variants: s.variants.into_iter().map(|(height, url)| Variant { height, url }).collect(),
        referer: s.referer,
        user_agent: anime::USER_AGENT.into(),
        subtitles: s.subtitles,
        mal_id: s.mal_id,
    })
}

/// Opening/Ending (AniSkip); leer, wenn unbekannt.
#[uniffi::export]
pub async fn segments(mal_id: String, episode: u32) -> Vec<Segment> {
    let v = qu_core::bg(aniskip::segments(mal_id, episode)).await;
    v.into_iter().map(|s| Segment { start: s.start, end: s.end, label: s.label.into() }).collect()
}

/// Anime-ID und Episoden-ID aus einem hianime-Link (geteilt aus dem Browser).
#[uniffi::export]
pub fn parse_link(url: String) -> Option<Vec<String>> {
    let id = anime::anime_id(&url)?;
    Some([Some(id), anime::episode_param(&url)].into_iter().flatten().collect())
}

#[uniffi::export]
pub fn available(a: Anime, number: u32, dub: bool) -> bool {
    anime::available(&a.into(), number, dub)
}

#[uniffi::export]
pub fn continue_watching(n: u32) -> Vec<Resume> {
    progress::continue_watching(n as usize)
        .into_iter()
        .map(|r| Resume { anime: r.anime.into(), episode: r.episode, fraction: r.fraction })
        .collect()
}

/// Wo weiterschauen (Sekunden), falls angefangen und nicht fertig.
#[uniffi::export]
pub fn resume_at(anime: String, episode: u32) -> Option<f64> {
    progress::resume_at(&anime, episode)
}

/// Fortschritt speichern (alle paar Sekunden und beim Verlassen). `total`: Episodenzahl des Animes.
#[uniffi::export]
pub fn record(a: Anime, episode: u32, total: u32, pos: f64, dur: f64) {
    let m = progress::Meta { anime: a.id, episode, title: a.title, poster: a.poster, total };
    progress::record(&m, pos, dur);
}

/// „Watched“: aus „Continue watching“ nehmen.
#[uniffi::export]
pub fn hide(anime: String) {
    progress::hide(&anime);
}

#[uniffi::export]
pub fn watchlist() -> Vec<Anime> {
    list(lists::watchlist())
}

#[uniffi::export]
pub fn on_watchlist(id: String) -> bool {
    lists::on_watchlist(&id)
}

/// Gibt zurück, ob der Anime jetzt auf der Watchlist steht.
#[uniffi::export]
pub fn toggle_watchlist(a: Anime) -> bool {
    lists::toggle_watchlist(&a.into())
}

#[uniffi::export]
pub fn dub(anime: String) -> bool {
    lists::dub(&anime)
}

#[uniffi::export]
pub fn set_dub(anime: String, dub: bool) {
    lists::set_dub(&anime, dub);
}

#[uniffi::export]
pub fn speed(anime: String) -> f64 {
    lists::anime_speed(&anime)
}

#[uniffi::export]
pub fn set_speed(anime: String, speed: f64) {
    lists::set_anime_speed(&anime, speed);
}
