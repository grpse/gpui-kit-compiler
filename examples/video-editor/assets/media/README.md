# Bundled real media

These files are imported through the same native FFmpeg preprocessing pipeline as user-selected files. Startup needs no download. `manifest.json` records source pages, authors, download URLs, transformations, byte sizes, and SHA-256 checksums.

| File | Creator and source | License / changes |
| --- | --- | --- |
| `murchison-falls.webm` | [BMKME, Rainbow over Murchison Falls National Park](https://commons.wikimedia.org/wiki/File:V8.webm) | CC0 1.0; original VP9/Opus file renamed |
| `bell.wav` | [code4fukui/sound-cc0](https://github.com/code4fukui/sound-cc0) | CC0 dedication in the project README; original `bell1.wav` renamed |
| `cracker-stereo.wav` | [code4fukui/sound-cc0](https://github.com/code4fukui/sound-cc0) | CC0 dedication in the project README; original `cracker1v-stereo.wav` renamed |
| `piano-emo-10-excerpt.ogg` | [Tozan, Piano Emo 10](https://opengameart.org/content/piano-emo-10) | CC0 1.0; approximately 8-second excerpt with compressed packets preserved |

The sound-cc0 repository also carries an Unlicense license file. Its README expressly dedicates the sound recordings to CC0. All media here is available for personal and commercial use. Credits are retained even where attribution is optional. The CC0 legal text is included in `CC0-1.0.txt`, obtained from the [SPDX license list](https://github.com/spdx/license-list-data/blob/main/text/CC0-1.0.txt).

To recreate the piano excerpt from the original download (asset preparation only):

```sh
ffmpeg -i pianoemo10_0.ogg -t 8 -map 0:a:0 -c:a copy piano-emo-10-excerpt.ogg
```

The application itself uses the FFmpeg libraries in-process for import, channel separation, waveform analysis, video decoding, and playback resampling. It does not invoke this command.
