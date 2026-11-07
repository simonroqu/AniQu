//! Merkt sich pro Episode, wie weit sie geschaut wurde (plus Titel und Poster für „Continue watching“).
//! Datei: ~/.local/share/aniqu/progress.tsv, eine Zeile pro Episode:
//! Anime-ID, Episode, Position, Dauer, Zeitstempel, Titel, Poster, Episoden gesamt.

use crate::anime::Anime;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Kürzer geschaut zählt als „nur reingeklickt“ und wird nicht gespeichert.
const MIN_WATCHED: f64 = 30.0;
/// Ab diesem Anteil gilt eine Episode als fertig (Ending läuft meist noch) und startet beim nächsten Mal von vorne.
const FINISHED_AT: f64 = 0.9;
/// Beim Fortsetzen ein paar Sekunden zurückspringen, um wieder reinzukommen.
const REWIND: f64 = 3.0;

/// Was gerade läuft: reicht zum Speichern und für „Continue watching“.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub anime: String,
    pub episode: u32,
    pub title: String,
    pub poster: Option<String>,
    /// Anzahl Episoden (für „nächste Episode“ in „Continue watching“).
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
        self.dur > 0.0 && self.pos >= self.dur * FINISHED_AT
    }

    /// Anteil für den Balken (0.0 – 1.0).
    pub fn fraction(&self) -> f64 {
        if self.finished() { 1.0 } else { (self.pos / self.dur).clamp(0.0, 1.0) }
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
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/share"));
    base.join("aniqu").join(name)
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
    resumes(&DB.lock().unwrap(), n)
}

fn resumes(db: &HashMap<Key, Entry>, n: usize) -> Vec<Resume> {
    let mut latest: HashMap<&str, (u32, &Entry)> = HashMap::new();
    for ((id, ep), e) in db {
        if latest.get(id.as_str()).is_none_or(|(_, old)| e.updated > old.updated) {
            latest.insert(id, (*ep, e));
        }
    }
    let mut out: Vec<(u64, Resume)> = latest
        .into_iter()
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

/// Stelle zum Fortsetzen, falls die Episode angefangen und nicht fertig ist.
pub fn resume_at(anime: &str, episode: u32) -> Option<f64> {
    let e = get(anime, episode)?;
    (!e.finished()).then(|| (e.pos - REWIND).max(0.0))
}

/// Position speichern (unter 30 s geschaut: nichts tun).
pub fn record(m: &Meta, pos: f64, dur: f64) {
    if dur <= 0.0 || pos < MIN_WATCHED {
        return;
    }
    let updated = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let mut db = DB.lock().unwrap();
    let entry = Entry { pos, dur, updated, title: m.title.clone(), poster: m.poster.clone().unwrap_or_default(), total: m.total };
    db.insert((m.anime.clone(), m.episode), entry);
    // Erst in eine Hilfsdatei schreiben, dann umbenennen: ein Absturz mittendrin zerstört nichts.
    let p = path();
    let tmp = p.with_extension("tsv.tmp");
    let ok = std::fs::create_dir_all(p.parent().unwrap())
        .and_then(|_| std::fs::write(&tmp, serialize(&db)))
        .and_then(|_| std::fs::rename(&tmp, &p));
    if let Err(e) = ok {
        eprintln!("Fortschritt nicht gespeichert: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(pos: f64, updated: u64, total: u32) -> Entry {
        Entry { pos, dur: 1400.0, updated, title: "T\tmit Tab".into(), poster: "p".into(), total }
    }

    #[test]
    fn rules_and_roundtrip() {
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
        let r = resumes(&db, 10);
        assert_eq!(r.iter().map(|r| (r.anime.id.as_str(), r.episode)).collect::<Vec<_>>(), vec![("b", 4), ("a", 3)]);
        assert_eq!(r[0].fraction, 0.5);
        assert_eq!(r[1].fraction, 0.0);
    }
}
