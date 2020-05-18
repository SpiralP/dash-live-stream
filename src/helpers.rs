use crate::error::*;
use std::{
    env,
    path::{Path, PathBuf},
    time::Duration,
};

pub fn get_absolute_path(path: &Path) -> PathBuf {
    if path.is_relative() {
        let mut abs = env::current_dir().unwrap();
        abs.push(path);
        abs
    } else {
        path.to_owned()
    }
}

pub fn parse_duration(input: &str) -> Result<Duration> {
    let parts: Vec<_> = input.split(':').collect();
    let secs = match parts.as_slice() {
        [hours, minutes, seconds] => {
            let hours: u32 = hours.parse()?;
            let minutes: u32 = minutes.parse()?;
            let seconds: f32 = seconds.parse()?;

            seconds + (minutes * 60 + hours * 60 * 60) as f32
        }

        [minutes, seconds] => {
            let minutes: u32 = minutes.parse()?;
            let seconds: f32 = seconds.parse()?;

            seconds + (minutes * 60) as f32
        }

        [seconds] => {
            let seconds: f32 = seconds.parse()?;

            seconds
        }

        _ => {
            bail!("couldn't convert {:?} to seconds", input);
        }
    };

    Ok(Duration::from_secs_f32(secs))
}
