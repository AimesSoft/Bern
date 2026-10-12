#!/bin/sh
# Regenerate the showcase's sample pictures.
#
#   landscape.jpg  640x360 JPEG, a Mandelbrot set rendered by ffmpeg
#   bern.png       256x256 PNG with transparency, the Bern mark
#   hdr-*          the HDR fixtures of bern-image (crates/bern-image/tests/
#                  fixtures/make-fixtures.swift): SDR white on the left, four
#                  times as bright on the right, as a JPEG with a gain map
#                  and as a PQ PNG
set -eu
cd "$(dirname "$0")"
ffmpeg -loglevel error -y -f lavfi -i "mandelbrot=size=640x360:start_scale=2.4:end_pts=1" \
    -frames:v 1 -q:v 6 landscape.jpg
cp ../../../crates/bern-target-api/assets/icon/round-256.png bern.png
cp ../../../crates/bern-image/tests/fixtures/hdr-gainmap.jpg ../../../crates/bern-image/tests/fixtures/hdr-pq.png .
