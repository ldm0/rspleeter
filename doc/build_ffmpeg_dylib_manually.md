## Windows 

Get a Linux machine or WSL.

```bash
# Install nasm
sudo apt install nasm

# Prepare llvm-mingw
curl -OL https://github.com/mstorsjo/llvm-mingw/releases/download/20220323/llvm-mingw-20220323-ucrt-ubuntu-18.04-x86_64.tar.xz
tar xvf llvm-mingw-20220323-ucrt-ubuntu-18.04-x86_64.tar.xz
export PATH=${PWD}/llvm-mingw-20220323-ucrt-ubuntu-18.04-x86_64/bin:${PATH}

# Cross compile `libmp3lame`
curl -O https://jaist.dl.sourceforge.net/project/lame/lame/3.100/lame-3.100.tar.gz
tar xvf lame-3.100.tar.gz
cd lame-3.100
CC="x86_64-w64-mingw32-gcc" ./configure --prefix=${PWD}/../lame_build
make -j$(nproc)
make install 
cd ..

# Build FFmpeg with libmp3lame
git clone https://github.com/ffmpeg/ffmpeg
cd ffmpeg
./configure \
    --arch=x86_64 \
    --pkg-config=pkg-config \
    --target-os=mingw32 \
    --cross-prefix=x86_64-w64-mingw32- \
    --prefix=${PWD}/../ffmpeg_build \
    --disable-hwaccels \
    --disable-encoders \
    --enable-encoder=flac,aac,opus,pcm_f32le,alac,wmav2,libmp3lame \
    --disable-decoders \
    --enable-decoder=flac,aac,opus,pcm_f32le,alac,wmav2,mp3 \
    --disable-parsers \
    --disable-protocols \
    --disable-bsfs \
    --disable-indevs \
    --disable-outdevs \
    --disable-filters \
    --disable-programs \
    --enable-protocol=file \
    --extra-cflags=-I${PWD}/../lame_build/include \
    --extra-ldflags=-L${PWD}/../lame_build/lib \
    --enable-libmp3lame
make -j$(nproc)
make install
cd ..

# Generate dll from static lib
cd ffmpeg_build/lib/
x86_64-w64-mingw32-gcc -shared \
    -o libffmpeg.dll \
    -Wl,--out-implib,libffmpeg.lib \
    -Wl,--whole-archive *.a -Wl,--no-whole-archive \
    -L${PWD}/../../lame_build/lib \
    -lmp3lame \
    -lws2_32 \
    -lbcrypt \
    -lole32
cd ../..
```

## macOS & Linux

First of all, install `nasm` and `libmp3lame`.

- macOS: `brew install nasm lame`
- Linux: `sudo apt install nasm libmp3lame-dev`

Then run `cargo xtask build --release`, after it completes, you will find the ffmpeg artifacts in `target/ffmpeg_build`.

If you want to know the FFmpeg build arguments, check `xtask/main.rs`
