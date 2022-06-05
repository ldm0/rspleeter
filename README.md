# rspleeter

Split a song into vocal and accompaniments, taking advantage of machine learning.

Rust implementation of [`spleeter`](https://github.com/deezer/spleeter). One of the [`rsmpeg`](https://github.com/larksuite/rsmpeg) demos.

Inspired by:
- https://github.com/deezer/spleeter
- https://github.com/wudicgi/SpleeterMsvcExe
- https://github.com/gvne/spleeterpp

MSRV: `1.59.0`

## Getting started

Attention: If you are using Windows, read the next section first.

```bash
# Prepare pre-trained models
curl -L -O https://github.com/gvne/spleeterpp/releases/download/models-1.0/models.zip

unzip models.zip -d models

# Get a test song.
curl -O https://ldm0.xyz/assets/noodles.mp3

# Split the tests song with 2stems model, the output folder is `target/noodles`.
# Run `cargo xtask run --release -- --help` for more options.
cargo xtask run --release -- noodles.mp3 target/noodles
```

Then play the `target/noodles/accompaniment.mp3`, have fun!

## FFmpeg dylib

For Windows, get prebuilt FFmpeg from the release page or manually cross compile FFmpeg is needed.

Download prebuilt FFmpeg artifacts from the release page, decompress it and put it under the source folder. (e.g. `./prebuilt_ffmpeg/lib/libffmpeg.dylib`)

If you want to build it manually, check this [doc](doc/build_ffmpeg_dylib_manually.md).
