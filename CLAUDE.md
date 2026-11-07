# AniQu – Anime-Player für Linux (Rust + GTK4 + libmpv)

Schwester-App von Tubequ (`~/Work/tubequ`): gleicher Aufbau und Style, aber **unabhängig** (eigenes Repo,
kein geteilter Code). UI-Verbesserungen aus Tubequ bei Bedarf von Hand übernehmen (Code ist oft 1:1
übertragbar, z. B. `Nav`, Nav-Bubble, Miniplayer). App-ID `dev.roqu.AniQu`.

**Code-Kommentare: Deutsch. Alle Texte in der Oberfläche: Englisch.**

**Diese Datei bei jeder Änderung am Projekt mitpflegen** (gleicher Schritt wie die Code-Änderung), damit eine
neue Session sofort Bescheid weiß. Nur Entwicklungsrelevantes, keine persönlichen Bemerkungen.

## Bauen, starten, installieren

```bash
cargo build && cargo test            # Unit-Tests, ohne Netzwerk
ANIQU_DEV=1 cargo run                # eigene Instanz neben der installierten
cargo run -- frieren                 # direkt suchen
cargo run -- <hianime-link>          # Anime-Seite; mit ?ep=<id> direkt die Episode
./install.sh                         # ~/.local/bin/aniqu + Startmenü-Eintrag + Icon
```

Eine laufende installierte Instanz bekommt das neue Binary erst nach einem Neustart.

## Aufbau (`src/`)

| Datei | Aufgabe |
|---|---|
| `main.rs` | Fenster, Navigation (`Nav`: `browse`/`show`/`watch`, Verlauf zurück/vor), Nav-Bubble, Tasten, Maus-Seitentasten, Startargumente, `play`/`resume`/Links |
| `anime.rs` | **Einzige Stelle, die mit hianime.at spricht** (Trending, Suche, Anime-Seite, Episoden, Stream, Poster) |
| `aniskip.rs` | Opening/Ending-Zeiten (api.aniskip.com, MAL-ID aus der Einbettungs-Adresse) |
| `grid.rs` | Poster-Raster: Startseite („Continue watching“ oben, dann Watchlist, darunter Trending ohne Watchlist-Animes), Rechtsklick-Menü der Karten (`card_menu`: Watchlist an/aus, bei „Continue watching“ „Watched“), Suche; Check auf neue Folgen; Strg+Mausrad = Spalten 4–12 (hoch = mehr, gespeichert in `~/.config/aniqu/columns`, nur so viele, wie Karten ≥ 120 px passen) |
| `show.rs` | Anime-Seite: Poster, Infos, Beschreibung, Sub/Dub, Lesezeichen-Knopf (Watchlist), Episoden-Kacheln; `Play` (Episode + Liste) |
| `watch.rs` | Ansehen-Seite: Player, Titel (→ Anime-Seite), Episode, vorige/nächste; Strg+Mausrad = Player-Breite (gespeichert in `~/.config/aniqu/player-width`) |
| `player.rs` | libmpv in `GtkGLArea`, Fortschritt, „Skip intro“-Knopf (ani-skip), Qualitätsstufe wählen |
| `controls.rs` | Bubble-Bedienleiste (wie Tubequ), Qualität ruft `connect_quality` |
| `progress.rs` | Fortschritt pro Episode, „Continue watching“ (fertig → nächste Episode), `caught_up`/`set_total`; „Watched“ = `hide` (`hidden.tsv`: ID + Zeitpunkt): Anime bleibt aus „Continue watching“ und ohne Neue-Folgen-Prüfung, bis danach wieder eine Episode gespeichert wird (neuerer Zeitstempel) |
| `lists.rs` | Suchverlauf, Watchlist (`~/.local/share/aniqu/watchlist.tsv`, Format wie `trending.tsv`: `anime::read_list`/`write_list`), Tempo und Sub/Dub pro Anime |
| `search.rs`, `theme.rs`, `icons.rs`, `nav_shape.rs`, `smooth.rs`, `style.css` | wie in Tubequ |

## Datenquelle hianime.at (Stand 2026-11, Ablauf wie ani-cli 5.1.0)

- Trending: `/home`, Block `id="trending-home"` (12 Stück); Reserve aus derselben Seite: Top 10 (`li.item-top`) und die `flw-item`-Listen, damit Trending ohne Watchlist-Animes immer ganze Reihen füllt (`grid.rs` `trending_count`: 12 auf volle Reihen aufgerundet). Suche: `/search?keyword=` (Seitenleiste ab `id="main-sidebar"` abschneiden).
- Anime-Seite: `/<id>` (id = Slug mit `-<Nummer>` am Ende). Episoden: `/api/theme/episode/list/<Nummer>` (JSON mit `html`).
- Stream: `/api/theme/episode/servers?episodeId=` → Server „ZokoAnime“, `data-hash` = base64-Adresse
  `zokoanime.video/stream/mal/<mal>/<ep>/<sub|dub>` → `window.__P` = base64, XOR mit `otaku-embed-v1` → JSON
  (`src` = master.m3u8, `subtitles`). Master-Playlist mit Referer der Einbettung holen → Stufen 360/720/1080.
- reqwest mit Browser-User-Agent kommt durch Cloudflare (getestet 2026-11-07). Bei „Just a moment“ → Fehlermeldung.
- mpv: `referrer` + `user-agent`, Untertitel per **`sub-file`** (nicht `sub-files`: Pfadliste, zerlegt URLs am „:“).
  Werte in `loadfile`-Optionen als `%Länge%Wert`.
- Kein Anfragen-Limit wie bei Tubequ nötig, aber sparsam bleiben (kein Vorladen).

## Tests – Regeln

- Standard-Prüfung: `cargo build && cargo test`.
- Live-Tests gegen hianime nur gezielt, wenige Anfragen. Rohseiten zum Entwickeln einmal speichern und offline parsen.
- **Der Nutzer hat die installierte App meist parallel offen** (gleiche Fensterklasse `dev.roqu.AniQu`!).
  Fenster daher nur über die **PID des selbst gestarteten Prozesses** (`$!`) finden, nie über die Klasse;
  `kill` nur mit dieser PID. (Ein Klassen-Filter hat schon einmal die falsche Instanz fotografiert und beendet.)
- Dev-Instanz mit eigenen Daten starten: `XDG_DATA_HOME`/`XDG_CONFIG_HOME` ins Scratchpad. Sonst überschreiben
  sich beide Instanzen `progress.tsv` gegenseitig (jede schreibt ihre Liste aus dem Speicher komplett neu).
- Sichtprüfung: App mit `ANIQU_DEV=1`, Fenster per PID über `hyprctl clients -j`. **Vor `grim` prüfen, dass der
  Workspace des Fensters gerade sichtbar ist** (`hyprctl monitors -j` → `activeWorkspace`), sonst landet ein
  fremder Bildschirminhalt im Screenshot.
- Abspiel-Tests stumm: `volume` (im Test-`XDG_CONFIG_HOME`) auf 0. Achtung: das Überspringen des Openings
  schiebt die Position sofort über 30 s, d. h. schon kurze Tests speichern Fortschritt.
- **Nie** `wtype`/Eingaben simulieren, **nie** `pkill mpv` oder breite `pkill -f`-Muster; nur `kill <pid>` der Dev-Instanz.
- Temporäre Test-Hacks danach **nachweislich** zurücksetzen (grep).

## Design-Regeln

Wie Tubequ (siehe `~/Work/tubequ/CLAUDE.md`): keine Kopfzeile, Nav-Bubble oben links, Buttons nur Icons mit Tooltip,
Spinner = `icons::spinner()` (nie `gtk::Spinner`), keine knalligen Farben, keine festen Pixelbreiten im Raster.
Poster im Hochformat 2:3, Karten ab 170 px, höchstens 12 Spalten (im Vollbild sonst riesige Poster).

## Produktentscheidungen (so gewollt – nicht ohne Rückfrage ändern)

- Startseite: **„Continue watching“ ganz oben**, darunter alle Trending (so viele, wie die Seite liefert).
- Laufende Animes: Ist man auf dem Stand (letzte Folge ≥ 90 % geschaut, in den letzten 60 Tagen), prüft die
  Startseite höchstens alle 10 min bis zu 10 solcher Animes auf neue Folgen (Anime-Seite, Sub- bzw. Dub-Zahl je
  nach gewählter Sprache). Neue Folge → erscheint in „Continue watching“; auf dem Stand → nichts.
- Standard Sub, Umschalter auf der Anime-Seite (pro Anime gemerkt).
- Opening/Ending **nicht** automatisch überspringen: Knopf „Skip intro“/„Skip ending“ rechts direkt über der
  Timeline (Bubble-Style), sichtbar solange man im Abschnitt ist, auch bei ausgeblendeter Leiste.
- Fortschritt: < 30 s nicht speichern, ≥ 90 % = fertig (Ending läuft oft noch), beim Fortsetzen 3 s zurück.

## Bedienung (Komfort)

- Player-Leiste: weg nach 2 s ohne Mausbewegung (`HIDE_AFTER`). Bewegung zählt nur bei geänderter Position (`player.rs`): GTK schickt nach Layout-Änderungen Bewegungs-Ereignisse an derselben Stelle nach.
- Maus-Seitentasten (GTK-Knopf 8/9) = Zurück/Vor, überall im Fenster (`Nav::forward`, merkt nur die Seite,
  nicht deren Inhalt; neuer Weg oder Home leert „Vor“).
- Weiches Scrollen mit dem Mausrad (`smooth.rs`, an jedem ScrolledWindow): Raststufen gleiten statt zu springen; Touchpad bleibt bei GTK.
- Feinschliff (style.css, Ende): Vorschaubilder mit grauem Platzhalter (`.thumb-box`), Einblenden beim Laden (`.fade` → `.loaded`), Hover-Zoom 4 %; Ansehen-Seite blendet ein (`page-in-a/-b` im Wechsel, sonst startet die Animation nicht neu), Miniplayer gleitet in die Ecke (`.mini`).
- Ton: Stummschalten blendet über den Filter `@fade` (lavfi `volume`, per `af-command`) in 160 ms aus/ein, `+`/`-` ändern die Lautstärke in kleinen Schritten (`Controls::toggle_mute`/`step_volume`). `af-command` meldet trotzdem einen Fehler (Ziel „all“ trifft auch Filter ohne den Befehl), wirkt aber.
- Tastatur im Raster: Pfeiltasten wählen die erste Karte, danach navigiert GTK; die Liste gleitet zur gewählten Karte (`smooth.rs`, nur bei Tastatur-Fokus). Timeline braucht nichts: mpv meldet `time-pos` jedes Bild.
- Miniplayer: Verlässt man die Ansehen-Seite, während ein Video läuft, spielt es unten rechts klein weiter (400 px, Knöpfe Pause/Ton/Zurück zum Video/Schließen). Die Ansehen-Seite liegt dafür als Ebene über dem Stack (im Stack nur ein Platzhalter „watch“) und wird nur verkleinert (`WatchPage::set_mini`, `Nav::set_mini`): der Player wird **nicht umgehängt**, sonst baut GTK die GLArea neu und mpv schaltet das Video ab. Am Ende/bei Fehler stoppt das Video (`Player::active`).
