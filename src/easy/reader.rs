use super::{owned::*, AVResult};
use crate::ffi::{AVCodecID::*, AVFieldOrder::*, AVMediaType::*, AVPixelFormat::*, *};
use std::convert::TryInto;
use std::fmt::Debug;
use std::path::Path;

pub struct FrameIter<'a> {
    reader: &'a mut SimpleReader,
}

impl<'a> Iterator for FrameIter<'a> {
    type Item = AVPacketOwned;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.read_frame()
    }
}

impl<'a> FrameIter<'a> {
    pub fn new(reader: &'a mut SimpleReader) -> Self {
        Self { reader }
    }
}

/// Simple Reader for Demuxing Media Files.
#[derive(Debug)]
pub struct SimpleReader {
    ctx: AVFormatContextOwned,
    bsfs: Vec<AVBSFContextOwned>,
}

impl SimpleReader {
    const AV_CODEC_TAG_AVC1: u32 = MKTAG!('a','v','c','1') as u32;
    const AV_CODEC_TAG_HEV1: u32 = MKTAG!('h','e','v','1') as u32;

    /// Create a new simple reader.
    /// # Arguments
    /// * `path` - Path of the input file.
    /// * `format_options` - The options for demuxing format，like: movfragement.
    ///
    /// # Panics
    ///
    pub fn open<P>(path: P, format_options: Option<&str>) -> AVResult<Self>
    where
        P: AsRef<Path> + Sized,
    {
        let mut ctx = AVFormatContextOwned::with_input(path, format_options)?;
        let mut bsfs: Vec<AVBSFContextOwned> = vec![];
        for stream in ctx.streams() {
            if let Some(codec) = stream.codec() {
                let filter_name = match codec.codec_tag {
                    Self::AV_CODEC_TAG_AVC1 => { "h264_mp4toannexb"} ,
                    Self::AV_CODEC_TAG_HEV1 => { "hevc_mp4toannexb" },
                    _ => { "null" },
                };
                println!("codec_tag={:08X?}, filter_name={:?}",codec.codec_tag,  filter_name);
                let mut bsf = AVBSFContextOwned::new(filter_name)?;
                bsf.prepare(stream.codecpar())?;
                bsfs.push(bsf);
            }
        }
        Ok(Self { ctx, bsfs })
    }

    /// Return the next frame of a stream.
    pub fn read_frame(&mut self) -> Option<AVPacketOwned> {
        loop {
            for mut bsf in self.bsfs.iter_mut() {
                // println!("bsf={:?}", bsf);
                match bsf.receive_packet() {
                    Ok(packet) => {
                        // println!("Get packet={:?}", packet);
                        return Some(packet);
                    }
                    Err(err) => match err {
                        AVBSFError::Again => {}
                        AVBSFError::Reason(_) => {}
                    },
                }
            }
            if let Some(mut packet) = self.ctx.read_frame() {
                // println!("readed frame!");
                let r = self.bsfs[packet.stream_index as usize].send_packet(&mut packet);
                if r.is_err() {
                    return None;
                }
            } else {
                return None;
            }
        }
    }

    pub fn frames(&mut self) -> FrameIter<'_> {
        FrameIter::new(self)
    }

    pub fn streams(&self) -> &[&AVStream] {
        self.ctx.streams()
    }
}
