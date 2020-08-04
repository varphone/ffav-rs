use crate::ffi::{
    av_err2str, AVCodecID, AVCodecID::*, AVFormatContext, AVMediaType, AVMediaType::*, AVPacket,
    AVPixelFormat, AVRational, AVSampleFormat,
};
use std::convert::TryInto;
use std::error::Error;
use std::ffi::CString;
use std::ops::{Deref, DerefMut};
use std::path::Path;

use super::owned::*;

pub trait MediaOptions {
    fn codec_id(&self) -> AVCodecID {
        Default::default()
    }

    fn is_audio(&self) -> bool {
        false
    }

    fn is_video(&self) -> bool {
        false
    }

    fn as_audio_options(&self) -> Option<&AudioOptions> {
        None
    }

    fn as_video_options(&self) -> Option<&VideoOptions> {
        None
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct AudioOptions {
    pub codec_id: AVCodecID,
    pub sample_fmt: AVSampleFormat,
    pub bit_rate: usize,
    pub sample_rate: usize,
    pub channels: usize,
}

impl MediaOptions for AudioOptions {
    fn codec_id(&self) -> AVCodecID {
        self.codec_id
    }
}

impl AudioOptions {
    pub fn new() -> Self {
        Default::default()
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct VideoOptions {
    pub codec_id: AVCodecID,
    pub bit_rate: i64,
    pub width: i32,
    pub height: i32,
    pub time_base: AVRational,
    pub gop_size: i32,
    pub pix_fmt: AVPixelFormat,
}

impl MediaOptions for VideoOptions {
    fn codec_id(&self) -> AVCodecID {
        self.codec_id
    }
    fn as_video_options(&self) -> Option<&VideoOptions> {
        Some(self)
    }
}

impl VideoOptions {
    pub fn new() -> Self {
        Default::default()
    }
}

pub struct Mp4Writer {
    ctx: AVFormatContextOwned,
    header_writed: bool,
    trailer_writed: bool,
}

impl Drop for Mp4Writer {
    fn drop(&mut self) {
        println!("impl Drop for Mp4Writer");
        if !self.trailer_writed {
            self.ctx.write_trailer().unwrap();
            self.trailer_writed = true;
        }
    }
}

impl Mp4Writer {
    pub fn new<P: AsRef<Path> + Sized>(
        path: P,
        options: &[&dyn MediaOptions],
    ) -> Result<Self, Box<dyn Error>> {
        let mut ctx = AVFormatContextOwned::with_output(None, "mp4", path)?;
        for o in options {
            let codec_id = o.codec_id();
            match codec_id {
                AV_CODEC_ID_H264 | AV_CODEC_ID_HEVC => {
                    let mut stream = ctx.new_stream(codec_id)?;
                    let cp = stream.codecpar_mut();
                    let vo = o.as_video_options().unwrap();
                    cp.codec_type = AVMEDIA_TYPE_VIDEO;
                    cp.codec_id = codec_id;
                    cp.bit_rate = vo.bit_rate;
                    cp.width = vo.width;
                    cp.height = vo.height;
                }
                _ => {}
            }
        }
        Ok(Self {
            ctx,
            header_writed: false,
            trailer_writed: false,
        })
    }

    pub fn write(&mut self, bytes: &[u8], pts: i64, duration: i64, stream_index: usize) {
        if !self.header_writed {
            self.ctx.write_header().unwrap();
            self.header_writed = true;
        }
        let mut pkt = AVPacket::default();
        pkt.pts = pts;
        pkt.dts = pts;
        pkt.data = bytes.as_ptr() as *mut u8;
        pkt.size = bytes.len().try_into().unwrap();
        pkt.stream_index = stream_index.try_into().unwrap();
        pkt.flags = 0;
        pkt.duration = duration;
        pkt.pos = -1;
        self.ctx.write_frame_interleaved(&mut pkt).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mp4_writer() {
        let a_opts = AudioOptions::new();
        let v_opts = VideoOptions {
            codec_id: AV_CODEC_ID_H264,
            bit_rate: 4000,
            width: 1280,
            height: 720,
            time_base: AVRational {
                num: 1,
                den: 1000000,
            },
            gop_size: 25,
            pix_fmt: AVPixelFormat::AV_PIX_FMT_YUV420P,
        };
        let mut writer = Mp4Writer::new("example.mp4", &[&a_opts, &v_opts]).unwrap();
        writer.write(b"Hello", 0, 40000, 0);
    }
}
