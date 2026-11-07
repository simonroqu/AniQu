//! Kleine lokale Listen in `~/.local/share/aniqu/`: Suchverlauf, Watchlist, Geschwindigkeit und Sub/Dub pro Anime.

use crate::progress::data_file;

fn read(name: &str) -> String {
    std::fs::read_to_string(data_file(name)).unwrap_or_default()
}

fn write(name: &str, text: &str) {
    let path = data_file(name);
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let _ = std::fs::write(path, text);
}

// ---------- Suchverlauf ----------

const SEARCHES_MAX: usize = 8;

pub fn searches() -> Vec<String> {
    read("searches.txt").lines().map(String::from).collect()
}

pub fn add_search(q: &str) {
    let mut list = searches();
    list.retain(|s| s != q);
    list.insert(0, q.replace('\n', " "));
    list.truncate(SEARCHES_MAX);
    write("searches.txt", &list.join("\n"));
}

// ---------- Watchlist (neueste zuerst) ----------

pub fn watchlist() -> Vec<crate::anime::Anime> {
    crate::anime::read_list(&data_file("watchlist.tsv"))
}

pub fn on_watchlist(id: &str) -> bool {
    watchlist().iter().any(|a| a.id == id)
}

/// Aufnehmen bzw. entfernen; liefert, ob er jetzt drauf ist.
pub fn toggle_watchlist(a: &crate::anime::Anime) -> bool {
    let mut list = watchlist();
    let before = list.len();
    list.retain(|x| x.id != a.id);
    let added = list.len() == before;
    if added {
        list.insert(0, a.clone());
    }
    crate::anime::write_list(&data_file("watchlist.tsv"), &list);
    added
}

// ---------- Werte pro Anime (Tabelle „id<TAB>wert“) ----------

fn parse_table(text: &str) -> Vec<(String, String)> {
    text.lines().filter_map(|l| l.split_once('\t')).map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn lookup(file: &str, anime: &str) -> Option<String> {
    parse_table(&read(file)).into_iter().find(|(k, _)| k == anime).map(|(_, v)| v)
}

/// Eintrag setzen; `None` (= Standardwert) entfernt ihn.
fn store(file: &str, anime: &str, value: Option<String>) {
    let mut list = parse_table(&read(file));
    list.retain(|(k, _)| k != anime);
    if let Some(v) = value {
        list.push((anime.to_string(), v));
    }
    write(file, &list.iter().map(|(k, v)| format!("{k}\t{v}")).collect::<Vec<_>>().join("\n"));
}

/// Zuletzt für diesen Anime gewählte Geschwindigkeit (sonst normal).
pub fn anime_speed(anime: &str) -> f64 {
    lookup("speeds.tsv", anime).and_then(|s| s.parse().ok()).unwrap_or(1.0)
}

pub fn set_anime_speed(anime: &str, speed: f64) {
    store("speeds.tsv", anime, (speed != 1.0).then(|| speed.to_string()));
}

/// Sub oder Dub für diesen Anime (Standard: Sub).
pub fn dub(anime: &str) -> bool {
    lookup("modes.tsv", anime).is_some_and(|m| m == "dub")
}

pub fn set_dub(anime: &str, dub: bool) {
    store("modes.tsv", anime, dub.then(|| "dub".to_string()));
}

#[cfg(test)]
mod tests {
    #[test]
    fn table_parse() {
        let l = super::parse_table("a\t1.5\nkaputt\nb\tdub");
        assert_eq!(l, vec![("a".to_string(), "1.5".to_string()), ("b".to_string(), "dub".to_string())]);
    }
}
