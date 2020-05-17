# dash-live-stream

Simple command line app that hosts a DASH live stream on an http server. Takes input from an rtmp stream.

## Runtime Requirements

- [FFmpeg](https://www.ffmpeg.org/download.html) 4.2.2

## Compiling

- install openssl

```
set OPENSSL_STATIC=1
set OPENSSL_LIBS=libssl_static:libcrypto_static
cargo build --release
```
