//! Kleine lokale Listen in `~/.local/share/aniqu/`: Watchlist, Geschwindigkeit und Sub/Dub pro Anime.

use qu_core::app::data_file;
use qu_core::store::put;

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

/// Zuletzt für diesen Anime gewählte Geschwindigkeit (sonst normal).
pub fn anime_speed(anime: &str) -> f64 {
    qu_core::store::speed(anime)
}

pub fn set_anime_speed(anime: &str, speed: f64) {
    qu_core::store::set_speed(anime, speed);
}

/// Sub oder Dub für diesen Anime (Standard: Sub).
pub fn dub(anime: &str) -> bool {
    qu_core::store::lookup("modes.tsv", anime).is_some_and(|m| m == "dub")
}

pub fn set_dub(anime: &str, dub: bool) {
    put("modes.tsv", anime, dub.then(|| "dub".to_string()));
}

