//! ani-skip: von der Community markierte Openings und Endings automatisch überspringen.
//! Fragt api.aniskip.com (MyAnimeList-ID + Episodennummer), nicht hianime.

use std::sync::LazyLock;

#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    /// Anzeigename, z. B. „opening“.
    pub label: &'static str,
}

static HTTP: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

/// Abschnitte zum Überspringen. Leer, wenn es keine gibt oder ani-skip nicht erreichbar ist.
pub async fn segments(mal_id: String, episode: u32) -> Vec<Segment> {
    let url = format!("https://api.aniskip.com/v2/skip-times/{mal_id}/{episode}?types[]=op&types[]=ed&episodeLength=0");
    let Ok(res) = HTTP.get(url).send().await else { return vec![] };
    let Ok(text) = res.text().await else { return vec![] };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { return vec![] };
    parse(&json)
}

fn parse(json: &serde_json::Value) -> Vec<Segment> {
    let mut out: Vec<Segment> = json["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let label = match r["skipType"].as_str()? {
                "op" => "opening",
                "ed" => "ending",
                _ => return None,
            };
            let start = r["interval"]["startTime"].as_f64()?;
            let end = r["interval"]["endTime"].as_f64()?;
            (end > start).then_some(Segment { start, end, label })
        })
        .collect();
    out.sort_by(|a, b| a.start.total_cmp(&b.start));
    out
}

/// Liegt `pos` in einem noch nicht übersprungenen Abschnitt? Dann dessen Index.
/// Kurz vor dem Ende nicht mehr springen (sonst Sprung um Bruchteile einer Sekunde).
pub fn to_skip(segments: &[Segment], skipped: &[usize], pos: f64) -> Option<usize> {
    segments.iter().enumerate().position(|(i, s)| !skipped.contains(&i) && pos >= s.start && pos < s.end - 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_decides() {
        let json = serde_json::json!({"found": true, "results": [
            {"interval": {"startTime": 1417.1, "endTime": 1507.1}, "skipType": "ed"},
            {"interval": {"startTime": 3.2, "endTime": 93.2}, "skipType": "op"},
            {"interval": {"startTime": 5.0, "endTime": 9.0}, "skipType": "recap"}
        ]});
        let s = parse(&json);
        assert_eq!(s.len(), 2, "nur Opening und Ending");
        assert_eq!((s[0].start, s[0].label), (3.2, "opening"));
        assert_eq!(to_skip(&s, &[], 10.0), Some(0));
        assert_eq!(to_skip(&s, &[0], 10.0), None, "schon übersprungen: nicht nochmal");
        assert_eq!(to_skip(&s, &[], 93.0), None, "kurz vor Ende nicht mehr springen");
        assert!(parse(&serde_json::json!({"found": false})).is_empty());
    }
}
