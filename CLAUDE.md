# AniQu – Anime-Player für Linux (Rust + GTK4 + libmpv)

Schwester-App von TubeQu (`~/Work/tubequ`) und BoxQu: gleicher Aufbau und Style, eigenes Repo. Den gemeinsamen
Unterbau (Theme, CSS, Icons, Animationen, Suche, Upscaling) teilen alle drei über **qu-core** (`~/Work/quCore`).
Player/Bedienleiste/Ansehen-Seite sind noch je App eigen: Verbesserungen dort bei Bedarf von Hand übernehmen (Code ist oft 1:1
übertragbar, z. B. `Nav`, Nav-Bubble, Miniplayer). App-ID `dev.roqu.AniQu`.

**Code-Kommentare: Deutsch. Alle Texte in der Oberfläche: Englisch.**

**Diese Datei bei jeder Änderung am Projekt mitpflegen** (gleicher Schritt wie die Code-Änderung), damit eine
neue Session sofort Bescheid weiß. Nur Entwicklungsrelevantes, keine persönlichen Bemerkungen.

**Zwei Oberflächen, eine Logik:** Desktop (GTK, `src/`) und Android/TV (Compose, `android/`). Jede Änderung am Aussehen
oder an der Bedienung **immer in beiden** umsetzen (gleicher Schritt), Android danach per Fernbedienung prüfen
(`adb shell input keyevent DPAD_*`, siehe Abschnitt Android). Logik (was angezeigt wird, Reihenfolge, Regeln) gehört nach
`core/` und gilt dann für beide; neue Funktionen für Android in `ffi/` exportieren. Geht etwas nur auf einer Seite
(z. B. Miniplayer, Upscaling), kurz beim Nutzer nachfragen.

## Bauen, starten, installieren

```bash
cargo build && cargo test            # Unit-Tests, ohne Netzwerk
ANIQU_DEV=1 cargo run                # eigene Instanz neben der installierten
cargo run -- frieren                 # direkt suchen
cargo run -- <hianime-link>          # Anime-Seite; mit ?ep=<id> direkt die Episode
./install.sh                         # ~/.local/bin/aniqu + Startmenü-Eintrag + Icon
```

Eine laufende installierte Instanz bekommt das neue Binary erst nach einem Neustart.

## Windows (ungetestet auf echtem Windows, Stand 2026-11-17)

Nur über die Installationsskripte, kein GitHub-Build/Release. Gebaut wird unter MSYS2 (UCRT64, GTK4 + libmpv aus MSYS2),
gebündelt als `bin/aniqu.exe` + DLLs, `lib/gdk-pixbuf-2.0`, `share/` mit Schemas und Icons. Windows-Anpassungen: `main.rs`
`windows_subsystem` (kein Konsolenfenster), `sub-files`-Trenner in `player.rs`, Pfade und GL-Loader in qu-core.
Installieren: `install.ps1` (MSYS2 per winget, dann `windows-bundle.sh`: Pakete, Bauen, Bündeln nach `target/<Titel>/`) → `%LOCALAPPDATA%\Programs\<Titel>` + Startmenü-Verknüpfung. `install.sh` = Linux (pacman/apt/dnf/zypper)
und macOS (Homebrew, `--install` legt `~/Applications/<Titel>.app` an). Skripte in allen drei Apps gleich (Name aus `qu.toml`),
App-Extras in `extra-deps.sh`/`.ps1` (nur TubeQu).

## Android (`android/` + `ffi/`, Stand 2026-11-18, in Waydroid getestet: Trending, Link, Anime-Seite, Abspielen, Skip, Untertitel, Continue watching)

- `ffi/` = Crate `aniqu-ffi`: reicht `aniqu-core` per **UniFFI** an Kotlin weiter (eigene `Record`-Typen, Netz über
  `qu_core::bg`, `init(filesDir, cacheDir)` setzt Ordner, qu.toml und `TMPDIR` für die Untertitel). Neue Funktionen für
  Android hier exportieren, Logik selbst bleibt in `core/`.
- `android/` = Gradle-Projekt (Kotlin, Compose, Media3, Coil), **ohne Android Studio**: `cd android && ./gradlew assembleDebug`
  → `app/build/outputs/apk/debug/app-debug.apk`. `preBuild` ruft qu-cores `android/build-rust.sh aniqu-ffi` (cargo-ndk →
  `jniLibs/`, Kotlin-Anbindung → `java/uniffi/`, beides nicht im Repo). **Bausteine (Bubble, Suche, Raster, Kacheln, Player,
  Theme, Icons) kommen aus qu-core `android/`** (siehe `~/Work/quCore/CLAUDE.md`), hier nur die Seiten (`MainActivity.kt`).
- Werkzeuge ohne root unter `~/Android`: `Sdk/` (cmdline-tools, NDK, platform 37.0), `rust/` (eigenes rustup mit
  Android-Zielen; System-Rust aus pacman hat keine), `gradle-9.9.1/`. cargo-ndk in `~/.cargo/bin`. JDK = System (27).
- **Aussehen wie die Desktop-App** (so gewollt): Farben aus qu-core `DEFAULT_COLORS` („Last Horizon“), schwebende Glas-Bubble
  oben links (Zurück/Home/Suche wie `[nav] top`), Suche als Spotlight-Karte mit Suchverlauf (`searches.txt`, über
  `qu_core::store`), fette Überschriften/Titel, Episoden als Kacheln wie `.episode`, Sub/Dub als Tabs, Lesezeichen-Icon.
  Fernbedienung: Runter aus der Bubble springt per `content.requestFocus(FocusDirection.Down)` in die Seite (findet Compose
  allein nicht); im Suchfeld führt Runter zu den letzten Suchen.
- `MainActivity.kt`: Seiten Home (Continue watching, Watchlist, Trending), Suchergebnisse, Anime-Seite (Sub/Dub, Watchlist,
  Episoden), Player (ExoPlayer, HLS mit Referer/User-Agent, VTT-Untertitel als Spuren, Fortsetzen, Fortschritt alle 5 s,
  „Skip intro/ending“, nächste Episode am Ende, Querformat/Vollbild). hianime.at-Links öffnen die App.
- Testen in Waydroid (x86_64): `waydroid app install app/build/outputs/apk/debug/app-debug.apk`. Steuern nur im Container
  per adb (`~/Android/Sdk/platform-tools/adb connect <IP aus waydroid status>:5555`, dann `-s <IP>:5555 shell input tap/keyevent`,
  `am start -a android.intent.action.VIEW -d <hianime-Link> dev.roqu.aniqu`, Bild per `exec-out screencap -p`), nie Eingaben am
  Desktop. Vorher stumm: `shell cmd media_session volume --stream 3 --set 0`. `run-as` geht in Waydroid nicht.
  Kein Netz in Waydroid = ufw (Omarchy): `ufw allow in on waydroid0` + `ufw route allow in/out on waydroid0`.
- **Android TV (wichtig, so gewollt):** `LEANBACK_LAUNCHER` + Banner (`res/drawable-xhdpi/banner.png`, aus dem App-Icon),
  Touchscreen nicht nötig. **Alles muss per Fernbedienung (D-Pad) erreichbar sein**: jedes klickbare Element bekommt
  `Modifier.tvFocus()` vor `clickable` (weißer Rahmen, Karten wachsen 5 %, breite Zeilen mit `grow = 1f`). Suche ist erst
  ein Knopf, OK macht das Textfeld (sonst öffnet schon das Durchnavigieren die Tastatur); Zurück schließt Suche/Ergebnisse.
  Player: Fokus auf `PlayerView` (Taste blendet die Leiste ein), „Skip …“ holt sich den Fokus, Timeline-Schritt 10 s,
  „Weiter“-Knopf = nächste Episode (`ForwardingPlayer`). Neue Knöpfe immer mit `adb shell input keyevent DPAD_*` prüfen.
- **APK für Nutzer: `build-apk.sh`** (Linux/macOS; in allen drei Apps gleich) richtet fehlende Werkzeuge ein (JDK/unzip/cc
  über den Paketmanager, SDK-Kommandozeilenwerkzeuge laut `repository2-3.xml` + `platforms;android-37.0` + `ndk;30.0.16248370`
  nach `~/Android/Sdk`, rustup nach `~/Android/rust`, cargo-ndk), baut und kopiert nach `target/<Titel>.apk`, installiert per
  adb bei genau einem Gerät (`ADB=` überschreibt adb, `NO_ADB=1` lässt es aus). App-Zusätze in `extra-apk.sh` (nur TubeQu:
  libclang). **Windows: `build-apk.ps1`** führt dasselbe Skript in WSL aus (richtet WSL + Ubuntu ein: Admin + Neustart),
  installiert danach mit dem Windows-adb. `.gitattributes` hält `*.sh`/`gradlew` auf LF (sonst bricht sh in WSL/MSYS2 an `\r`).
  Getestet nur unter Linux mit vorhandenen Werkzeugen; Neueinrichtung und Windows ungetestet.
- Handy: `adb install`. Fehlt noch: App-Icon, Upscaling (nur mit libmpv), Links bei schon offener App (`onNewIntent`).

## Aufbau (`src/` = GTK-App, `core/` = Crate `aniqu-core` ohne Oberfläche)

`core/` hängt nicht an GTK/mpv (qu-core mit `default-features = false`, reqwest auf Android mit rustls) – Grundlage
für eine spätere Android-App. Logik dorthin, Widgets nach `src/`. `main.rs` holt die Module per `use aniqu_core::{…}`.

| Datei | Aufgabe |
|---|---|
| `main.rs` | Fenster, Navigation (`Nav`: `browse`/`show`/`watch`, Verlauf zurück/vor), Nav-Bubble, Tasten, Maus-Seitentasten, Startargumente, `play`/`resume`/Links |
| `core/anime.rs` | **Einzige Stelle, die mit hianime.at spricht** (Trending, Suche, Anime-Seite, Episoden, Stream, Poster); Links (`anime_id`, `episode_param`), `available` (Dub) |
| `core/aniskip.rs` | Opening/Ending-Zeiten (api.aniskip.com, MAL-ID aus der Einbettungs-Adresse) |
| `grid.rs` | Poster-Raster: Startseite („Continue watching“ oben, dann Watchlist, darunter Trending ohne Watchlist-Animes), Rechtsklick-Menü der Karten (`card_menu`: Watchlist an/aus, bei „Continue watching“ „Watched“), Suche; Check auf neue Folgen; Strg+Mausrad = Spalten 4–12 (hoch = mehr, gespeichert in `~/.config/aniqu/columns`, nur so viele, wie Karten ≥ 120 px passen) |
| `show.rs` | Anime-Seite: Poster, Infos, Beschreibung, Sub/Dub, Lesezeichen-Knopf (Watchlist), Episoden-Kacheln; `Play` (Episode + Liste) |
| `player.rs` | Player-Anbindung an qu-core: `Hooks` (User-Agent, Fortschritt pro Episode, AniSkip-Abschnitte, Tempo pro Anime) und `media(meta, stream)` (Referer, Untertitel-Dateien, Fortsetzen). Item = `(Meta, MAL-ID)` |
| `watch.rs` | Infos unter dem Video (Titel → Anime-Seite, Episode, vorige/nächste) im `WatchFrame` aus qu-core |
| `core/progress.rs` | Fortschritt pro Episode, „Continue watching“ (fertig → nächste Episode), `caught_up`/`set_total`; „Watched“ = `hide` (`hidden.tsv`: ID + Zeitpunkt): Anime bleibt aus „Continue watching“ und ohne Neue-Folgen-Prüfung, bis danach wieder eine Episode gespeichert wird (neuerer Zeitstempel) |
| `core/lists.rs` | Suchverlauf, Watchlist (`~/.local/share/aniqu/watchlist.tsv`, Format wie `trending.tsv`: `anime::read_list`/`write_list`), Tempo und Sub/Dub pro Anime |
| `qu.toml` | App-Konfiguration für qu-core: Name/Titel/ID/Startmenü-Texte, **Menü oben links** (`[nav]`), Suche, Player, Fortschrittsregeln, Raster. Format in `~/Work/quCore/CLAUDE.md`; Test `qu_toml` in `main.rs` prüft sie (`cargo test --workspace`) |
| **qu-core** (`~/Work/quCore`) | Gemeinsamer Unterbau, **dort ändern**: App-Start (`start`, `--install`), Dateien/Tempo (`store`), Fortschrittsregeln (`progress`), Raster-Bausteine (`cards`), Fenster/Seitenverlauf/Miniplayer (`nav`), Player (`player`, `controls`, Rahmen `watch`), Menü (`navbar`), Ladeanzeige (`Busy`), `bg`, CSS-Schichten (`style`), `app` (Pfade), `theme`, gemeinsames CSS (`qu_core::CSS`), `icons` (`qu-*-symbolic`), `nav_shape`, `fly`, `smooth`, `search` (Suchfenster + Suchverlauf), `upscale` (Shader-Set hier: `UPSCALER` in `controls.rs`). Beschreibung in `~/Work/quCore/CLAUDE.md` |
| `style.css` | Nur App-eigenes CSS (Poster-Badges `.badge`, Episoden-Kacheln `.episode`); geladen nach `qu_core::CSS` |

## Datenquelle hianime.at (Stand 2026-11, Ablauf wie ani-cli 5.1.0)

- Trending: `/home`, Block `id="trending-home"` (12 Stück); Reserve aus derselben Seite: Top 10 (`li.item-top`) und die `flw-item`-Listen, damit Trending ohne Watchlist-Animes immer ganze Reihen füllt (`grid.rs` `trending_count`: 12 auf volle Reihen aufgerundet). Suche: `/search?keyword=` (Seitenleiste ab `id="main-sidebar"` abschneiden).
- Anime-Seite: `/<id>` (id = Slug mit `-<Nummer>` am Ende). Episoden: `/api/theme/episode/list/<Nummer>` (JSON mit `html`).
- Stream: `/api/theme/episode/servers?episodeId=` → Server „ZokoAnime“, `data-hash` = base64-Adresse
  `zokoanime.video/stream/mal/<mal>/<ep>/<sub|dub>` → `window.__P` = base64, XOR mit `otaku-embed-v1` → JSON
  (`src` = master.m3u8, `subtitles`). Master-Playlist mit Referer der Einbettung holen → Stufen 360/720/1080.
- reqwest mit Browser-User-Agent kommt durch Cloudflare (getestet 2026-11-07). Bei „Just a moment“ → Fehlermeldung.
- mpv: `referrer` + `user-agent`. Untertitel lädt `anime::stream` vorher nach `<tmp>/aniqu-subs/<nr>-<datei>/<Label>.vtt`
  (mit Referer) und gibt sie als **lokale** `sub-files` (Pfadliste mit „:“, unter Windows „;“; mit Adressen ginge das nicht); mpv nimmt den
  Dateinamen als Spur-Titel („English“), `player.rs` beobachtet daher `current-tracks/sub/title`.
- Untertitel-Knopf (wie TubeQu): Linksklick/`c` = an/aus, Rechtsklick = Menü (Sprache, Größe, Hintergrund).
  Gespeichert in `~/.config/aniqu/subtitles` (Sprache = Spur-Name, Größe, `box`/`outline`, `on`/`off`); an/aus gilt
  auch fürs nächste Video (`Controls::apply_subs` bei `FileLoaded`). Einblendungen „Subtitles: …“ nicht in den
  ersten 1,5 s nach `play`.
  Werte in `loadfile`-Optionen als `%Länge%Wert`.
- Kein Anfragen-Limit wie bei TubeQu nötig, aber sparsam bleiben (kein Vorladen).

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

Wie TubeQu (siehe `~/Work/tubequ/CLAUDE.md`): keine Kopfzeile, Nav-Bubble oben links, Buttons nur Icons mit Tooltip,
Laden von Inhalten = graue Platzhalter in Form des Inhalts mit Glanzstreifen (qu-core-CSS `.skeleton`, wie TubeQu): Raster `grid.rs` `ghost_card`/`set_ghost` (Breiten über `part`, keine festen Pixel), Anime-Seite `show.rs` `set_loading` (Infos, Beschreibung, Poster-Fläche, Episoden-Kacheln); sonst (Bubble, Player) Spinner = `icons::spinner()` (nie `gtk::Spinner`), keine knalligen Farben, keine festen Pixelbreiten im Raster.
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

- Timeline-Sprung: Die Timeline hält das Ziel (`seek_target`, `seek_state` in qu-core `controls.rs`), bis das Video 1 s darüber hinaus ist. mpv meldet nach dem Sprung erst das Ziel, dann das Schlüsselbild davor (`hr-seek=no`), vorher noch die alte Position – sonst springt der Zeiger Ziel → alt → Ziel bzw. kurz zurück.

- Player-Leiste: weg nach 2 s ohne Mausbewegung (`HIDE_AFTER`). Bewegung zählt nur bei geänderter Position (qu-core `player.rs`): GTK schickt nach Layout-Änderungen Bewegungs-Ereignisse an derselben Stelle nach.
- Maus-Seitentasten (GTK-Knopf 8/9) = Zurück/Vor, überall im Fenster (`Nav::forward`, merkt nur die Seite,
  nicht deren Inhalt; neuer Weg oder Home leert „Vor“).
- Weiches Scrollen mit dem Mausrad (`smooth.rs`, an jedem ScrolledWindow): Raststufen gleiten statt zu springen; Touchpad bleibt bei GTK.
- Feinschliff (qu-core-CSS, Ende): Vorschaubilder mit grauem Platzhalter (`.thumb-box`), Einblenden beim Laden (`.fade` → `.loaded`), Hover-Zoom 4 %; Ansehen-Seite blendet ein (`page-in-a/-b` im Wechsel, sonst startet die Animation nicht neu), Öffnen und Miniplayer: Zoom-Animation (`fly.rs`).
- Ton: Stummschalten blendet über den Filter `@fade` (lavfi `volume`, per `af-command`) in 160 ms aus/ein, `+`/`-` ändern die Lautstärke in kleinen Schritten (`Controls::toggle_mute`/`step_volume`). `af-command` meldet trotzdem einen Fehler (Ziel „all“ trifft auch Filter ohne den Befehl), wirkt aber.
- Tastatur im Raster: Pfeiltasten wählen die erste Karte, danach navigiert GTK; die Liste gleitet zur gewählten Karte (`smooth.rs`, nur bei Tastatur-Fokus). Timeline braucht nichts: mpv meldet `time-pos` jedes Bild.
- Miniplayer: Verlässt man die Ansehen-Seite, während ein Video läuft, spielt es unten rechts klein weiter (400 px; Knöpfe als Bubbles in den Ecken: oben links groß machen, oben rechts schließen, unten links Play + Ton). Tasten im Miniplayer: Leertaste/`k` Pause, `m` Ton, `f` zurück zum großen Video (kein Vollbild), `q` schließen; Pfeile/Ziffern bleiben fürs Raster. Die Ansehen-Seite liegt dafür als Ebene über dem Stack (im Stack nur ein Platzhalter „watch“) und wird nur verkleinert (`WatchFrame::set_mini`, `Nav::set_mini` in qu-core): der Player wird **nicht umgehängt**, sonst baut GTK die GLArea neu und mpv schaltet das Video ab. Am Ende/bei Fehler stoppt das Video (`Player::active`). Die Mini-Knöpfe erscheinen erst nach der Schrumpf-Animation (`Zoom::after`), sonst würden sie mit hochskaliert gezeichnet. Der Miniplayer lässt sich ziehen (`GestureDrag` an der Ansehen-Seite, Capture, ab 8 px; gezeichnet über `Zoom::set_offset`) und gleitet beim Loslassen (Ziehen in qu-core `nav.rs`) mit der Zoom-Animation in die nächste Ecke (`WatchFrame::set_corner`, gilt für die Sitzung). Klick ins Miniplayer-Video pausiert deshalb nicht (qu-core qu-core `player.rs` `in_mini`), Pause über den Knopf.
