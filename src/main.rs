mod error;
mod ffmpeg;
mod helpers;
mod logger;
mod web;

use crate::{
    error::*,
    ffmpeg::{Ffmpeg, FfmpegInput, FfmpegOutput},
    helpers::*,
};
use clap::{crate_name, crate_version, App, Arg};
use futures::{channel::mpsc, stream::StreamExt};
use log::*;
use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    time::Duration,
};
use tokio::runtime::Runtime;

fn main() -> Result<()> {
    #[allow(unused_mut)]
    let mut app = App::new(crate_name!())
        .version(crate_version!())
        .arg(
            Arg::with_name("verbose")
                .long("verbose")
                .short("v")
                .help("Show debug messages, use multiple times for higher verbosity")
                .multiple(true),
        )
        .arg(Arg::with_name("file").help("Play a file instead of starting an rtmp server"))
        .arg(
            Arg::with_name("seek")
                .alias("time")
                .long("seek")
                .help("Seek input file to time")
                .value_name("time")
                .takes_value(true)
                .requires("file"),
        )
        .arg(
            Arg::with_name("subtitles")
                .long("subtitles")
                .help("Use a subtitles file to hardsub subtitles into the video track")
                .value_name("file")
                .takes_value(true)
        )
        .arg(
            Arg::with_name("remote-rtmp")
                .long("remote")
                .help("Instead of hosting a dash server, stream to a remote rtmp server")
                .value_name("address")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("rtmp-ip")
                .long("rtmp-ip")
                .help("Sets the listen ip address for rtmp")
                .value_name("address")
                .takes_value(true)
                .default_value("127.0.0.1"),
        )
        .arg(
            Arg::with_name("rtmp-port")
                .long("rtmp-port")
                .short("r")
                .help("Sets the listen rtmp port")
                .value_name("port")
                .takes_value(true)
                .default_value("1935"),
        )
        .arg(
            Arg::with_name("http-ip")
                .long("http-ip")
                .short("i")
                .help("Sets the listen ip address for http")
                .value_name("address")
                .takes_value(true)
                .default_value("0.0.0.0"),
        )
        .arg(
            Arg::with_name("http-port")
                .long("http-port")
                .short("p")
                .help("Sets the listen http port")
                .value_name("port")
                .takes_value(true)
                .default_value("3000"),
        )
        .arg(
            Arg::with_name("cpu-used")
                .alias("speed")
                .long("cpu-used")
                .help("Sets amount of cpu to use for encoding, higher values mean less cpu")
                .long_help(
                    "Sets amount of cpu to use for encoding, higher values mean less cpu.\nThis \
                     is a value between 0 and 15 that controls how efficient the compression will \
                     be.\nSpeed 5 to 8 should be used for live / real-time encoding.\nLower \
                     numbers (5 or 6) are higher quality but require more CPU power.\nHigher \
                     numbers (7 or 8) will be lower quality but more manageable for lower latency \
                     use cases and also for lower CPU power devices such as mobile.\nMore info at \
                     https://developers.google.com/media/vp9/live-encoding and under 'CPU \
                     Utilization / Speed' at https://trac.ffmpeg.org/wiki/Encode/VP9 and \
                     https://www.webmproject.org/docs/encoder-parameters/",
                )
                .value_name("number")
                .takes_value(true)
                .default_value("5"),
        )
        .arg(
            Arg::with_name("video-resolution")
                .long("resolution")
                .help("Sets resolution of the output video")
                .value_name("WIDTHxHEIGHT")
                .takes_value(true)
                .default_value("1280x720"),
        )
        .arg(
            Arg::with_name("video-bitrate")
                .long("video-bitrate")
                .help("Sets bitrate of the output video")
                .long_help(
                    "Sets bitrate of the output video.\n1200-4000k for 720p\n4000-8000k for 1080p",
                )
                .value_name("bitrate")
                .takes_value(true)
                .default_value("4000k"),
        )
        .arg(
            Arg::with_name("crf")
                .long("crf")
                .help("Sets the CRF value of the output video")
                .long_help(
                    "Sets the CRF (Constant Rate Factor) value of the output video.\nThe CRF \
                     value can be from 0–63.\nLower values mean better quality.\nRecommended \
                     values range from 15–35, with 31 being recommended for 1080p HD video.\nMore \
                     info under 'Constrained Quality' at https://trac.ffmpeg.org/wiki/Encode/VP9",
                )
                .value_name("value")
                .takes_value(true)
                .default_value("30"),
        )
        .arg(
            Arg::with_name("framerate")
                .alias("frame-rate")
                .long("framerate")
                .help("Sets the framerate of the output video")
                .value_name("fps")
                .takes_value(true)
                .default_value("30"),
        )
        .arg(
            Arg::with_name("audio-sample-rate")
                .long("audio-sample-rate")
                .help("Sets the sample rate of the output audio")
                .value_name("sample-rate")
                .takes_value(true)
                .default_value("44100"),
        )
        .arg(
            Arg::with_name("audio-bitrate")
                .long("audio-bitrate")
                .help("Sets the bitrate of the output audio")
                .long_help(
                    "Sets the bitrate of the output audio.\n128kbps for 720p\n192kbps for 1080p",
                )
                .value_name("bitrate")
                .takes_value(true)
                .default_value("128k"),
        );

    #[cfg(feature = "tls")]
    {
        app = app.arg(
            Arg::with_name("tls")
                .short("s")
                .long("tls")
                .alias("ssl")
                .alias("https")
                .help("Use secured https"),
        );
    }

    let matches = app.get_matches();

    let verbose = matches.is_present("verbose");
    logger::initialize(
        cfg!(debug_assertions) || verbose,
        matches.occurrences_of("verbose") > 1,
    );

    let log_http = matches.is_present("verbose");

    let tls = matches.is_present("tls");

    let input = if let Some(path) = matches.value_of("file") {
        let path = PathBuf::from(path);
        let seek = if let Some(seek) = matches.value_of("seek") {
            Some(parse_duration(seek)?)
        } else {
            None
        };
        FfmpegInput::File { path, seek }
    } else {
        let rtmp_ip: IpAddr = matches.value_of("rtmp-ip").unwrap().parse()?;
        let rtmp_port: u16 = matches.value_of("rtmp-port").unwrap().parse()?;
        FfmpegInput::Rtmp(SocketAddr::new(rtmp_ip, rtmp_port))
    };

    let http_ip: IpAddr = matches.value_of("http-ip").unwrap().parse()?;
    let http_port: u16 = matches.value_of("http-port").unwrap().parse()?;

    let framerate = matches.value_of("framerate").unwrap().parse()?;

    let video_bitrate = matches.value_of("video-bitrate").unwrap().to_string();
    let video_resolution = matches.value_of("video-resolution").unwrap().to_string();

    let audio_sample_rate = matches.value_of("audio-sample-rate").unwrap().to_string();
    let audio_bitrate = matches.value_of("audio-bitrate").unwrap().to_string();

    let cpu_used = matches.value_of("cpu-used").unwrap().parse()?;
    let crf = matches.value_of("crf").unwrap().parse()?;

    let subtitles_path = matches.value_of("subtitles").map(Into::into);

    let temp_dir = tempfile::Builder::new()
        .prefix(&format!(".{}", crate_name!()))
        .tempdir()?;

    let temp_dir_path = temp_dir.path().to_owned();
    debug!("created temp dir {:?}", temp_dir_path);

    let output = if let Some(addr) = matches.value_of("remote-rtmp") {
        FfmpegOutput::Rtmp(addr.parse()?)
    } else {
        FfmpegOutput::Dash(temp_dir_path)
    };

    let mut rt = Runtime::new()?;
    rt.block_on(async move {
        let (sender, mut receiver) = mpsc::unbounded();

        {
            let sender = sender.clone();
            ctrlc::set_handler(move || {
                info!("stopping...");
                let _ignore = sender.unbounded_send(());
            })
            .expect("Error setting Ctrl-C handler");
        }

        match &output {
            FfmpegOutput::Dash(temp_dir_path) => {
                // only start http server if we're going to use it

                let temp_dir_path = temp_dir_path.clone();
                let sender = sender.clone();

                tokio::spawn(async move {
                    if let Err(e) = web::start(
                        SocketAddr::new(http_ip, http_port),
                        temp_dir_path,
                        tls,
                        log_http,
                    )
                    .await
                    {
                        error!("web: {}", e);
                    }
                    let _ignore = sender.unbounded_send(());
                });
            }

            FfmpegOutput::Rtmp(addr) => {
                info!("sending to remote rtmp at {}", addr);
            }
        }

        {
            let mut ffmpeg = Ffmpeg {
                command: None,
                verbose,
                input,
                output,
                cpu_used,
                framerate,
                crf,
                video_bitrate,
                video_resolution,
                audio_bitrate,
                audio_sample_rate,
                subtitles_path,
            };

            tokio::spawn(async move {
                if let Err(e) = ffmpeg.run().await {
                    error!("ffmpeg: {}", e);
                } else {
                    info!(
                        "ffmpeg exited cleanly, sleeping for a bit so that the video finishes \
                         downloading"
                    );
                    // exited cleanly, let's keep hosting the files until the video stops
                    tokio::time::delay_for(Duration::from_secs(6)).await;
                }
                let _ignore = sender.unbounded_send(());
            });
        }

        // wait until something either fails, or user presses ctrl-c
        receiver.next().await;
        debug!("exiting");

        Ok::<_, Error>(())
    })?;

    drop(rt);

    if let Err(e) = temp_dir.close() {
        error!("temp_dir: {}", e);
    }

    Ok(())
}
