mod error;
mod logger;
mod web;

use crate::error::*;
use clap::{crate_name, crate_version, App, Arg};
use lazy_static::lazy_static;
use log::*;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    process::{Child, Command},
    sync::Mutex,
    time::Duration,
};
use tempdir::TempDir;

const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

lazy_static! {
    static ref TEMP_DIR: Mutex<Option<TempDir>> = Default::default();
}

lazy_static! {
    static ref COMMAND: Mutex<Option<Child>> = Default::default();
}

#[tokio::main]
async fn main() -> Result<()> {
    logger::initialize(cfg!(debug_assertions), false);

    let ip = IP;

    let matches = App::new(crate_name!())
        .version(crate_version!())
        .about("Does awesome things")
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
            Arg::with_name("https-port")
                .short("p")
                .long("https-port")
                .value_name("PORT")
                .help("Sets the listen https port")
                .takes_value(true)
                .default_value("3000"),
        )
        .get_matches();

    let https_port: u16 = matches.value_of("https-port").unwrap().parse()?;
    let rtmp_port: u16 = matches.value_of("rtmp-port").unwrap().parse()?;

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
            if let Err(e) = web::start(SocketAddr::new(ip, https_port), temp_dir_path).await {
                error!("web: {}", e);
            }
        });
    }

    let command = Command::new("ffmpeg")
        .current_dir(&temp_dir_path)
        .args(vec![
            "-hide_banner",
            "-listen",
            "1",
            "-i",
            &format!("rtmp://127.0.0.1:{}/stream", rtmp_port),
            "-c:v",
            "libvpx-vp9",
            "-speed",
            "5",
            "-r",
            "20",
            "-preset",
            "ll",
            "-crf",
            "30",
            "-b:v",
            "2000k",
            "-s",
            "1280x720",
            "-keyint_min",
            "60",
            "-g",
            "60",
            "-tile-columns",
            "4",
            "-frame-parallel",
            "1",
            "-threads",
            "6",
            "-static-thresh",
            "0",
            "-max-intra-rate",
            "300",
            "-quality",
            "realtime",
            "-lag-in-frames",
            "0",
            "-error-resilient",
            "1",
            "-c:a",
            "libvorbis",
            "-b:a",
            "128k",
            "-ar",
            "44100",
            "-ac",
            "2",
            "-f",
            "dash",
            "-remove_at_exit",
            "1",
            "-dash_segment_type",
            "webm",
            "-window_size",
            "5",
            "-extra_window_size",
            "1",
            "-utc_timing_url",
            "http://time.akamai.com/",
            "-use_timeline",
            "1",
            "-use_template",
            "1",
            "-seg_duration",
            "5",
            "stream.mpd",
        ])
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
            if let Err(e) = command.kill() {
                error!("command.kill(): {}", e);
            }
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
