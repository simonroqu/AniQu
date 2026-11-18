# AniQu

A lightweight anime player for Linux (and, untested, Windows and macOS), written in Rust with GTK4 and libmpv.
Same look and feel as [TubeQu](https://github.com/simonroqu/tubequ).

- Home page with the six trending shows and everything you're in the middle of
- Search, show pages with all episodes, sub/dub switch (remembered per show)
- Embedded mpv player with its own controls, subtitles and quality switch
- "Skip intro" / "Skip ending" button during openings and endings ([AniSkip](https://aniskip.com))
- Continue watching per episode, playback speed per show
- Follows your [Omarchy](https://omarchy.org) theme live, ships its own default theme otherwise
- Android app for phones and Android TV (build it yourself, see [Android](#android-phone-and-android-tv))

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

### Android (phone and Android TV)

There is no ready-made APK to download: one script builds it from the same folder. Trending, search, show pages with sub/dub, episodes, subtitles, "Skip intro" / "Skip ending", continue watching. hianime.at links open the app.

Works on phones and on Android TV (shows up in the TV launcher, fully usable with the remote). Needs Android 8.0 or newer.

**Linux and macOS (macOS untested):**

```sh
./build-apk.sh
```

**Windows (untested):** in PowerShell, in the cloned folder:

```powershell
powershell -ExecutionPolicy Bypass -File build-apk.ps1
```

Windows builds through [WSL](https://learn.microsoft.com/windows/wsl/) (Linux inside Windows) with the same script.
If WSL isn't set up yet, the first run sets it up with Ubuntu: it asks for administrator rights, then **restart Windows**,
open "Ubuntu" from the start menu once (pick a user name and password) and run the script again. Needs Windows 10
(version 2004 or newer) or Windows 11 with virtualization enabled; company PCs sometimes block WSL.

**What the script does:** it sets up everything that's missing (a JDK, the Android SDK and NDK in `~/Android/Sdk`,
Rust with the Android targets in `~/Android/rust` — your normal Rust stays untouched — and cargo-ndk), then builds
`target/AniQu.apk`. The first run downloads a few GB and takes a while, later runs are quick. It asks for your password
only to install system packages (JDK, unzip, a C compiler).

The APK is **not** stored in the repository and does **not** update by itself: after every `git pull`, run the script again.

**Install on the device:**

- If exactly one device is connected with USB or wireless debugging (developer options), the script installs the APK
  right away. On Windows this needs adb: `winget install -e --id Google.PlatformTools`.
- Otherwise copy `target/AniQu.apk` to the device (USB, cloud, a USB stick for TVs), open it and allow installing apps
  from that file manager.
- Android TV with adb: enable developer options and network debugging on the TV, then
  `adb connect <tv-ip>:5555` and `adb install -r target/AniQu.apk`.

**Updates:** install the new APK over the old one, your data stays. The APK is signed with a debug key that is
created per computer: an APK built on a different computer can't update the installed one, uninstall first
(this deletes progress and lists).

### Update and uninstall

- Update: `git pull` in the folder, then run the install script (and `build-apk` for Android) again. It always builds against the latest
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

Paths for Linux and macOS; on Windows the same files are in `%APPDATA%\aniqu` and `%LOCALAPPDATA%\aniqu` (cache).
On Android everything stays inside the app (uninstalling deletes it).

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
