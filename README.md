# dash-live-stream

Simple command line app that hosts a DASH live stream on an http server. Takes input from an rtmp stream.

## Runtime Requirements

- [FFmpeg](https://www.ffmpeg.org/download.html) 4.2.2 minimum

## Compiling

- install openssl

```sh
# for Windows, build with static openssl and use scoop's file names
set OPENSSL_STATIC=1
set OPENSSL_LIBS=libssl_static:libcrypto_static

cargo install --git https://github.com/SpiralP/dash-live-stream.git
```

## Usage

```
USAGE:
    dash-live-stream [FLAGS] [OPTIONS] [file]

FLAGS:
    -h, --help
            Prints help information

    -s, --tls
            Use secured https

    -V, --version
            Prints version information

    -v, --verbose
            Show debug messages, use multiple times for higher verbosity


OPTIONS:
        --audio-bitrate <bitrate>
            Sets the bitrate of the output audio.
            128kbps for 720p
            192kbps for 1080p [default: 128k]
        --audio-sample-rate <sample-rate>
            Sets the sample rate of the output audio [default: 44100]

        --cpu-used <number>
            Sets amount of cpu to use for encoding, higher values mean less cpu.
            This is a value between 0 and 15 that controls how efficient the compression will be.
            Speed 5 to 8 should be used for live / real-time encoding.
            Lower numbers (5 or 6) are higher quality but require more CPU power.
            Higher numbers (7 or 8) will be lower quality but more manageable for lower latency use cases and also for
            lower CPU power devices such as mobile.
            More info at https://developers.google.com/media/vp9/live-encoding and under 'CPU Utilization / Speed' at
            https://trac.ffmpeg.org/wiki/Encode/VP9 and https://www.webmproject.org/docs/encoder-parameters/ [default:
            5]
        --crf <value>
            Sets the CRF (Constant Rate Factor) value of the output video.
            The CRF value can be from 0–63.
            Lower values mean better quality.
            Recommended values range from 15–35, with 31 being recommended for 1080p HD video.
            More info under 'Constrained Quality' at https://trac.ffmpeg.org/wiki/Encode/VP9 [default: 30]
        --framerate <fps>
            Sets the framerate of the output video [default: 30]

    -i, --http-ip <address>
            Sets the listen ip address for http [default: 0.0.0.0]

    -p, --http-port <port>
            Sets the listen http port [default: 3000]

        --remote <address>
            Instead of hosting a dash server, stream to a remote rtmp server

        --rtmp-ip <address>
            Sets the listen ip address for rtmp [default: 127.0.0.1]

    -r, --rtmp-port <port>
            Sets the listen rtmp port [default: 1935]

        --seek <time>
            Seek input file to time

        --subtitles <file>
            Use a subtitles file to hardsub subtitles into the video track

        --video-bitrate <bitrate>
            Sets bitrate of the output video.
            1200-4000k for 720p
            4000-8000k for 1080p [default: 4000k]
        --resolution <WIDTHxHEIGHT>
            Sets resolution of the output video [default: 1280x720]


ARGS:
    <file>
            Play a file instead of starting an rtmp server
```
