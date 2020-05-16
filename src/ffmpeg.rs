use crate::error::*;
use log::*;
use std::{
    net::IpAddr,
    path::PathBuf,
    process::{Child, Command},
    thread,
    time::Duration,
};

pub struct Ffmpeg {
    pub command: Option<Child>,
    pub rtmp_ip: IpAddr,
    pub rtmp_port: u16,
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
        let path = "stream";
        let stream_key = "";
        let rtmp_addr = format!(
            "rtmp://{}:{}/{}/{}",
            self.rtmp_ip, self.rtmp_port, path, stream_key
        );
        let cpu_used = format!("{}", self.cpu_used);
        let num_threads = format!("{}", num_cpus::get());
        let framerate = format!("{}", self.framerate);
        let crf = format!("{}", self.crf);
        let video_bitrate = &self.video_bitrate;
        let video_resolution = &self.video_resolution;
        let audio_bitrate = &self.audio_bitrate;
        let audio_sample_rate = &self.audio_sample_rate;
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
        ];

        debug!("ffmpeg {}", args.join(" "));

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
