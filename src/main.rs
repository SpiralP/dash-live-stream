mod error;
mod logger;
mod web;

use crate::error::*;
use clap::{crate_name, crate_version, App, Arg};
use lazy_static::lazy_static;
use log::*;
use std::{
    net::{IpAddr, SocketAddr},
    process::{Child, Command},
    sync::Mutex,
    time::Duration,
};
use tempdir::TempDir;

lazy_static! {
    static ref TEMP_DIR: Mutex<Option<TempDir>> = Default::default();
}

lazy_static! {
    static ref COMMAND: Mutex<Option<Child>> = Default::default();
}

#[tokio::main]
async fn main() -> Result<()> {
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

    let tls = matches.is_present("tls");

    let temp_dir = TempDir::new(env!("CARGO_PKG_NAME"))?;
    let temp_dir_path = temp_dir.path().to_owned();
    debug!("created temp dir {:?}", temp_dir_path);

    {
        let mut maybe_temp_dir = TEMP_DIR.lock().unwrap();
        *maybe_temp_dir = Some(temp_dir);
    }

    ctrlc::set_handler(move || {
        cleanup();
    })
    .expect("Error setting Ctrl-C handler");

    {
        let temp_dir_path = temp_dir_path.clone();
        tokio::spawn(async move {
            if let Err(e) =
                web::start(SocketAddr::new(http_ip, http_port), temp_dir_path, tls).await
            {
                error!("web: {}", e);
            }
        });
    }

    let framerate = matches.value_of("framerate").unwrap();

    let video_bitrate = matches.value_of("video-bitrate").unwrap();
    let video_resolution = matches.value_of("video-resolution").unwrap();

    let audio_sample_rate = matches.value_of("audio-sample-rate").unwrap();
    let audio_bitrate = matches.value_of("audio-bitrate").unwrap();

    let cpu_used = matches.value_of("cpu-used").unwrap();
    let crf = matches.value_of("crf").unwrap();

    let path = "stream";
    let stream_key = "";
    let rtmp_addr = format!("rtmp://{}:{}/{}/{}", rtmp_ip, rtmp_port, path, stream_key);
    let num_threads = format!("{}", num_cpus::get());
    let args = vec![
        "-hide_banner",
        "-loglevel",
        "warning",
        "-stats",
        "-listen",
        "1",
        "-i",
        &rtmp_addr,
        // video
        "-c:v",
        "libvpx-vp9",
        // https://developers.google.com/media/vp9/live-encoding
        "-quality",
        "realtime",
        "-cpu-used",
        &cpu_used,
        "-tile-columns",
        "4",
        "-frame-parallel",
        "1",
        "-threads",
        &num_threads,
        "-static-thresh",
        "0",
        "-max-intra-rate",
        "300",
        "-lag-in-frames",
        "0",
        "-qmin",
        "4",
        "-qmax",
        "48",
        "-row-mt",
        "1",
        "-error-resilient",
        "1",
        //
        "-r",
        &framerate,
        "-crf",
        &crf,
        "-b:v",
        &video_bitrate,
        "-s",
        &video_resolution,
        // at least 1 keyframe per second
        "-keyint_min",
        "60",
        "-g",
        "60",
        // audio
        "-c:a",
        "libvorbis",
        "-b:a",
        &audio_bitrate,
        "-ar",
        &audio_sample_rate,
        "-ac",
        "2",
        // output
        "-f",
        "dash",
        "-remove_at_exit",
        "1",
        "-dash_segment_type",
        "webm",
        "-window_size",
        "5",
        "-extra_window_size",
        "2",
        "-utc_timing_url",
        "https://time.akamai.com/",
        "-use_timeline",
        "0",
        "-use_template",
        "1",
        "-seg_duration",
        "3",
        "-index_correction",
        "1",
        "-ignore_io_errors",
        "1",
        "stream.mpd",
    ];

    debug!("ffmpeg {:?}", args);

    let command = Command::new("ffmpeg")
        .current_dir(&temp_dir_path)
        .args(args)
        .spawn()?;

    {
        let mut maybe_command = COMMAND.lock().unwrap();
        *maybe_command = Some(command);
    }

    loop {
        tokio::time::delay_for(Duration::from_secs(1)).await;

        {
            let mut maybe_command = COMMAND.lock().unwrap();
            if let Some(command) = maybe_command.as_mut() {
                match command.try_wait() {
                    Ok(Some(status)) => {
                        debug!("ffmpeg exited with: {}", status);
                        break;
                    }

                    Ok(None) => {
                        // still running
                    }

                    Err(e) => {
                        error!("ffmpeg error attempting to wait: {}", e);
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    cleanup();

    Ok(())
}

fn cleanup() {
    {
        let mut maybe_command = COMMAND.lock().unwrap();
        if let Some(mut command) = maybe_command.take() {
            let _ignore = command.kill();
            if let Err(e) = command.wait() {
                error!("command.wait(): {}", e);
            }
        }
    }

    // gross, for windows
    std::thread::sleep(Duration::from_secs(1));

    {
        let mut maybe_temp_dir = TEMP_DIR.lock().unwrap();
        if let Some(temp_dir) = maybe_temp_dir.take() {
            if let Err(e) = temp_dir.close() {
                error!("temp_dir: {}", e);
            }
        }
    }
}
