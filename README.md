# rspleeter

Split a song into vocals and accompaniments, taking advantage of machine learning.

Rust implementation of [`spleeter`](https://github.com/deezer/spleeter). One of the [`rsmpeg`](https://github.com/larksuite/rsmpeg) demos.

Inspired by:
- https://github.com/deezer/spleeter
- https://github.com/wudicgi/SpleeterMsvcExe
- https://github.com/gvne/spleeterpp

MSRV: `1.59.0`

You can check the examples we provided.

- Original: <https://ldm0.xyz/assets/ten_years.mp3>
- Vocal: <https://ldm0.xyz/assets/ten_years-vocals.mp3>
- Accompaniment: <https://ldm0.xyz/assets/ten_years-accompaniment.mp3>

## Getting started

**Attention**: For Windows developers, get prebuilt FFmpeg from the release page or manually cross compile FFmpeg is needed. If you are using Windows, read the next section first.


```bash
# Install `nasm` and `libmp3lame`.
# macOS
brew install nasm lame
# Linux
sudo apt install nasm libmp3lame-dev

# Prepare pre-trained models
curl -L -O https://github.com/ldm0/rspleeter/releases/download/0.1.0-models/models.zip

unzip models.zip -d models

# Get a test song.
curl -O https://ldm0.xyz/assets/ten_years.mp3

# Split the tests song with 2stems model, the output folder is `target/ten_years`.
# Run `cargo xtask run --release -- --help` for more options.
cargo xtask run --release -- ten_years.mp3 target/ten_years
```

Then play the `target/ten_years/accompaniment.mp3`, have fun!

## FFmpeg dylib

If you find building ffmpeg annoying, you can skip it by using prebuilt FFmpeg. Download prebuilt FFmpeg artifacts from the release page, decompress it and put it under the source folder. (e.g. `./prebuilt_ffmpeg/lib/libffmpeg.dylib`).

When `cargo xtask` find the prebuilt FFmpeg artifacts, it will link against it and skip the FFmpeg building.

If you want to build it manually, check this [doc](doc/build_ffmpeg_dylib_manually.md).
