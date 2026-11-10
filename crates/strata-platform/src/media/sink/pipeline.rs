//! The GStreamer pipeline: appsrc ! audioconvert ! audioresample ! volume ! the output.

use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app::{AppSrc, AppStreamType};

use super::{MediaError, Output, SinkOptions};
use crate::media::format::PcmFormat;

#[derive(Clone)]
pub(super) struct Parts {
    pub(super) pipeline: gst::Pipeline,
    pub(super) src: AppSrc,
    /// The output element, whose position is what has been heard.
    pub(super) sink: gst::Element,
    volume: gst::Element,
}

fn make(factory: &'static str) -> Result<gst::Element, MediaError> {
    gst::ElementFactory::make(factory)
        .build()
        .map_err(|_| MediaError::MissingElement(factory))
}

impl Parts {
    pub(super) fn build(format: &PcmFormat, options: &SinkOptions) -> Result<Parts, MediaError> {
        let pipeline = gst::Pipeline::new();
        #[cfg(test)]
        if let Some(clock) = &options.clock {
            pipeline.use_clock(Some(clock));
        }
        let src = gst::ElementFactory::make("appsrc")
            .build()
            .map_err(|_| MediaError::MissingElement("appsrc"))?
            .downcast::<AppSrc>()
            .map_err(|_| MediaError::Pipeline("appsrc isn't an AppSrc".into()))?;
        src.set_caps(Some(
            &gst::Caps::builder("audio/x-raw")
                .field("format", format.sample.gst_name())
                .field("layout", "interleaved")
                .field("rate", i32::try_from(format.rate).unwrap_or(i32::MAX))
                .field("channels", i32::from(format.channels))
                .build(),
        ));
        src.set_format(gst::Format::Time);
        src.set_stream_type(AppStreamType::Stream);
        src.set_is_live(false);
        // The sink bounds what's queued by time (SinkOptions::lookahead); appsrc must never
        // refuse below that, so its own byte cap is twice the lookahead's bytes.
        src.set_block(false);
        let lookahead = format.frames_in(options.lookahead) * format.frame_bytes() as u64;
        src.set_max_bytes(lookahead.saturating_mul(2).max(1 << 20));
        let convert = make("audioconvert")?;
        let resample = make("audioresample")?;
        let volume = make("volume")?;
        let sink = match options.output {
            Output::Default => make("autoaudiosink")?,
            Output::Null => {
                let sink = make("fakesink")?;
                sink.set_property("sync", true);
                sink
            }
        };
        let elements = [src.upcast_ref(), &convert, &resample, &volume, &sink];
        pipeline
            .add_many(elements)
            .map_err(|e| MediaError::Pipeline(e.to_string()))?;
        gst::Element::link_many(elements).map_err(|e| MediaError::Pipeline(e.to_string()))?;
        Ok(Parts {
            pipeline,
            src,
            sink,
            volume,
        })
    }

    pub(super) fn apply_gain(&self, volume: f64, muted: bool) {
        self.volume.set_property("volume", volume);
        self.volume.set_property("mute", muted);
    }

    /// Whether `message` came from the output element or one inside it (autoaudiosink's
    /// chosen device sink).
    pub(super) fn posted_by_output(&self, message: &gst::Message) -> bool {
        message.src().is_some_and(|src| {
            src == self.sink.upcast_ref::<gst::Object>() || src.has_as_ancestor(&self.sink)
        })
    }
}
