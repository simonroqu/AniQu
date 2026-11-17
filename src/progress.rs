//! Merkt sich pro Episode, wie weit sie geschaut wurde (plus Titel und Poster für „Continue watching“).
//! Datei: ~/.local/share/aniqu/progress.tsv, eine Zeile pro Episode:
//! Anime-ID, Episode, Position, Dauer, Zeitstempel, Titel, Poster, Episoden gesamt.

use crate::anime::Anime;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Auf neue Folgen prüfen nur bei Animes, die in dieser Zeit geschaut wurden (laufende Staffeln).
const AIRING_WINDOW: u64 = 60 * 86400;

/// Was gerade läuft: reicht zum Speichern und für „Continue watching“.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub anime: String,
    pub episode: u32,
    pub title: String,
    pub poster: Option<String>,
    /// Anzahl verfügbarer Episoden (in der gewählten Sprache), für „nächste Episode“ in „Continue watching“.
    pub total: u32,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Entry {
    pub pos: f64,
    pub dur: f64,
    pub updated: u64,
    pub title: String,
    pub poster: String,
    pub total: u32,
}

impl Entry {
    pub fn finished(&self) -> bool {
        qu_core::progress::finished(self.pos, self.dur)
    }

    /// Anteil für den Balken (0.0 – 1.0).
    pub fn fraction(&self) -> f64 {
        qu_core::progress::fraction(self.pos, self.dur)
    }
}

/// Eine Karte in „Continue watching“: Anime, welche Episode als Nächstes, wie weit.
#[derive(Clone, Debug, PartialEq)]
pub struct Resume {
    pub anime: Anime,
    pub episode: u32,
    pub fraction: f64,
}

type Key = (String, u32);

/// Datei in `~/.local/share/aniqu/`.
pub fn data_file(name: &str) -> PathBuf {
    qu_core::app::data_file(name)
}

fn path() -> PathBuf {
    data_file("progress.tsv")
}

fn parse(s: &str) -> HashMap<Key, Entry> {
    s.lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            let key = (f.next()?.to_string(), f.next()?.parse().ok()?);
            let pos = f.next()?.parse().ok()?;
            let dur = f.next()?.parse().ok()?;
            let updated = f.next()?.parse().ok()?;
            let mut text = || f.next().unwrap_or_default().to_string();
            let (title, poster) = (text(), text());
            let total = text().parse().unwrap_or(0);
            Some((key, Entry { pos, dur, updated, title, poster, total }))
        })
        .collect()
}

/// Tabs und Zeilenumbrüche würden das Dateiformat zerbrechen.
fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

fn serialize(db: &HashMap<Key, Entry>) -> String {
    db.iter()
        .map(|((id, ep), e)| {
            format!("{id}\t{ep}\t{:.1}\t{:.1}\t{}\t{}\t{}\t{}\n", e.pos, e.dur, e.updated, clean(&e.title), clean(&e.poster), e.total)
        })
        .collect()
}

// ponytail: ganze Datei im Speicher und bei jedem Speichern komplett neu geschrieben.
// Reicht für zehntausende Episoden; erst bei mehr auf SQLite umstellen.
static DB: LazyLock<Mutex<HashMap<Key, Entry>>> =
    LazyLock::new(|| Mutex::new(parse(&std::fs::read_to_string(path()).unwrap_or_default())));

pub fn get(anime: &str, episode: u32) -> Option<Entry> {
    DB.lock().unwrap().get(&(anime.to_string(), episode)).cloned()
}

/// „Continue watching“: pro Anime die zuletzt geschaute Episode. Ist sie fertig, die nächste
/// (falls es eine gibt). Zuletzt geschaute zuerst.
pub fn continue_watching(n: usize) -> Vec<Resume> {
    resumes(&DB.lock().unwrap(), &HIDDEN.lock().unwrap(), n)
}

// ---------- „Watched“: aus „Continue watching“ genommen ----------
// Datei `hidden.tsv`: Anime-ID und Zeitpunkt. Der Anime bleibt weg (auch bei neuen Folgen und ohne
// Prüfung auf neue Folgen), bis danach wieder eine Episode geschaut wird (`record` = neuerer Zeitstempel).

type Hidden = HashMap<String, u64>;

static HIDDEN: LazyLock<Mutex<Hidden>> = LazyLock::new(|| {
    let text = std::fs::read_to_string(data_file("hidden.tsv")).unwrap_or_default();
    Mutex::new(text.lines().filter_map(|l| l.split_once('\t')).filter_map(|(id, t)| Some((id.to_string(), t.parse().ok()?))).collect())
});

/// Anime als geschaut markieren: verschwindet aus „Continue watching“.
pub fn hide(anime: &str) {
    let mut h = HIDDEN.lock().unwrap();
    h.insert(anime.to_string(), now());
    let text: String = h.iter().map(|(id, t)| format!("{id}\t{t}\n")).collect();
    let p = data_file("hidden.tsv");
    let _ = std::fs::create_dir_all(p.parent().unwrap());
    let _ = std::fs::write(p, text);
}

/// Seit dem Markieren nichts mehr geschaut?
fn is_hidden(hidden: &Hidden, id: &str, e: &Entry) -> bool {
    hidden.get(id).is_some_and(|t| e.updated <= *t)
}

fn resumes(db: &HashMap<Key, Entry>, hidden: &Hidden, n: usize) -> Vec<Resume> {
    let mut out: Vec<(u64, Resume)> = latest(db)
        .into_iter()
        .filter(|(id, (_, e))| !is_hidden(hidden, id, e))
        .filter_map(|(id, (ep, e))| {
            let (episode, fraction) = if !e.finished() {
                (ep, e.fraction())
            } else if ep < e.total {
                (ep + 1, 0.0)
            } else {
                return None;
            };
            let anime = Anime {
                id: id.to_string(),
                title: e.title.clone(),
                poster: (!e.poster.is_empty()).then(|| e.poster.clone()),
                ..Default::default()
            };
            Some((e.updated, Resume { anime, episode, fraction }))
        })
        .collect();
    out.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    out.into_iter().take(n).map(|(_, r)| r).collect()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Animes, bei denen man auf dem neuesten Stand ist (letzte bekannte Episode fertig geschaut) und die
/// in den letzten Wochen geschaut wurden: hier könnte inzwischen eine neue Folge draußen sein.
/// Zuletzt geschaute zuerst, höchstens `n`.
pub fn caught_up(n: usize) -> Vec<(String, u32)> {
    caught_up_in(&DB.lock().unwrap(), &HIDDEN.lock().unwrap(), now(), n)
}

fn latest(db: &HashMap<Key, Entry>) -> HashMap<&str, (u32, &Entry)> {
    let mut latest: HashMap<&str, (u32, &Entry)> = HashMap::new();
    for ((id, ep), e) in db {
        if latest.get(id.as_str()).is_none_or(|(_, old)| e.updated > old.updated) {
            latest.insert(id, (*ep, e));
        }
    }
    latest
}

fn caught_up_in(db: &HashMap<Key, Entry>, hidden: &Hidden, now: u64, n: usize) -> Vec<(String, u32)> {
    let mut v: Vec<(u64, String, u32)> = latest(db)
        .into_iter()
        .filter(|(id, (ep, e))| !is_hidden(hidden, id, e) && e.finished() && *ep >= e.total && now.saturating_sub(e.updated) < AIRING_WINDOW)
        .map(|(id, (_, e))| (e.updated, id.to_string(), e.total))
        .collect();
    v.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    v.into_iter().take(n).map(|(_, id, total)| (id, total)).collect()
}

/// Neue Episodenzahl eines Animes merken (neue Folge erschienen): dann zeigt „Continue watching“
/// die nächste Episode.
pub fn set_total(anime: &str, total: u32) {
    let mut db = DB.lock().unwrap();
    for ((id, _), e) in db.iter_mut() {
        if id == anime {
            e.total = total;
        }
    }
    persist(&db);
}

/// Stelle zum Fortsetzen, falls die Episode angefangen und nicht fertig ist.
pub fn resume_at(anime: &str, episode: u32) -> Option<f64> {
    let e = get(anime, episode)?;
    qu_core::progress::resume_from(e.pos, e.dur)
}

/// Position speichern (unter 30 s geschaut: nichts tun).
pub fn record(m: &Meta, pos: f64, dur: f64) {
    if !qu_core::progress::worth_saving(pos, dur) {
        return;
    }
    let mut db = DB.lock().unwrap();
    let entry = Entry { pos, dur, updated: now(), title: m.title.clone(), poster: m.poster.clone().unwrap_or_default(), total: m.total };
    db.insert((m.anime.clone(), m.episode), entry);
    persist(&db);
}

fn persist(db: &HashMap<Key, Entry>) {
    // Erst in eine Hilfsdatei schreiben, dann umbenennen: ein Absturz mittendrin zerstört nichts.
    let p = path();
    let tmp = p.with_extension("tsv.tmp");
    let ok = std::fs::create_dir_all(p.parent().unwrap())
        .and_then(|_| std::fs::write(&tmp, serialize(db)))
        .and_then(|_| std::fs::rename(&tmp, &p));
    if let Err(e) = ok {
        eprintln!("Fortschritt nicht gespeichert: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() {
        qu_core::config::load(include_str!("../qu.toml"));
    }

    fn e(pos: f64, updated: u64, total: u32) -> Entry {
        Entry { pos, dur: 1400.0, updated, title: "T\tmit Tab".into(), poster: "p".into(), total }
    }

    #[test]
    fn rules_and_roundtrip() {
        cfg();
        assert!(!e(700.0, 1, 0).finished() && e(1300.0, 1, 0).finished());
        assert_eq!(e(700.0, 1, 0).fraction(), 0.5);

        let db: HashMap<Key, Entry> = [(("a".into(), 3), e(700.0, 1, 12))].into();
        let back = parse(&serialize(&db));
        assert_eq!(back[&("a".to_string(), 3)].title, "T mit Tab");
        assert_eq!(back[&("a".to_string(), 3)].total, 12);
        assert!(parse("kaputt\nx\t1\n").is_empty());
    }

    #[test]
    fn continue_watching_picks_next() {
        cfg();
        let db: HashMap<Key, Entry> = [
            // a: Episode 2 fertig (neuer als Episode 1) -> weiter mit 3.
            (("a".into(), 1), e(700.0, 1, 12)),
            (("a".into(), 2), e(1350.0, 5, 12)),
            // b: mitten in Episode 4, zuletzt geschaut -> zuerst.
            (("b".into(), 4), e(700.0, 9, 0)),
            // c: letzte Episode fertig -> nicht mehr anzeigen.
            (("c".into(), 12), e(1350.0, 7, 12)),
        ]
        .into();
        let r = resumes(&db, &Hidden::new(), 10);
        assert_eq!(r.iter().map(|r| (r.anime.id.as_str(), r.episode)).collect::<Vec<_>>(), vec![("b", 4), ("a", 3)]);
        assert_eq!(r[0].fraction, 0.5);
        assert_eq!(r[1].fraction, 0.0);
    }

    #[test]
    fn caught_up_only_recent_and_finished() {
        cfg();
        let day = 86400;
        let now = 100 * day;
        let db: HashMap<Key, Entry> = [
            // a: letzte Folge (12 von 12) fertig, vor 3 Tagen -> prüfen.
            (("a".into(), 12), e(1350.0, now - 3 * day, 12)),
            // b: mitten in Folge 4 -> nicht (steht ohnehin in „Continue watching“).
            (("b".into(), 4), e(700.0, now - day, 12)),
            // c: auf dem Stand, aber vor 90 Tagen -> nicht mehr prüfen.
            (("c".into(), 5), e(1350.0, now - 90 * day, 5)),
            // d: Folge 3 von 12 fertig -> nicht (nächste Folge gibt es schon).
            (("d".into(), 3), e(1350.0, now - day, 12)),
        ]
        .into();
        assert_eq!(caught_up_in(&db, &Hidden::new(), now, 10), vec![("a".to_string(), 12)]);
        // Als „Watched“ markiert: keine Prüfung auf neue Folgen mehr.
        let hidden: Hidden = [("a".to_string(), now)].into();
        assert!(caught_up_in(&db, &hidden, now, 10).is_empty());
        // Neue Folge erschienen: „Continue watching“ zeigt Folge 13.
        let mut db = db;
        db.get_mut(&("a".to_string(), 12)).unwrap().total = 13;
        assert!(resumes(&db, &Hidden::new(), 10).iter().any(|r| r.anime.id == "a" && r.episode == 13));
        // Markiert: bleibt trotz neuer Folge weg, bis wieder geschaut wird.
        assert!(!resumes(&db, &hidden, 10).iter().any(|r| r.anime.id == "a"));
        db.insert(("a".to_string(), 13), e(300.0, now + 1, 13));
        assert!(resumes(&db, &hidden, 10).iter().any(|r| r.anime.id == "a" && r.episode == 13));
    }
}
