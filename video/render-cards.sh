#!/usr/bin/env bash
# Renders the website card clips into ../site/assets/cards:
# 1920x1080 (scale 2): alpha WebM (Chrome/Firefox), black MP4 (Safari, blended with `screen`), PNG poster.
set -euo pipefail
cd "$(dirname "$0")"
out=../site/assets/cards
mkdir -p "$out"
for c in Speed:110 Effects:110 Friend:120 Pipeline:120 Search:120 Loaders:110; do
  id=${c%%:*}; frame=${c##*:}; low=$(echo "$id" | tr 'A-Z' 'a-z')
  npx remotion render "$id" "$out/$low.webm" --codec=vp9 --pixel-format=yuva420p --image-format=png --crf=20 --scale=2 --props='{"alpha":true}' --log=error
  npx remotion render "$id" "$out/$low.mp4" --codec=h264 --crf=18 --pixel-format=yuv420p --image-format=png --scale=2 --log=error
  npx remotion still "$id" "$out/$low.png" --frame="$frame" --image-format=png --scale=2 --props='{"alpha":true}' --log=error
done
ls -la "$out"
