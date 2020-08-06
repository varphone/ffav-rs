use super::{owned::*, AVResult};
use crate::ffi::{AVCodecID::*, AVFieldOrder::*, AVMediaType::*, AVPixelFormat::*, *};
use std::convert::TryInto;
use std::path::Path;

/// Trait for Media Description.
pub trait MediaDesc {
    /// Returns the CodecID.
    fn codec_id(&self) -> AVCodecID {
        Default::default()
    }

    /// Cast to AudioDesc reference.
    fn as_audio_desc(&self) -> Option<&AudioDesc> {
        None
    }

    /// Cast to VideoDesc reference.
    fn as_video_desc(&self) -> Option<&VideoDesc> {
        None
    }
}

/// Audio Description
#[derive(Copy, Clone, Debug, Default)]
pub struct AudioDesc {
    pub codec_id: AVCodecID,
    pub sample_fmt: AVSampleFormat,
    pub bit_rate: i64,
    pub sample_rate: usize,
    pub channels: usize,
}

impl MediaDesc for AudioDesc {
    fn codec_id(&self) -> AVCodecID {
        self.codec_id
    }
}

impl AudioDesc {
    pub fn new() -> Self {
        Default::default()
    }
}

/// Video Description
#[derive(Copy, Clone, Debug, Default)]
pub struct VideoDesc {
    pub codec_id: AVCodecID,
    pub width: i32,
    pub height: i32,
    pub bit_rate: i64,
    pub time_base: AVRational,
    pub gop_size: i32,
    pub pix_fmt: AVPixelFormat,
}

impl MediaDesc for VideoDesc {
    fn codec_id(&self) -> AVCodecID {
        self.codec_id
    }
    fn as_video_desc(&self) -> Option<&VideoDesc> {
        Some(self)
    }
}

impl VideoDesc {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_h264(width: i32, height: i32, bit_rate: i64, time_unit: i32) -> Self {
        Self {
            codec_id: AV_CODEC_ID_H264,
            width,
            height,
            bit_rate,
            time_base: AVRational::with_normalize(time_unit),
            gop_size: 12,
            pix_fmt: AV_PIX_FMT_YUV420P,
        }
    }

    pub fn with_h265(width: i32, height: i32, bit_rate: i64, time_unit: i32) -> Self {
        Self {
            codec_id: AV_CODEC_ID_HEVC,
            width,
            height,
            bit_rate,
            time_base: AVRational::with_normalize(time_unit),
            gop_size: 12,
            pix_fmt: AV_PIX_FMT_YUV420P,
        }
    }
}

/// Stream Information
#[derive(Debug)]
pub struct Stream {
    stream: AVStreamOwned,
    in_time_base: AVRational,
}

/// Simple Writer for Muxing Audio and Video.
#[derive(Debug)]
pub struct SimpleWriter {
    ctx: AVFormatContextOwned,
    streams: Vec<Stream>,
    header_writed: bool,
    trailer_writed: bool,
}

impl Drop for SimpleWriter {
    fn drop(&mut self) {
        if !self.trailer_writed {
            self.ctx.write_trailer().unwrap();
            self.trailer_writed = true;
        }
    }
}

impl SimpleWriter {
    /// Create a new simple writer.
    /// # Arguments
    /// * `path` - Path of the output file.
    /// * `descs` - Media description of input streams.
    /// * `format` - The format to muxing，like: mp4, mpegts.
    pub fn new<P>(path: P, descs: &[&dyn MediaDesc], format: Option<&str>) -> AVResult<Self>
    where
        P: AsRef<Path> + Sized,
    {
        let mut ctx = AVFormatContextOwned::with_output(path, format, None)?;
        let mut streams: Vec<Stream> = vec![];
        for desc in descs {
            let codec_id = desc.codec_id();
            match codec_id {
                AV_CODEC_ID_H264 | AV_CODEC_ID_HEVC => {
                    let desc = desc.as_video_desc().unwrap();
                    let mut st = ctx.new_stream(codec_id)?;
                    // st.time_base = AVRational::new(1, 90000);
                    let par = st.codecpar_mut();
                    par.codec_type = AVMEDIA_TYPE_VIDEO;
                    par.codec_id = codec_id;
                    par.bit_rate = desc.bit_rate;
                    par.width = desc.width;
                    par.height = desc.height;
                    par.field_order = AV_FIELD_UNKNOWN;
                    par.sample_aspect_ratio = AVRational::new(0, 1);
                    par.profile = FF_PROFILE_UNKNOWN;
                    par.level = FF_LEVEL_UNKNOWN;
                    streams.push(Stream {
                        stream: st,
                        in_time_base: desc.time_base,
                    });
                }
                _ => {}
            }
        }
        Ok(Self {
            ctx,
            streams,
            header_writed: false,
            trailer_writed: false,
        })
    }

    /// Write frame bytes to the stream.
    /// # Arguments
    /// * `bytes` - Stream byte data.
    /// * `pts` - Timestamp of the frame.
    /// * `duration` - Duration of the frame.
    /// * `stream_index` - Index of the stream.
    pub fn write_bytes(
        &mut self,
        bytes: &[u8],
        pts: i64,
        duration: i64,
        stream_index: usize,
    ) -> AVResult<()> {
        if !self.header_writed {
            self.ctx.write_header()?;
            self.header_writed = true;
        }
        unsafe {
            let stm = self.streams.get(stream_index).unwrap();
            let in_time_base = stm.in_time_base;
            let out_time_base = stm.stream.time_base;
            let mut pkt = AVPacket::default();
            let pts = av_rescale_q_rnd(
                pts,
                in_time_base,
                out_time_base,
                AVRounding::new().near_inf().pass_min_max(),
            );
            pkt.pts = pts;
            pkt.dts = pts;
            pkt.data = bytes.as_ptr() as *mut u8;
            pkt.size = bytes.len().try_into()?;
            pkt.stream_index = stream_index.try_into()?;
            pkt.flags = 0;
            pkt.duration = av_rescale_q(duration, in_time_base, out_time_base);
            pkt.pos = -1;
            self.ctx.write_frame_interleaved(&mut pkt)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_writer() {
        let a_desc = AudioDesc::new();
        let v_desc = VideoDesc::with_h264(352, 288, 4000, 1000000);
        let example_bytes = include_bytes!("../../examples/envivio-352x288.264.framed");
        for _ in 0..100 {
            let mut mp4_writer =
                SimpleWriter::new("/tmp/envivio-352x288.264.mp4", &[&a_desc, &v_desc], None)
                    .unwrap();
            let mut ts_writer = SimpleWriter::new(
                "/tmp/envivio-352x288.264.ts",
                &[&a_desc, &v_desc],
                Some("mpegts"),
            )
            .unwrap();
            let mut offset: usize = 0;
            let mut pts = 0;
            while offset + 4 < example_bytes.len() {
                let size_bytes = &example_bytes[offset..offset + 4];
                let frame_size = i32::from_be_bytes(size_bytes.try_into().unwrap()) as usize;
                offset += 4;
                let frame_bytes = &example_bytes[offset..offset + frame_size];
                offset += frame_size;
                mp4_writer.write_bytes(frame_bytes, pts, 40000, 0).unwrap();
                ts_writer.write_bytes(frame_bytes, pts, 40000, 0).unwrap();
                pts += 40000;
            }
        }
    }
}
