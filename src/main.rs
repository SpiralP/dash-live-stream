mod error;
mod ffmpeg;
mod logger;
mod web;

use self::ffmpeg::Ffmpeg;
use crate::error::*;
use clap::{crate_name, crate_version, App, Arg};
use futures::{channel::mpsc, stream::StreamExt};
use log::*;
use std::net::{IpAddr, SocketAddr};
use tempdir::TempDir;
use tokio::runtime::Runtime;

fn main() -> Result<()> {
    logger::initialize(cfg!(debug_assertions), false);

    let matches = App::new(crate_name!())
        .version(crate_version!())
        .about("Does awesome things")
        .arg(
            Arg::with_name("rtmp-ip")
                .long("rtmp-ip")
                .value_name("ADDRESS")
                .help("Sets the listen ip address for rtmp")
                .takes_value(true)
                .default_value("127.0.0.1"),
        )
        .arg(
            Arg::with_name("rtmp-port")
                .short("r")
                .long("rtmp-port")
                .value_name("PORT")
                .help("Sets the listen rtmp port")
                .takes_value(true)
                .default_value("1935"),
        )
        .arg(
            Arg::with_name("http-ip")
                .short("i")
                .long("http-ip")
                .value_name("ADDRESS")
                .help("Sets the listen ip address for http")
                .takes_value(true)
                .default_value("127.0.0.1"),
        )
        .arg(
            Arg::with_name("http-port")
                .short("p")
                .long("http-port")
                .value_name("PORT")
                .help("Sets the listen http port")
                .takes_value(true)
                .default_value("3000"),
        )
        .arg(
            Arg::with_name("tls")
                .short("s")
                .long("tls")
                .alias("ssl")
                .alias("https")
                .help("Use secured https"),
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
                .takes_value(true)
                .default_value("5"),
        )
        .arg(
            Arg::with_name("video-resolution")
                .long("resolution")
                .help("Sets resolution of the output video")
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
                .takes_value(true)
                .default_value("30"),
        )
        .arg(
            Arg::with_name("framerate")
                .long("framerate")
                .help("Sets the framerate of the output video")
                .takes_value(true)
                .default_value("30"),
        )
        .arg(
            Arg::with_name("audio-sample-rate")
                .long("audio-sample-rate")
                .help("Sets the sample rate of the output audio")
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
                .takes_value(true)
                .default_value("128k"),
        )
        .get_matches();

    let http_ip: IpAddr = matches.value_of("http-ip").unwrap().parse()?;
    let http_port: u16 = matches.value_of("http-port").unwrap().parse()?;

    let rtmp_ip: IpAddr = matches.value_of("rtmp-ip").unwrap().parse()?;
    let rtmp_port: u16 = matches.value_of("rtmp-port").unwrap().parse()?;

    let framerate = matches.value_of("framerate").unwrap().parse()?;

    let video_bitrate = matches.value_of("video-bitrate").unwrap().to_string();
    let video_resolution = matches.value_of("video-resolution").unwrap().to_string();

    let audio_sample_rate = matches.value_of("audio-sample-rate").unwrap().to_string();
    let audio_bitrate = matches.value_of("audio-bitrate").unwrap().to_string();

    let cpu_used = matches.value_of("cpu-used").unwrap().parse()?;
    let crf = matches.value_of("crf").unwrap().parse()?;

    let tls = matches.is_present("tls");

    let temp_dir = TempDir::new(env!("CARGO_PKG_NAME"))?;
    let temp_dir_path = temp_dir.path().to_owned();
    debug!("created temp dir {:?}", temp_dir_path);

    let mut rt = Runtime::new()?;
    rt.block_on(async move {
        let (sender, mut receiver) = mpsc::unbounded();

        {
            let sender = sender.clone();
            ctrlc::set_handler(move || {
                let _ignore = sender.unbounded_send(());
            })
            .expect("Error setting Ctrl-C handler");
        }

        {
            let temp_dir_path = temp_dir_path.clone();
            let sender = sender.clone();

            tokio::spawn(async move {
                if let Err(e) =
                    web::start(SocketAddr::new(http_ip, http_port), temp_dir_path, tls).await
                {
                    error!("web: {}", e);
                }
                let _ignore = sender.unbounded_send(());
            });
        }

        {
            let mut ffmpeg = Ffmpeg {
                command: None,
                rtmp_ip,
                rtmp_port,
                cpu_used,
                framerate,
                crf,
                video_bitrate,
                video_resolution,
                audio_bitrate,
                audio_sample_rate,
                temp_dir_path,
            };

            tokio::spawn(async move {
                if let Err(e) = ffmpeg.run().await {
                    error!("ffmpeg: {}", e);
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
