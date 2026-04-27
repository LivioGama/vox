use std::io;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::clone;

pub const RATE: u32 = 16_000;
pub const FRAME_MS: u32 = 30;
pub const FRAME_SAMPLES: usize = 480;
pub const FRAME_BYTES: usize = 960;

static TEMP_WAV_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct RecChild {
    child: Child,
    stdout: ChildStdout,
}

impl RecChild {
    pub fn spawn() -> Result<Self> {
        let mut child = std::process::Command::new("rec")
            .args([
                "--no-show-progress",
                "-r",
                "16000",
                "-c",
                "1",
                "-t",
                "raw",
                "-e",
                "signed-integer",
                "-b",
                "16",
                "-",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context(clone::sox_install_hint())?;
        let stdout = child.stdout.take().context("sox stdout missing")?;
        Ok(Self { child, stdout })
    }

    pub fn read_frame(&mut self, buf: &mut [u8; FRAME_BYTES]) -> io::Result<usize> {
        let mut read = 0;
        while read < FRAME_BYTES {
            match self.stdout.read(&mut buf[read..]) {
                Ok(0) => break,
                Ok(n) => read += n,
                Err(e) => return Err(e),
            }
        }
        Ok(read)
    }
}

impl Drop for RecChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn write_wav_i16_mono_16k(path: &Path, samples: &[i16]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("failed to create utterance cache directory")?;
    }

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).context("failed to create WAV")?;
    for sample in samples {
        writer
            .write_sample(*sample)
            .context("failed to write sample")?;
    }
    writer.finalize().context("failed to finalize WAV")?;
    Ok(())
}

pub fn temp_wav_path() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let counter = TEMP_WAV_COUNTER.fetch_add(1, Ordering::Relaxed);
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("vox")
        .join(format!(
            "utterance-{}-{stamp}-{counter}.wav",
            std::process::id()
        ))
}

#[cfg(test)]
mod tests {
    use super::temp_wav_path;

    #[test]
    fn temp_wav_paths_are_unique() {
        let first = temp_wav_path();
        let second = temp_wav_path();
        assert_ne!(first, second);
    }
}
