#!/usr/bin/env bash
# Stopgap glue for Phase 2 until a single `xperience` binary exists:
# open the selector, then launch the chosen ROM in emu-run.
#
#   scripts/play.sh /path/to/snes9x_libretro.dylib
#
# Everything else (catalogue, save dir) uses the defaults.
set -euo pipefail

CORE="${1:?usage: play.sh <path to snes9x_libretro.{dylib,so,dll}>}"
BIN_DIR="$(cd "$(dirname "$0")/.." && pwd)/target/release"

while :; do
    rom="$("$BIN_DIR/selector")" || exit 0   # cancelled
    "$BIN_DIR/emu-run" --core "$CORE" --rom "$rom" --save-dir "${XPERIENCE_SAVES:-$HOME/.local/share/snes-xperience/saves}"
done
