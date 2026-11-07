//! Eigener minimalistischer Icon-Satz (data/icons). Die SVGs stecken im Programm und werden beim
//! Start in den Cache geschrieben und GTK bekannt gemacht. Endung `-symbolic`: GTK färbt sie in
//! der aktuellen Schriftfarbe ein, sie passen sich also dem Theme an.

use gtk4 as gtk;
use std::path::PathBuf;

const ICONS: [(&str, &str); 17] = [
    ("aniqu-search-symbolic", include_str!("../data/icons/aniqu-search-symbolic.svg")),
    ("aniqu-home-symbolic", include_str!("../data/icons/aniqu-home-symbolic.svg")),
    ("aniqu-back-symbolic", include_str!("../data/icons/aniqu-back-symbolic.svg")),
    ("aniqu-next-symbolic", include_str!("../data/icons/aniqu-next-symbolic.svg")),
    ("aniqu-play-symbolic", include_str!("../data/icons/aniqu-play-symbolic.svg")),
    ("aniqu-pause-symbolic", include_str!("../data/icons/aniqu-pause-symbolic.svg")),
    ("aniqu-volume-symbolic", include_str!("../data/icons/aniqu-volume-symbolic.svg")),
    ("aniqu-mute-symbolic", include_str!("../data/icons/aniqu-mute-symbolic.svg")),
    ("aniqu-settings-symbolic", include_str!("../data/icons/aniqu-settings-symbolic.svg")),
    ("aniqu-fullscreen-symbolic", include_str!("../data/icons/aniqu-fullscreen-symbolic.svg")),
    ("aniqu-restore-symbolic", include_str!("../data/icons/aniqu-restore-symbolic.svg")),
    ("aniqu-reload-symbolic", include_str!("../data/icons/aniqu-reload-symbolic.svg")),
    ("aniqu-subtitles-symbolic", include_str!("../data/icons/aniqu-subtitles-symbolic.svg")),
    ("aniqu-spinner-symbolic", include_str!("../data/icons/aniqu-spinner-symbolic.svg")),
    ("aniqu-close-symbolic", include_str!("../data/icons/aniqu-close-symbolic.svg")),
    ("aniqu-watchlist-symbolic", include_str!("../data/icons/aniqu-watchlist-symbolic.svg")),
    ("aniqu-watchlisted-symbolic", include_str!("../data/icons/aniqu-watchlisted-symbolic.svg")),
];

/// Lade-Kreis: eigenes Icon, per CSS gedreht (`.aniqu-spinner` in style.css), sichtbar = läuft.
/// Statt `gtk::Spinner`: der friert in GTK 4.24 ein, sobald er einmal ausgeblendet war
/// (auch durch Seitenwechsel), und dreht sich nach dem Wiedereinblenden nicht mehr.
pub fn spinner(size: i32) -> gtk::builders::ImageBuilder {
    gtk::Image::builder().icon_name("aniqu-spinner-symbolic").pixel_size(size).css_classes(["aniqu-spinner"])
}

pub fn install(display: &gtk::gdk::Display) {
    // Lose Dateien direkt im Suchpfad findet GTK ohne Theme-Struktur.
    let dir = PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cache/aniqu/icons");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    for (name, svg) in ICONS {
        let _ = std::fs::write(dir.join(format!("{name}.svg")), svg);
    }
    gtk::IconTheme::for_display(display).add_search_path(dir);
}
