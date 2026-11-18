# AniQu

A lightweight anime player for Linux (and, untested, Windows and macOS), written in Rust with GTK4 and libmpv.
Same look and feel as [Tube](https://github.com/simonroqu/tube).

- Home page with the six trending shows and everything you're in the middle of
- Search, show pages with all episodes, sub/dub switch (remembered per show)
- Embedded mpv player with its own controls, subtitles and quality switch
- "Skip intro" / "Skip ending" button during openings and endings ([AniSkip](https://aniskip.com))
- Continue watching per episode, playback speed per show
- Follows your [Omarchy](https://omarchy.org) theme live, ships its own default theme otherwise

## Requirements

- GTK ≥ 4.20
- mpv / libmpv
- Optional: `mpv-mpris` for media keys

## Install

One script per platform: it installs whatever is missing (GTK, mpv, Rust), builds AniQu and adds it to
your app launcher / start menu. The first run takes a while (downloading and compiling), later runs are quick.

### Linux

```sh
git clone https://github.com/simonroqu/AniQu
cd AniQu
./install.sh
```

Supports Arch, Debian/Ubuntu, Fedora and openSUSE (asks for your sudo password if packages are missing).
Needs a distribution with GTK ≥ 4.20 (e.g. Arch, Fedora 43, Ubuntu 25.10).
Installs to `~/.local/bin/aniqu` plus a launcher entry and icon.

### macOS (untested)

Needs [Homebrew](https://brew.sh). Then in Terminal:

```sh
git clone https://github.com/simonroqu/AniQu
cd AniQu
./install.sh
```

Installs `~/Applications/AniQu.app` (shows up in Launchpad and Spotlight, no icon yet) and `~/.local/bin/aniqu`.

### Windows (untested)

In PowerShell:

```powershell
winget install -e --id Git.Git      # once, if git is missing (then open a new PowerShell)
git clone https://github.com/simonroqu/AniQu
cd AniQu
powershell -ExecutionPolicy Bypass -File install.ps1
```

The script installs [MSYS2](https://www.msys2.org) (if missing) and builds AniQu with GTK and mpv from there.
Installs to `%LOCALAPPDATA%\Programs\AniQu` plus a start menu entry.

### Update and uninstall

- Update: `git pull` in the folder, then run the install script again. It always builds against the latest
  [qu-core](https://github.com/simonroqu/quCore).
- Uninstall: delete the files the install script lists at the end.
- Building fetches qu-core from GitHub, so it has to be reachable from your machine.

## Usage

Run directly:

```sh
aniqu                    # home page
aniqu frieren            # search
aniqu <show-link>        # open a show
aniqu <episode-link>     # play an episode (link with ?ep=…)
```

## Keyboard shortcuts

| Key | Action |
|---|---|
| `/`, `Ctrl+K` | Search |
| `F5`, `Ctrl+R` | Reload |
| `Ctrl+V` | Open link from clipboard |
| `n` | Next episode |
| `0`–`9` | Seek to 0–90 % |
| `,` / `.` | Previous / next frame |
| `c` | Subtitles on/off (right-click the subtitle button for language, size and background) |

Double-click left/right on the player seeks −/+10 s, the middle toggles fullscreen.

## Data

- `~/.local/share/aniqu/`: progress, recent searches, speed and sub/dub per show
- `~/.config/aniqu/`: quality, volume
- `~/.cache/aniqu/`: posters, icons, last trending list

## Credits

- Stream lookup follows [ani-cli](https://github.com/pystardust/ani-cli) (GPL-3.0)
- [AniSkip](https://aniskip.com) for opening/ending times, [mpv](https://mpv.io) for playback
- [Anime4K](https://github.com/bloc97/Anime4K) upscaling shaders by bloc97 (MIT, via qu-core)

## Disclaimer

AniQu is a personal project for private use. It does not host any content; it only plays what
third-party sites provide. Use it in accordance with the laws of your country and the terms of
the services you access. AniQu is not affiliated with any of these sites or with any rights holder.

## License

[GPL-3.0-or-later](LICENSE)
