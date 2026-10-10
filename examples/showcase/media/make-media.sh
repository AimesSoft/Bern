#!/bin/sh
# Regenerate the showcase's controlled test media with ffmpeg.
#
#   aurora.mp4     8 s test pattern with a 440 Hz tone, seekable
#   tides.mp4      6 s color bars with a 660 Hz tone, seekable
#   live/live.m3u8 HLS event playlist without an end tag: playing, but its
#                  duration is unknown and it cannot seek
#   broken.mp4     not a movie, to exercise playback failure
set -eu
cd "$(dirname "$0")"
ffmpeg -loglevel error -y \
    -f lavfi -i "testsrc2=size=640x360:rate=25:duration=8" \
    -f lavfi -i "sine=frequency=440:sample_rate=44100:duration=8" \
    -c:v libx264 -profile:v main -pix_fmt yuv420p -crf 34 -preset veryslow \
    -c:a aac -b:a 48k -movflags +faststart -shortest aurora.mp4
ffmpeg -loglevel error -y \
    -f lavfi -i "smptehdbars=size=640x360:rate=25:duration=6" \
    -f lavfi -i "sine=frequency=660:sample_rate=44100:duration=6" \
    -c:v libx264 -profile:v main -pix_fmt yuv420p -crf 34 -preset veryslow \
    -c:a aac -b:a 48k -movflags +faststart -shortest tides.mp4
rm -rf live
mkdir live
ffmpeg -loglevel error -y \
    -f lavfi -i "testsrc=size=640x360:rate=25:duration=12" \
    -f lavfi -i "sine=frequency=330:sample_rate=44100:duration=12" \
    -c:v libx264 -profile:v main -pix_fmt yuv420p -crf 36 -preset veryslow -g 50 \
    -c:a aac -b:a 48k -shortest \
    -f hls -hls_time 4 -hls_playlist_type event -hls_flags omit_endlist \
    -hls_segment_filename "live/segment%d.ts" live/live.m3u8
printf 'This file is deliberately not a movie.\n' > broken.mp4
