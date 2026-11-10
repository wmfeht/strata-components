//! The PCM a sink takes: interleaved samples, a rate and a channel count.

use std::fmt;
use std::time::Duration;

/// One sample's encoding, little-endian.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SampleFormat {
    /// Signed 16-bit integers.
    S16Le,
    /// Signed 32-bit integers.
    S32Le,
    /// 32-bit floats, nominally in -1.0..=1.0.
    F32Le,
}

impl SampleFormat {
    /// Bytes in one sample of one channel.
    pub const fn bytes(self) -> usize {
        match self {
            SampleFormat::S16Le => 2,
            SampleFormat::S32Le | SampleFormat::F32Le => 4,
        }
    }

    /// GStreamer's name for it, in `audio/x-raw` caps.
    pub(crate) const fn gst_name(self) -> &'static str {
        match self {
            SampleFormat::S16Le => "S16LE",
            SampleFormat::S32Le => "S32LE",
            SampleFormat::F32Le => "F32LE",
        }
    }
}

/// Interleaved PCM: `channels` samples per frame, `rate` frames a second.
///
/// The sink converts it to whatever the device takes, so any rate and channel count works.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PcmFormat {
    pub rate: u32,
    pub channels: u16,
    pub sample: SampleFormat,
}

impl PcmFormat {
    /// Stereo 16-bit at 48 kHz.
    pub const STEREO_S16_48K: PcmFormat = PcmFormat {
        rate: 48_000,
        channels: 2,
        sample: SampleFormat::S16Le,
    };

    /// Bytes in one frame: one sample for each channel.
    pub const fn frame_bytes(&self) -> usize {
        self.sample.bytes() * self.channels as usize
    }

    /// How long `frames` frames play.
    pub fn duration_of(&self, frames: u64) -> Duration {
        let nanos = u128::from(frames) * 1_000_000_000 / u128::from(self.rate.max(1));
        Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX))
    }

    /// The whole frames that play within `time`.
    pub fn frames_in(&self, time: Duration) -> u64 {
        let frames = time.as_nanos() * u128::from(self.rate) / 1_000_000_000;
        u64::try_from(frames).unwrap_or(u64::MAX)
    }

    pub(crate) fn validate(&self) -> Result<(), FormatError> {
        if self.rate == 0 || self.rate > 768_000 {
            return Err(FormatError(format!("a rate of {} Hz", self.rate)));
        }
        if self.channels == 0 || self.channels > 64 {
            return Err(FormatError(format!("{} channels", self.channels)));
        }
        Ok(())
    }
}

/// A [`PcmFormat`] the sink can't play.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatError(String);

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unsupported PCM format: {}", self.0)
    }
}

impl std::error::Error for FormatError {}
