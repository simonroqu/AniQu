# AniQu

A lightweight anime player for Linux, written in Rust with GTK4 and libmpv.
Same look and feel as [Tube](https://github.com/simonroqu/tube).

- Home page with the six trending shows and everything you're in the middle of
- Search, show pages with all episodes, sub/dub switch (remembered per show)
- Embedded mpv player with its own controls, subtitles and quality switch
- Skips openings and endings automatically ([AniSkip](https://aniskip.com))
- Continue watching per episode, playback speed per show
- Follows your [Omarchy](https://omarchy.org) theme live, ships its own default theme otherwise

## Requirements

- GTK ≥ 4.20
- mpv / libmpv
- Optional: `mpv-mpris` for media keys

## Build & install

```sh
cargo build --release
./install.sh        # installs to ~/.local/bin/aniqu and adds a launcher entry
```

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
| `c` | Toggle subtitles |

Double-click left/right on the player seeks −/+10 s, the middle toggles fullscreen.

## Data

- `~/.local/share/aniqu/`: progress, recent searches, speed and sub/dub per show
- `~/.config/aniqu/`: quality, volume
- `~/.cache/aniqu/`: posters, icons, last trending list

## Credits

- Stream lookup follows [ani-cli](https://github.com/pystardust/ani-cli) (GPL-3.0)
- [AniSkip](https://aniskip.com) for opening/ending times, [mpv](https://mpv.io) for playback

## Disclaimer

AniQu is a personal project for private use. It does not host any content; it only plays what
third-party sites provide. Use it in accordance with the laws of your country and the terms of
the services you access.

## License

[GPL-3.0-or-later](LICENSE)
