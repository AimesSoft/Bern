#!/bin/sh
# Regenerate the showcase's sample pictures.
#
#   landscape.jpg  640x360 JPEG, a Mandelbrot set rendered by ffmpeg
#   bern.png       256x256 PNG with transparency, the Bern mark
set -eu
cd "$(dirname "$0")"
ffmpeg -loglevel error -y -f lavfi -i "mandelbrot=size=640x360:start_scale=2.4:end_pts=1" \
    -frames:v 1 -q:v 6 landscape.jpg
cp ../../../crates/bern-target-api/assets/icon/round-256.png bern.png
