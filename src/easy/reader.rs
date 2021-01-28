use super::{owned::*, AVResult};
use crate::ffi::*;
use std::fmt::Debug;
use std::path::Path;

#[derive(Copy, Clone, Default, Debug)]
pub struct FrameInfo {
    pub codec_id: AVCodecID,
    pub codec_type: AVMediaType,
}

pub struct FrameIter<'a> {
    reader: &'a mut SimpleReader,
    frame_infos: Vec<FrameInfo>,
}

impl<'a> Iterator for FrameIter<'a> {
    type Item = (AVPacketOwned, FrameInfo);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(frame) = self.reader.read_frame() {
            let stream_index = frame.stream_index as usize;
            Some((frame, self.frame_infos[stream_index]))
        } else {
            None
        }
    }
}

impl<'a> FrameIter<'a> {
    pub fn new(reader: &'a mut SimpleReader) -> Self {
        let frame_infos: Vec<FrameInfo> = reader
            .streams()
            .iter()
            .map(|stream| {
                if let Some(codecpar) = stream.codecpar() {
                    FrameInfo {
                        codec_id: codecpar.codec_id,
                        codec_type: codecpar.codec_type,
                    }
                } else {
                    FrameInfo::default()
                }
            })
            .collect();
        Self {
            reader,
            frame_infos,
        }
    }
}

/// Simple Reader for Demuxing Media Files.
#[derive(Debug)]
pub struct SimpleReader {
    ctx: AVFormatContextOwned,
    bsfs: Vec<AVBSFContextOwned>,
    time_base: Option<AVRational>,
}

impl SimpleReader {
    /// Create a new simple reader.
    /// # Arguments
    /// * `path` - Path of the input file.
    /// * `format_options` - The options for demuxing format，like: movfragement.
    /// * `time_unit` - Convert the pts, dts or duration to specified time unit,
    //                  For example: convert to `us` unit: `time_unit=1000000`.
    /// # Panics
    ///
    pub fn open<P>(path: P, format_options: Option<&str>, time_unit: Option<i32>) -> AVResult<Self>
    where
        P: AsRef<Path> + Sized,
    {
        let ctx = AVFormatContextOwned::with_input(path, format_options)?;
        let mut bsfs: Vec<AVBSFContextOwned> = vec![];
        for stream in ctx.streams() {
            if let Some(codecpar) = stream.codecpar() {
                let filter_name = match codecpar.codec_tag {
                    AV_CODEC_TAG_AVC1 => "h264_mp4toannexb",
                    AV_CODEC_TAG_HEV1 | AV_CODEC_TAG_HVC1 => "hevc_mp4toannexb",
                    _ => "null",
                };
                let mut bsf = AVBSFContextOwned::new(filter_name)?;
                bsf.prepare(Some(codecpar))?;
                bsfs.push(bsf);
            }
        }
        Ok(Self {
            ctx,
            bsfs,
            time_base: time_unit.map(|x| AVRational::new(1, x)),
        })
    }

    /// Returns the total stream bitrate in bit/s, 0 if not available.
    pub fn bit_rate(&self) -> i64 {
        self.ctx.bit_rate
    }

    /// Returns the duration of the stream.
    pub fn duration(&self) -> i64 {
        self.ctx.duration
    }

    /// Returns a list to describe the frame for each stream.
    pub fn frame_infos(&self) -> Vec<FrameInfo> {
        self.streams()
            .iter()
            .map(|stream| {
                if let Some(codecpar) = stream.codecpar() {
                    FrameInfo {
                        codec_id: codecpar.codec_id,
                        codec_type: codecpar.codec_type,
                    }
                } else {
                    FrameInfo::default()
                }
            })
            .collect()
    }

    // Returns an iterator over the frames.
    pub fn frames(&mut self) -> FrameIter<'_> {
        FrameIter::new(self)
    }

    /// Return the next frame of a stream.
    pub fn read_frame(&mut self) -> Option<AVPacketOwned> {
        'outer: loop {
            // Fetch frames from bitstream filter first.
            for bsf in self.bsfs.iter_mut() {
                match bsf.receive_packet() {
                    Ok(packet) => {
                        return Some(packet);
                    }
                    Err(err) => match err {
                        AVBSFError::Again => {}
                        AVBSFError::Reason(_) => {}
                    },
                }
            }
            // Read frame from I/O context.
            if let Some(mut packet) = self.ctx.read_frame() {
                let stream_index = packet.stream_index as usize;
                // Convert pts, dts, duratin to user specified.
                if let (Some(out_time_base), Some(stream)) =
                    (self.time_base, self.ctx.streams().get(stream_index))
                {
                    let in_time_base = stream.time_base;
                    let pts = unsafe {
                        av_rescale_q_rnd(
                            packet.pts,
                            in_time_base,
                            out_time_base,
                            AVRounding::new().near_inf().pass_min_max(),
                        )
                    };
                    let dts = unsafe {
                        av_rescale_q_rnd(
                            packet.dts,
                            in_time_base,
                            out_time_base,
                            AVRounding::new().near_inf().pass_min_max(),
                        )
                    };
                    let duration =
                        unsafe { av_rescale_q(packet.duration, in_time_base, out_time_base) };
                    packet.pts = pts;
                    packet.dts = dts;
                    packet.duration = duration;
                }
                // Send to bitstream filter.
                if self.bsfs[stream_index].send_packet(&mut packet).is_err() {
                    break 'outer;
                }
            } else {
                break 'outer;
            }
        }

        None
    }

    /// Returns the position of the first frame of the component.
    pub fn start_time(&self) -> i64 {
        self.ctx.start_time
    }

    /// Returns then stream at index of the file.
    pub fn stream(&self, index: usize) -> Option<&AVStream> {
        self.streams().get(index).copied()
    }

    /// Returns a list of all streams in the file.
    pub fn streams(&self) -> &[&AVStream] {
        self.ctx.streams()
    }
}
