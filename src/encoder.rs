use anyhow::{anyhow, bail, Context, Result};
use ffmpeg::ffi::AVCodecContext;
use ffmpeg::{codec::Context as CodecContext, encoder::Video, Frame, Packet};
use ffmpeg_next as ffmpeg;
use log::info;
use std::{
    collections::HashMap,
    ffi::{c_void, CString},
};

pub struct Encoder {
    encoder: Video,
    dimensions: (u32, u32),
}

impl Encoder {
    pub fn new<F>(
        encoder: &str,
        encoder_options: Option<HashMap<String, String>>,
        setting_func: F,
    ) -> Result<Self>
    where
        F: FnOnce(&mut ffmpeg::encoder::video::Video) -> Result<()>,
    {
        let codec = ffmpeg::encoder::find_by_name(encoder)
            .ok_or_else(|| anyhow!("Missing encoder {}", encoder))?;

        let codec_context = CodecContext::new_with_codec(codec);

        let mut encoder = codec_context.encoder().video()?;

        setting_func(&mut encoder)?;

        let dimensions = (encoder.width(), encoder.height());

        if let Some(encoder_options) = encoder_options {
            for (key, value) in encoder_options.iter() {
                info!("Setting option {key} {value}");
                unsafe { Self::set_option(encoder.as_mut_ptr(), &key, &value)? };
            }
        }

        Ok(Encoder {
            encoder: encoder.open()?,
            dimensions,
        })
    }

    pub fn encode(&mut self, frame: &Frame) -> Result<Option<Packet>> {
        self.encoder.send_frame(frame)?;

        let mut packet = Packet::empty();
        if self.encoder.receive_packet(&mut packet).is_ok() {
            return Ok(Some(packet));
        }

        Ok(None)
    }

    unsafe fn set_option(context: *mut AVCodecContext, name: &str, val: &str) -> Result<()> {
        let name_c = CString::new(name).context("Error in CString")?;
        let val_c = CString::new(val).context("Error in CString")?;
        let retval: i32 = ffmpeg::ffi::av_opt_set(
            context as *mut c_void,
            name_c.as_ptr(),
            val_c.as_ptr(),
            ffmpeg::ffi::AV_OPT_SEARCH_CHILDREN,
        );
        if retval != 0 {
            bail!("set_option failed: {retval}");
        }
        Ok(())
    }

    pub fn dimensions(&self) -> (u32, u32) {
        return self.dimensions;
    }
}
