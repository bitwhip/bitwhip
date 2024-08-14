use std::time::Instant;

use anyhow::Result;
use bytes::Bytes;

pub enum SourceOutput {
    RawFrame(ffmpeg_next::frame::video::Video),
    EncodedFrame(EncodedFrame)
}

pub struct EncodedFrame {
    pub start_time: Instant,
    pub data: Bytes
}

#[cfg(target_os = "windows")]
pub mod dxdup;

#[cfg(target_os = "windows")]
pub mod rhino;

pub trait Source {
    fn get_frame(&mut self) -> Result<SourceOutput>;
}
