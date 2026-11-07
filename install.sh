#!/bin/sh
# Baut AniQu und installiert es für den aktuellen Nutzer (ohne root) samt Startmenü-Eintrag.
# Entfernen: die unten genannten Dateien löschen.
set -e
cd "$(dirname "$0")"

cargo build --release

bin="$HOME/.local/bin/aniqu"
icon="$HOME/.local/share/icons/hicolor/scalable/apps/dev.roqu.AniQu.svg"
desktop="$HOME/.local/share/applications/dev.roqu.AniQu.desktop"

install -Dm755 target/release/aniqu "$bin"
install -Dm644 data/dev.roqu.AniQu.svg "$icon"
mkdir -p "$(dirname "$desktop")"
sed "s|@BIN@|$bin|" data/dev.roqu.AniQu.desktop > "$desktop"

# Startmenü neu einlesen (falls das Werkzeug da ist).
command -v update-desktop-database >/dev/null && update-desktop-database -q "$(dirname "$desktop")" || true

echo "Installiert: $bin"
echo "Startmenü:   $desktop"
