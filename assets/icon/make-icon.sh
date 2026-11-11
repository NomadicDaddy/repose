#!/usr/bin/env bash
# Regenerates repose's icon files from repose.svg (program-icon clause 4). Needs ImageMagick 7
# (`magick`). Writes repose-256.png, which the window loads as its icon, and repose.ico, which
# crates/app/build.rs compiles into the Windows executable, holding 16, 20, 24, 32, 40, 48, 64
# and 256 px. Run it from anywhere; commit the files it writes with the change to repose.svg.
set -euo pipefail
cd "$(dirname "$0")"
render() { magick -background none -density 384 repose.svg -resize "$1x$1" "$2"; }
render 256 repose-256.png
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
files=()
for s in 16 20 24 32 40 48 64 256; do
	render "$s" "$work/$s.png"
	files+=("$work/$s.png")
done
magick "${files[@]}" repose.ico
magick identify repose.ico
