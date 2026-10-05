# Native preprocessing fixture

`multichannel.mkv` is generated test media (132 KiB): 10 frames of 96×64, 25 fps FFV1 in `yuv420p10le`; stereo 24-bit PCM at 48 kHz; and mono AAC at 44.1 kHz. Tests use only the integrated FFmpeg libraries. Its timestamps include AAC encoder priming and coarse Matroska audio timing.

Regenerate from the repository root with an installed FFmpeg CLI (remove the existing fixture first):

```sh
ffmpeg -v error -nostdin -n \
  -f lavfi -i 'testsrc2=size=96x64:rate=25:duration=0.4' \
  -f lavfi -i 'aevalsrc=0.25*sin(2*PI*440*t)|0.5*sin(2*PI*880*t):s=48000:d=0.4' \
  -f lavfi -i 'sine=frequency=220:sample_rate=44100:duration=0.4' \
  -map 0:v -map 1:a -map 2:a -c:v ffv1 -pix_fmt yuv420p10le \
  -c:a:0 pcm_s24le -c:a:1 aac -b:a:1 96k \
  examples/video-editor/assets/tests/multichannel.mkv
```
