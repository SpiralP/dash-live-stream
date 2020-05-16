use crate::error::*;
use log::*;
use std::{
    net::SocketAddr,
    path::PathBuf,
    process::{Child, Command},
    thread,
    time::Duration,
};

pub enum FfmpegInput {
    Rtmp(SocketAddr),
    File(PathBuf),
}

pub struct Ffmpeg {
    pub command: Option<Child>,

    pub input: FfmpegInput,
    pub cpu_used: u8,
    pub framerate: u8,
    pub crf: u8,
    pub video_bitrate: String,
    pub video_resolution: String,
    pub audio_bitrate: String,
    pub audio_sample_rate: String,
    pub temp_dir_path: PathBuf,
}

impl Ffmpeg {
    pub async fn run(&mut self) -> Result<()> {
        let stream_path = "stream";
        let stream_key = "";

        let mut args: Vec<String> = vec!["-hide_banner", "-loglevel", "warning", "-stats"]
            .iter()
            .map(ToString::to_string)
            .collect();

        match &self.input {
            FfmpegInput::Rtmp(addr) => {
                let rtmp_addr = format!("rtmp://{}/{}/{}", addr, stream_path, stream_key);
                args.append(
                    &mut vec!["-listen", "1", "-i", &rtmp_addr]
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                );
            }

            FfmpegInput::File(path) => {
                let path = format!("{}", path.display());
                args.append(
                    &mut vec!["-re", "-i", &path]
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                );
            }
        }

        args.append(
            &mut vec![
                // video
                "-c:v",
                "libvpx-vp9",
                // https://developers.google.com/media/vp9/live-encoding
                "-quality",
                "realtime",
                "-cpu-used",
                &format!("{}", self.cpu_used),
                "-tile-columns",
                "4",
                "-frame-parallel",
                "1",
                "-threads",
                &format!("{}", num_cpus::get()),
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
                &format!("{}", self.framerate),
                "-crf",
                &format!("{}", self.crf),
                "-b:v",
                &self.video_bitrate,
                "-s",
                &self.video_resolution,
                // at least 1 keyframe per second
                "-keyint_min",
                "60",
                "-g",
                "60",
                // audio
                "-c:a",
                "libvorbis",
                "-b:a",
                &self.audio_bitrate,
                "-ar",
                &self.audio_sample_rate,
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
                "1",
                "-use_template",
                "1",
                "-seg_duration",
                "3",
                "-index_correction",
                "1",
                "-ignore_io_errors",
                "1",
                "stream.mpd",
            ]
            .iter()
            .map(ToString::to_string)
            .collect(),
        );

        debug!("ffmpeg {}", args.join(" "));

        match &self.input {
            FfmpegInput::Rtmp(addr) => {
                let rtmp_addr = format!("rtmp://{}/{}/{}", addr, stream_path, stream_key);
                info!("ffmpeg listening for rtmp connections at {}", rtmp_addr);
            }

            FfmpegInput::File(path) => {
                info!("ffmpeg playing from {}", path.display());
            }
        }

        let command = Command::new("ffmpeg")
            .current_dir(&self.temp_dir_path)
            .args(args)
            .spawn()?;

        self.command = Some(command);

        loop {
            tokio::time::delay_for(Duration::from_millis(500)).await;

            {
                if let Some(command) = self.command.as_mut() {
                    match command.try_wait() {
                        Ok(Some(status)) => {
                            if status.success() {
                                info!("ffmpeg exited with: {}", status);
                            } else {
                                warn!("ffmpeg exited with: {}", status);
                            }

                            return Ok(());
                        }

                        Ok(None) => {
                            // still running
                        }

                        Err(e) => {
                            bail!("ffmpeg error attempting to wait: {}", e);
                        }
                    }
                } else {
                    return Ok(());
                }
            }
        }
    }
}

impl Drop for Ffmpeg {
    fn drop(&mut self) {
        if let Some(mut command) = self.command.take() {
            let _ignore = command.kill();
            if let Err(e) = command.wait() {
                error!("command.wait(): {}", e);
            }

            // gross, windows doesn't really wait here
            // temp folder is still locked so it can't be removed
            thread::sleep(Duration::from_millis(1000));
        }
    }
}
