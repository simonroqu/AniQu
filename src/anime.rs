//! Einzige Stelle, die mit hianime.at spricht. Ändert die Seite etwas, wird nur hier repariert.
//! Ablauf wie ani-cli 5.1.0: Suche/Startseite/Anime-Seite als HTML, Episoden und Server über
//! `/api/theme/…`, Stream über die ZokoAnime-Einbettung (base64-Adresse, XOR-verschlüsseltes JSON).

use base64::Engine;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::Semaphore;

const BASE: &str = "https://hianime.at";
/// Wie ani-cli: als normaler Browser auftreten, sonst blockt Cloudflare.
pub const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
/// Schlüssel, mit dem die Einbettung ihr JSON verschleiert (`window.__P`).
const EMBED_KEY: &[u8] = b"otaku-embed-v1";
/// Gleichzeitige Poster-Downloads.
const POSTER_PARALLEL: usize = 6;

static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder().user_agent(USER_AGENT).timeout(Duration::from_secs(15)).build().expect("HTTP-Client")
});
static POSTER_SLOTS: Semaphore = Semaphore::const_new(POSTER_PARALLEL);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Anime {
    /// Pfad auf hianime, z. B. „frieren-beyond-journeys-end-481“ (am Ende die Nummer für die Episoden-API).
    pub id: String,
    pub title: String,
    pub poster: Option<String>,
    /// Anzahl Episoden mit Untertiteln bzw. Synchronisation.
    pub sub: Option<u32>,
    pub dub: Option<u32>,
    /// „TV“, „Movie“ …
    pub kind: Option<String>,
    /// Episodenlänge, z. B. „25m“.
    pub duration: Option<String>,
}

pub struct Details {
    pub anime: Anime,
    pub description: String,
    /// Kurze Infos für die Zeile unter dem Titel, z. B. „Finished Airing“, „MAL 9.36“.
    pub info: Vec<String>,
    pub genres: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Episode {
    pub id: String,
    pub number: u32,
    pub title: String,
}

#[derive(Clone, Debug, Default)]
pub struct Stream {
    /// Qualitätsstufen (Bildhöhe, Adresse), beste zuerst. Höhe 0 = unbekannt (nur eine Stufe).
    pub variants: Vec<(u32, String)>,
    /// Der Stream-Server will die Einbettungs-Seite als Referer.
    pub referer: String,
    /// Untertitel als lokale Dateien („…/English.vtt“, mpv zeigt den Dateinamen als Spur-Titel), Standard zuerst.
    pub subtitles: Vec<String>,
    /// MyAnimeList-ID für ani-skip (steckt in der Einbettungs-Adresse).
    pub mal_id: Option<String>,
}

// ---------- Hilfen zum Auslesen ----------

fn between<'a>(s: &'a str, a: &str, b: &str) -> Option<&'a str> {
    let i = s.find(a)? + a.len();
    Some(&s[i..i + s[i..].find(b)?])
}

fn attr(s: &str, name: &str) -> Option<String> {
    between(s, &format!("{name}=\""), "\"").map(unescape)
}

fn unescape(s: &str) -> String {
    s.replace("&#039;", "'")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Tags entfernen, Leerraum zusammenfassen.
fn text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    unescape(&out.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn number(html: Option<&str>) -> Option<u32> {
    text(html?).parse().ok()
}

/// Letzter Pfadteil einer Adresse: „https://hianime.at/watch/x-481?ep=1“ → „x-481“.
fn slug(href: &str) -> String {
    href.split(['?', '#']).next().unwrap_or(href).trim_end_matches('/').rsplit('/').next().unwrap_or_default().to_string()
}

async fn get(url: &str, referer: Option<&str>) -> Result<String, String> {
    let mut req = HTTP.get(url);
    if let Some(r) = referer {
        req = req.header("Referer", r);
    }
    let res = req.send().await.map_err(|e| format!("Connection error: {e}"))?;
    let status = res.status();
    let body = res.text().await.map_err(|e| e.to_string())?;
    if body.contains("<title>Just a moment") {
        return Err("Blocked by Cloudflare, try again later".into());
    }
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    Ok(body)
}

/// Die `/api/theme/…`-Aufrufe liefern `{"status":true,"html":"…"}`.
async fn api_html(url: &str) -> Result<String, String> {
    let body = get(url, None).await?;
    let j: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    j["html"].as_str().map(String::from).ok_or_else(|| "Unexpected answer from the site".into())
}

// ---------- Startseite, Suche, Anime-Seite ----------

fn parse_trending(html: &str) -> Vec<Anime> {
    let Some(start) = html.find("id=\"trending-home\"") else { return vec![] };
    let block = &html[start..];
    let block = &block[..block.find("</section>").unwrap_or(block.len())];
    block
        .split("<div class=\"item\">")
        .skip(1)
        .filter_map(|item| {
            let link = item.find("class=\"film-poster\"").map(|i| &item[..i])?;
            let href = link.rsplit("<a href=\"").next()?.split('"').next()?;
            Some(Anime {
                id: slug(href),
                title: text(between(item, "data-jname=", "</div>")?.split_once('>')?.1),
                poster: between(item, "<img src=\"", "\"").map(String::from),
                ..Default::default()
            })
        })
        .filter(|a| !a.id.is_empty())
        .collect()
}

/// „Top 10“ der Seitenleiste (Heute, Woche, Monat hintereinander).
fn parse_top(html: &str) -> Vec<Anime> {
    html.split("<li class=\"item-top\">")
        .skip(1)
        .filter_map(|item| {
            let name = between(item, "<h3 class=\"film-name\">", "</h3>")?;
            Some(Anime {
                id: slug(&attr(name, "href")?),
                title: attr(name, "title")?,
                poster: between(item, "<img src=\"", "\"").map(String::from),
                sub: number(between(item, "tick-sub\">", "</div>")),
                dub: number(between(item, "tick-dub\">", "</div>")),
                ..Default::default()
            })
        })
        .filter(|a| !a.id.is_empty())
        .collect()
}

/// Trending-Liste von der Startseite der Seite, danach als Reserve (falls Trending-Einträge auf der
/// Watchlist stehen und ausgeblendet werden) Top 10 und neueste Episoden aus derselben Seite.
pub async fn trending() -> Result<Vec<Anime>, String> {
    let html = get(&format!("{BASE}/home"), None).await?;
    let mut list = parse_trending(&html);
    if list.is_empty() {
        return Err("Couldn't read the trending list".into());
    }
    for a in parse_top(&html).into_iter().chain(parse_search(&html)) {
        if !list.iter().any(|b| b.id == a.id) {
            list.push(a);
        }
    }
    Ok(list)
}

fn parse_search(html: &str) -> Vec<Anime> {
    // Die Seitenleiste (Top 10) wiederholt das Karten-Markup: abschneiden.
    let html = &html[..html.find("id=\"main-sidebar\"").unwrap_or(html.len())];
    html.split("class=\"flw-item")
        .skip(1)
        .filter_map(|item| {
            let name = between(item, "<h3 class=\"film-name\">", "</h3>")?;
            Some(Anime {
                id: slug(&attr(name, "href")?),
                title: attr(name, "title")?,
                poster: between(item, "<img src=\"", "\"").map(String::from),
                sub: number(between(item, "tick-sub\">", "</div>")),
                dub: number(between(item, "tick-dub\">", "</div>")),
                kind: between(item, "class=\"fdi-item\">", "<").map(text).filter(|s| !s.is_empty()),
                duration: between(item, "fdi-duration\">", "<").map(text).filter(|s| !s.is_empty()),
            })
        })
        .filter(|a| !a.id.is_empty())
        .collect()
}

pub async fn search(query: String) -> Result<Vec<Anime>, String> {
    let url = reqwest::Url::parse_with_params(&format!("{BASE}/search"), &[("keyword", &query)]).map_err(|e| e.to_string())?;
    Ok(parse_search(&get(url.as_str(), None).await?))
}

/// Wert hinter „<Name>:“ im Info-Kasten der Anime-Seite.
fn info_item(html: &str, head: &str) -> Option<String> {
    let rest = &html[html.find(&format!("item-head\">{head}:</span>"))?..];
    between(rest, "class=\"name\">", "</span>").map(text).filter(|s| !s.is_empty() && s != "?")
}

fn parse_details(id: &str, html: &str) -> Option<Details> {
    let title = text(between(html, "<h2 class=\"film-name", "</h2>")?.split_once('>')?.1);
    let stats = between(html, "class=\"film-stats\">", "class=\"film-buttons\"").unwrap_or_default();
    let mut items = stats.split("<span class=\"item\">").skip(1).filter_map(|s| s.split('<').next()).map(text);
    let anime = Anime {
        id: id.to_string(),
        title,
        poster: between(html, "class=\"anisc-poster\">", "</div>").and_then(|p| between(p, "<img src=\"", "\"")).map(String::from),
        sub: number(between(stats, "tick-sub\">", "</div>")),
        dub: number(between(stats, "tick-dub\">", "</div>")),
        kind: items.next(),
        duration: items.next(),
    };
    let description = between(html, "item-head\">Overview:</span>", "</div>").map(text).unwrap_or_default();
    let mut info = Vec::new();
    for head in ["Status", "Aired", "Premiered"] {
        info.extend(info_item(html, head));
    }
    info.extend(info_item(html, "MAL Score").map(|s| format!("MAL {s}")));
    let genres = html
        .find("item-head\">Genres:</span>")
        .and_then(|i| html[i..].split_once("</div>"))
        .map(|(g, _)| g.split("title=\"").skip(1).filter_map(|t| t.split('"').next()).map(unescape).collect())
        .unwrap_or_default();
    Some(Details { anime, description, info, genres })
}

pub async fn details(id: String) -> Result<Details, String> {
    let html = get(&format!("{BASE}/{id}"), None).await?;
    parse_details(&id, &html).ok_or_else(|| "Couldn't read the anime page".into())
}

// ---------- Episoden und Stream ----------

fn parse_episodes(id: &str, html: &str) -> Vec<Episode> {
    let mut eps: Vec<Episode> = html
        .split("<a title=\"")
        .skip(1)
        // Episoden-Links müssen zu diesem Anime gehören (alte IDs haben dieselbe Form, sagt ani-cli).
        .filter(|item| item.contains(&format!("/watch/{id}?ep=")))
        .filter_map(|item| {
            Some(Episode {
                title: unescape(item.split('"').next()?),
                number: attr(item, "data-number")?.parse().ok()?,
                id: attr(item, "data-id")?,
            })
        })
        .collect();
    eps.sort_by_key(|e| e.number);
    eps.dedup_by_key(|e| e.number);
    eps
}

pub async fn episodes(id: String) -> Result<Vec<Episode>, String> {
    let num = id.rsplit('-').next().unwrap_or_default();
    let html = api_html(&format!("{BASE}/api/theme/episode/list/{num}")).await?;
    Ok(parse_episodes(&id, &html))
}

/// Einbettungs-Adresse des ZokoAnime-Servers für `mode` („sub“/„dub“). Andere Server nutzen
/// andere Player, die hier (wie bei ani-cli) nicht verstanden werden.
fn embed_url(servers_html: &str, mode: &str) -> Option<String> {
    let item = servers_html
        .split("server-item")
        .find(|s| s.contains(&format!("data-type=\"{mode}\"")) && s.contains("data-server-name=\"ZokoAnime\""))?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(attr(item, "data-hash")?).ok()?;
    String::from_utf8(bytes).ok()
}

fn deobfuscate(blob: &str) -> Option<String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(blob).ok()?;
    String::from_utf8(bytes.iter().enumerate().map(|(i, b)| b ^ EMBED_KEY[i % EMBED_KEY.len()]).collect()).ok()
}

/// Qualitätsstufen aus der Master-Playlist, beste zuerst. Relative Adressen gelten zur Playlist.
fn parse_master(master_url: &str, text: &str) -> Vec<(u32, String)> {
    let base = &master_url[..master_url.rfind('/').map_or(master_url.len(), |i| i + 1)];
    let host = master_url.splitn(4, '/').take(3).collect::<Vec<_>>().join("/");
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let Some(l) = lines.next() {
        let Some(info) = l.strip_prefix("#EXT-X-STREAM-INF:") else { continue };
        let height = info.split("RESOLUTION=").nth(1).and_then(|r| r.split(['x', ',']).nth(1)).and_then(|h| h.parse().ok()).unwrap_or(0);
        let Some(uri) = lines.next().map(str::trim) else { break };
        let url = if uri.starts_with("http") {
            uri.to_string()
        } else if uri.starts_with('/') {
            format!("{host}{uri}")
        } else {
            format!("{base}{uri}")
        };
        out.push((height, url));
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out
}

pub async fn stream(episode_id: String, mode: String) -> Result<Stream, String> {
    let servers = api_html(&format!("{BASE}/api/theme/episode/servers?episodeId={episode_id}")).await?;
    let embed = embed_url(&servers, &mode).ok_or_else(|| format!("No {mode} version of this episode"))?;
    let referer = format!("{}/", embed.splitn(4, '/').take(3).collect::<Vec<_>>().join("/"));
    let page = get(&embed, None).await?;
    let json = between(&page, "window.__P=\"", "\"").and_then(deobfuscate).ok_or("Couldn't read the video player")?;
    let j: serde_json::Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let master = j["src"].as_str().filter(|s| s.contains(".m3u8")).ok_or("No video source found")?.to_string();
    let mut subs = j["subtitles"].as_array().cloned().unwrap_or_default();
    subs.sort_by_key(|s| s["default"] != true);
    let subtitles = download_subtitles(&subs, &referer).await;
    let mut variants = parse_master(&master, &get(&master, Some(&referer)).await?);
    if variants.is_empty() {
        variants.push((0, master));
    }
    let mal_id = between(&format!("{embed}/"), "/mal/", "/").map(String::from);
    Ok(Stream { variants, referer, subtitles, mal_id })
}

/// Untertitel (`src`, `label`) nach `<tmp>/aniqu-subs/<nr>-<datei>/<Label>.<endung>` laden: so heißt die Spur in
/// mpv „English“ statt nach der Zufalls-Datei. Fehlschläge auslassen (das Video geht trotzdem).
async fn download_subtitles(subs: &[serde_json::Value], referer: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, s) in subs.iter().enumerate() {
        let Some(src) = s["src"].as_str() else { continue };
        let file = src.split(['?', '#']).next().unwrap_or(src).rsplit('/').next().unwrap_or_default();
        let (stem, ext) = file.rsplit_once('.').unwrap_or((file, "vtt"));
        let label = s["label"].as_str().or(s["lang"].as_str()).unwrap_or("Subtitles").replace(['/', ':', '\\'], " ");
        let Ok(text) = get(src, Some(referer)).await else { continue };
        let dir = std::env::temp_dir().join("aniqu-subs").join(format!("{i}-{stem}"));
        let path = dir.join(format!("{label}.{ext}"));
        if tokio::fs::create_dir_all(&dir).await.is_ok() && tokio::fs::write(&path, text).await.is_ok() {
            out.push(path.to_string_lossy().into_owned());
        }
    }
    out
}

// ---------- Poster ----------

fn cache_dir() -> PathBuf {
    qu_core::app::cache_dir()
}

/// Poster als Bytes; einmal geladen, kommt es von der Festplatte.
pub async fn poster(url: String) -> Result<Vec<u8>, String> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut h);
    let path = cache_dir().join("posters").join(format!("{:x}", h.finish()));
    if let Ok(b) = tokio::fs::read(&path).await {
        return Ok(b);
    }
    let _slot = POSTER_SLOTS.acquire().await.map_err(|e| e.to_string())?;
    let res = HTTP.get(url).send().await.and_then(|r| r.error_for_status()).map_err(|e| e.to_string())?;
    let bytes = res.bytes().await.map_err(|e| e.to_string())?.to_vec();
    // ponytail: Cache wächst unbegrenzt (~30 KB pro Poster); Aufräumen nach Alter, falls es stört.
    let _ = tokio::fs::create_dir_all(path.parent().unwrap()).await;
    let _ = tokio::fs::write(&path, &bytes).await;
    Ok(bytes)
}

// ---------- Gespeicherte Trending-Liste (sofort beim Start zeigen) ----------

fn trending_file() -> PathBuf {
    cache_dir().join("trending.tsv")
}

pub fn saved_trending() -> Vec<Anime> {
    read_list(&trending_file())
}

pub fn save_trending(list: &[Anime]) {
    write_list(&trending_file(), list);
}

/// Anime-Liste als TSV (id, Titel, Poster) – für Trending und Watchlist.
pub fn read_list(path: &std::path::Path) -> Vec<Anime> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            let (id, title, poster) = (f.next()?, f.next()?, f.next()?);
            Some(Anime { id: id.into(), title: title.into(), poster: (!poster.is_empty()).then(|| poster.into()), ..Default::default() })
        })
        .collect()
}

pub fn write_list(path: &std::path::Path, list: &[Anime]) {
    let clean = |s: &str| s.replace(['\t', '\n'], " ");
    let text: String = list
        .iter()
        .map(|a| format!("{}\t{}\t{}\n", clean(&a.id), clean(&a.title), clean(a.poster.as_deref().unwrap_or_default())))
        .collect();
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let _ = std::fs::write(path, text);
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRENDING: &str = r#"<div class="trending-list" id="trending-home"><div class="swiper-slide">
<div class="item">
    <div class="number">
        <span>01</span>
        <div class="film-title dynamic-name" data-jname="One Piece">One Piece</div>
    </div>
    <a href="https://hianime.at/one-piece-1" class="film-poster" title="One Piece" aria-label="One Piece">
        <img src="https://hianime.at/storage/media/one-piece.webp"
            class="film-poster-img" alt="One Piece">
    </a>
</div></div><div class="swiper-slide"><div class="item">
        <div class="film-title dynamic-name" data-jname="x">I&#039;m Dating</div>
    <a href="https://hianime.at/im-dating-9" class="film-poster" title="I&#039;m Dating">
        <img src="https://x/p.webp"></a></div></div></section><div class="item">nicht mehr</div>"#;

    const SEARCH: &str = r#"<div class="flw-item flw-item-big">
    <div class="film-poster">
        <div class="tick ltr">
            <div class="tick-item tick-sub"><i class="fas fa-closed-captioning mr-1"></i>28</div>
            <div class="tick-item tick-dub"><i class="fas fa-microphone mr-1"></i>12</div>
        </div>
        <img src="https://cdn.anipixcdn.co/thumbnail/bb.jpg" class="film-poster-img">
    </div>
    <div class="film-detail">
        <h3 class="film-name">
            <a href="https://hianime.at/frieren-beyond-journeys-end-481"
                title="Frieren: Beyond Journey&#039;s End" class="dynamic-name">Frieren</a>
        </h3>
        <div class="fd-infor">
            <span class="fdi-item">TV</span><span class="dot"></span>
            <span class="fdi-item fdi-duration">25m</span>
        </div>
    </div></div>
<div id="main-sidebar"><div class="flw-item"><h3 class="film-name"><a href="/top-1" title="Top">x</a></h3></div></div>"#;

    #[test]
    fn reads_trending_and_search() {
        let t = parse_trending(TRENDING);
        assert_eq!(t.len(), 2);
        assert_eq!((t[0].id.as_str(), t[0].title.as_str()), ("one-piece-1", "One Piece"));
        assert_eq!(t[0].poster.as_deref(), Some("https://hianime.at/storage/media/one-piece.webp"));
        assert_eq!(t[1].title, "I'm Dating");

        let s = parse_search(SEARCH);
        assert_eq!(s.len(), 1, "Seitenleiste wird ignoriert");
        let a = &s[0];
        assert_eq!((a.id.as_str(), a.title.as_str()), ("frieren-beyond-journeys-end-481", "Frieren: Beyond Journey's End"));
        assert_eq!((a.sub, a.dub), (Some(28), Some(12)));
        assert_eq!((a.kind.as_deref(), a.duration.as_deref()), (Some("TV"), Some("25m")));
    }

    #[test]
    fn reads_episodes_and_servers() {
        let html = r#"<a title="The Journey&#039;s End" class="ssl-item ep-item" data-number="1" data-id="9227"
            href="https://hianime.at/watch/frieren-481?ep=9227">
          <a title="Fremd" class="ep-item" data-number="1" data-id="1" href="https://hianime.at/watch/anderes-1?ep=1">
          <a title="Zwei" class="ep-item" data-number="2" data-id="9228" href="https://hianime.at/watch/frieren-481?ep=9228">"#;
        let eps = parse_episodes("frieren-481", html);
        assert_eq!(eps.len(), 2);
        assert_eq!(eps[0], Episode { id: "9227".into(), number: 1, title: "The Journey's End".into() });

        let servers = r#"<div class="item server-item" data-type="sub" data-server-name="HD-1" data-hash="eA==">
            <div class="item server-item" data-type="sub" data-server-name="ZokoAnime"
            data-hash="aHR0cHM6Ly96b2tvYW5pbWUudmlkZW8vc3RyZWFtL21hbC81Mjk5MS8xL3N1Yg==">"#;
        assert_eq!(embed_url(servers, "sub").as_deref(), Some("https://zokoanime.video/stream/mal/52991/1/sub"));
        assert_eq!(embed_url(servers, "dub"), None);

        // XOR mit dem Schlüssel ist umkehrbar: verschleiern und wieder lesen.
        let plain = br#"{"src":"x.m3u8"}"#;
        let blob: Vec<u8> = plain.iter().enumerate().map(|(i, b)| b ^ EMBED_KEY[i % EMBED_KEY.len()]).collect();
        let blob = base64::engine::general_purpose::STANDARD.encode(blob);
        assert_eq!(deobfuscate(&blob).as_deref(), Some(r#"{"src":"x.m3u8"}"#));
    }

    #[test]
    fn reads_master_playlist() {
        let m = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=900000,RESOLUTION=640x360\n360/index.m3u8\n\
                 #EXT-X-STREAM-INF:BANDWIDTH=5300000,RESOLUTION=1920x1080\nhttps://cdn/1080.m3u8\n\
                 #EXT-X-STREAM-INF:BANDWIDTH=3000000,RESOLUTION=1280x720\n/abs/720.m3u8\n";
        let v = parse_master("https://h.top/p/a/master.m3u8", m);
        assert_eq!(v[0], (1080, "https://cdn/1080.m3u8".into()));
        assert_eq!(v[1], (720, "https://h.top/abs/720.m3u8".into()));
        assert_eq!(v[2], (360, "https://h.top/p/a/360/index.m3u8".into()));
        assert_eq!(slug("https://hianime.at/watch/x-481?ep=1"), "x-481");
        assert_eq!(text("<i class=\"a\"></i>\n  28 "), "28");
    }
}
