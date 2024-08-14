use super::{Source, SourceOutput};
use anyhow::{anyhow, Result};
use bytes::Bytes;
use dxfilter::ConvertARGBToNV12;
use rhinostream::{
    filter::{new_nv12_filter, DxColor},
    processor::nvenc::{NvEnc, NvencConfig},
    source::DxDesktopDuplication,
    stream::{RhinoStream, SignalType},
    Context, Packet,
};

pub struct Rhino {
    stream: RhinoStream<DxDesktopDuplication, DxColor<ConvertARGBToNV12>, NvEnc>,
    current_frame: u64,
}

impl Rhino {
    pub fn new() -> Result<Self> {
        let mut ctx = Context::None;
        let src = DxDesktopDuplication::new("--screen 0".parse().unwrap(), &mut ctx).unwrap();
        let filter = new_nv12_filter("-c rgb -r 1920x1080".parse().unwrap(), &mut ctx).unwrap();
        let config: NvencConfig = "-p p1 --profile auto --multi-pass disabled --aq disabled -t \
            ultra_low_latency -r 1920x1080 --codec h264 --color argb -b 10000000 -f 60"
            .parse()
            .unwrap();
        let processor = NvEnc::new(&mut ctx, &config).unwrap();

        Ok(Self {
            stream: RhinoStream::new(src, filter, processor).unwrap(),
            current_frame: 0u64,
        })
    }
}

impl Source for Rhino {
    fn get_frame(&mut self) -> Result<SourceOutput> {
        let mut packet = Packet::new();
        self.stream
            .get_next_frame(&mut packet)
            .map_err(|e| anyhow!("Failed to retrieve frame: {e:?}"))?;

        self.current_frame += 1;

        // Signal IDR roughly every 60 frames
        if self.current_frame % 120 == 0 {
            let _ = self.stream.signal(SignalType::Processor(1));
        }

        Ok(SourceOutput::EncodedFrame(super::EncodedFrame {
            start_time: packet.start_time,
            data: Bytes::from(packet.data),
        }))
    }
}
