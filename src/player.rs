//! Player-Anbindung: der Player aus qu-core mit AniQu-Daten – Fortschritt pro Episode, AniSkip-Abschnitte
//! (Knopf „Skip intro“), Tempo pro Anime, Stream mit Referer und Untertitel-Dateien.

use crate::anime::Stream;
use crate::progress::{self, Meta};
use qu_core::player::{Hooks, Media, quoted};
use qu_core::segments::Segment;
use std::future::Future;
use std::pin::Pin;

/// Abgespielt wird eine Episode, dazu die MAL-ID für AniSkip (steckt im Stream).
pub type Item = (Meta, Option<String>);
pub type Player = qu_core::player::Player<Item>;

struct AniHooks;

impl Hooks<Item> for AniHooks {
    fn init(&self, i: &libmpv2::MpvInitializer) -> libmpv2::Result<()> {
        i.set_property("user-agent", crate::anime::USER_AGENT)
    }

    fn save(&self, (m, _): &Item, pos: f64, dur: f64) {
        progress::record(m, pos, dur);
    }

    fn segments(&self, (m, mal): &Item) -> Option<Pin<Box<dyn Future<Output = Vec<Segment>> + Send>>> {
        Some(Box::pin(crate::aniskip::segments(mal.clone()?, m.episode)))
    }

    fn speed_changed(&self, (m, _): &Item, speed: f64) {
        crate::lists::set_anime_speed(&m.anime, speed);
    }
}

pub fn new() -> Player {
    Player::new(AniHooks)
}

/// Episode abspielen: fortsetzen, wo man aufgehört hat; Tempo, das zuletzt für diesen Anime galt.
pub fn media(meta: Meta, stream: Stream) -> Media<Item> {
    let mut opts = vec![format!("referrer={}", quoted(&stream.referer))];
    if !stream.subtitles.is_empty() {
        // Lokale Dateien (`anime::stream` lädt sie vorher): `sub-files` ist eine Pfadliste, Trenner wie bei PATH
        // („:“, unter Windows „;“, dort steckt „:“ ja in „C:\“).
        let sep = if cfg!(windows) { ";" } else { ":" };
        opts.push(format!("sub-files={}", quoted(&stream.subtitles.join(sep))));
    }
    Media {
        start: progress::resume_at(&meta.anime, meta.episode),
        speed: crate::lists::anime_speed(&meta.anime),
        item: Some((meta, stream.mal_id)),
        variants: stream.variants,
        opts,
        live: false,
        poster: false,
    }
}
