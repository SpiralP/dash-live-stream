use crate::{error::*, helpers::*};
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
    File {
        path: PathBuf,
        seek: Option<Duration>,
    },
}

pub enum FfmpegOutput {
    // temp_dir_path
    Dash(PathBuf),
    Rtmp(SocketAddr),
}

pub struct Ffmpeg {
    pub command: Option<Child>,
    pub verbose: bool,

    pub input: FfmpegInput,
    pub output: FfmpegOutput,

    pub cpu_used: u8,
    pub framerate: u8,
    pub crf: u8,
    pub video_bitrate: String,
    pub video_resolution: String,
    pub audio_bitrate: String,
    pub audio_sample_rate: String,
    pub subtitles_path: Option<PathBuf>,
}

impl Ffmpeg {
    pub async fn run(&mut self) -> Result<()> {
        let stream_path = "stream";
        let stream_key = "";

        let mut args: Vec<String> = Vec::new();

        macro_rules! append {
            ( $vec:ident, $( $item:expr ),* $(,)* ) => {
                $(
                    $vec.push($item.into());
                )*
            };
        }

        if !self.verbose {
            append!(args, "-hide_banner", "-loglevel", "warning", "-stats");
        }

        match &self.input {
            FfmpegInput::Rtmp(addr) => {
                let rtmp_addr = format!("rtmp://{}/{}/{}", addr, stream_path, stream_key);
                append!(args, "-listen", "1", "-i", rtmp_addr);
            }

            FfmpegInput::File { path, seek } => {
                let absolute_path = get_absolute_path(path);
                let path = format!("{}", absolute_path.display());

                // play at 1x speed
                append!(args, "-re");

                if let Some(seek) = seek {
                    append!(args, "-ss", format!("{}", seek.as_secs_f32()));
                }

                append!(args, "-i", &path);
            }
        }

        // subtitles
        if let Some(subtitles_path) = &self.subtitles_path {
            let absolute_path = get_absolute_path(subtitles_path);
            // https://superuser.com/questions/1247197/ffmpeg-absolute-path-error
            let escaped_path = format!("{}", absolute_path.display())
                .replace(r"\", r"\\\\")
                .replace(r":", r"\\:");

            let seek = if let FfmpegInput::File { path: _, seek } = &self.input {
                *seek
            } else {
                None
            };

            if let Some(seek) = seek {
                append!(
                    args,
                    "-vf",
                    format!(
                        "setpts=PTS+{}/TB,subtitles=filename={},setpts=PTS-STARTPTS",
                        seek.as_secs_f32(),
                        escaped_path
                    )
                );
            } else {
                append!(args, "-vf", format!("subtitles=filename={}", escaped_path));
            }
        }

        match &self.output {
            FfmpegOutput::Dash(_temp_dir_path) => {
                // conversion to vp9/vorbis
                append!(
                    args,
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
                    // at least 1 keyframe every 2 seconds
                    // to match our chunk duration of 2 seconds
                    // if this duration is longer than our chunk size,
                    // dash breaks weirdly where video doesn't encode fast enough
                    "-g",
                    &format!("{}", self.framerate * 2),
                    // audio
                    "-c:a",
                    // TODO why not opus?
                    "libvorbis",
                    "-b:a",
                    &self.audio_bitrate,
                    "-ar",
                    &self.audio_sample_rate,
                    // # audio channels
                    "-ac",
                    "2",
                );

                // output
                append!(
                    args,
                    "-f",
                    "dash",
                    // remove chunk files at exit
                    "-remove_at_exit",
                    "1",
                    "-dash_segment_type",
                    "webm",
                    // 5 chunk files in the manifest, 10 seconds of media in the manifest
                    "-window_size",
                    "5",
                    // 2 extra chunk files not in the manifest, 4 extra seconds
                    // before getting deleted
                    "-extra_window_size",
                    "2",
                    "-utc_timing_url",
                    "https://time.akamai.com/",
                    // template will use media="chunk-stream$RepresentationID$-$Number%05d$.webm"
                    // so the client knows where all the files are without fetching manifest again
                    // we don't want to use template because then the client will expect segments
                    // that might not exist because of a slow encoder
                    "-use_template",
                    "0",
                    // if template isn't used, timeline isn't used
                    "-use_timeline",
                    "0",
                    // 2 seconds each chunk file
                    // using 1 second causes issues:
                    // Correcting the segment index after file chunk-stream0-00017.webm: current=18 corrected=19
                    "-seg_duration",
                    "2",
                    "-index_correction",
                    "1",
                    "-ignore_io_errors",
                    "1",
                    // "One or more streams in WebM output format. Streaming option will be ignored"
                    // "-streaming",
                    // "0",
                    // "LDash option will be ignored as streaming is not enabled"
                    // "-ldash",
                    // "0",
                    "stream.mpd",
                );
            }

            FfmpegOutput::Rtmp(addr) => {
                let rtmp_addr = format!("rtmp://{}/{}/{}", addr, stream_path, stream_key);
                append!(args, "-f", "flv", rtmp_addr);
            }
        }

        debug!("ffmpeg {}", args.join(" "));

        match &self.input {
            FfmpegInput::Rtmp(addr) => {
                let rtmp_addr = format!("rtmp://{}/{}/{}", addr, stream_path, stream_key);
                info!("ffmpeg listening for rtmp connections at {}", rtmp_addr);
            }

            FfmpegInput::File { path, seek: _ } => {
                info!("ffmpeg playing from {}", path.display());
            }
        }

        let command = match &self.output {
            FfmpegOutput::Dash(temp_dir_path) => Command::new("ffmpeg")
                .args(args)
                .current_dir(temp_dir_path)
                .spawn()?,
            FfmpegOutput::Rtmp(_addr) => Command::new("ffmpeg").args(args).spawn()?,
        };

        self.command = Some(command);

        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;

            {
                if let Some(command) = self.command.as_mut() {
                    match command.try_wait() {
                        Ok(Some(status)) => {
                            if status.success() {
                                info!("ffmpeg exited with: {}", status);
                                return Ok(());
                            } else {
                                bail!("ffmpeg exited with: {}", status);
                            }
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
