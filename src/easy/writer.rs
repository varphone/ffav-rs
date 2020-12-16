use super::{owned::*, AVResult};
use crate::ffi::{AVCodecID::*, AVFieldOrder::*, AVMediaType::*, AVPixelFormat::*, *};
use std::convert::TryInto;
use std::fmt::Debug;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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

impl Debug for &dyn MediaDesc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MediaDesc {{ codec_id: {:?} }}", self.codec_id())
    }
}

impl Debug for Box<dyn MediaDesc> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MediaDesc {{ codec_id: {:?} }}", self.codec_id())
    }
}

/// Trait for Writer.
pub trait Writer {
    /// Write the header of the format to the stream.
    fn write_header(&mut self) -> AVResult<()>;

    /// Write frame bytes to the stream.
    /// # Arguments
    /// * `bytes` - Stream byte data.
    /// * `pts` - Timestamp of the frame.
    /// * `duration` - Duration of the frame.
    /// * `is_key_frame` - True if is key frame.
    /// * `stream_index` - Index of the stream.
    fn write_bytes(
        &mut self,
        bytes: &[u8],
        pts: i64,
        duration: i64,
        is_key_frame: bool,
        stream_index: usize,
    ) -> AVResult<()>;

    /// Write the trailer of the format to the stream.
    fn write_trailer(&mut self) -> AVResult<()>;

    /// Close all resouces accessed by the muxer.
    fn close(&mut self);

    /// Flush all buffered data to stream destionation.
    fn flush(&mut self);

    /// Returns the size of the stream processed.
    fn size(&self) -> u64;
}

impl Debug for &dyn Writer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Writer @ 0x{:p}", self)
    }
}

impl Debug for Box<dyn Writer> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Writer @ 0x{:p}", self)
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
    format_options: String,
    streams: Vec<Stream>,
    header_writed: bool,
    trailer_writed: bool,
}

impl Drop for SimpleWriter {
    fn drop(&mut self) {
        self.close();
    }
}

impl Writer for SimpleWriter {
    /// Write the header of the format to the stream.
    fn write_header(&mut self) -> AVResult<()> {
        Ok(())
    }

    /// Write frame bytes to the stream.
    /// # Arguments
    /// * `bytes` - Stream byte data.
    /// * `pts` - Timestamp of the frame.
    /// * `duration` - Duration of the frame.
    /// * `is_key_frame` - True if is key frame.
    /// * `stream_index` - Index of the stream.
    fn write_bytes(
        &mut self,
        bytes: &[u8],
        pts: i64,
        duration: i64,
        is_key_frame: bool,
        stream_index: usize,
    ) -> AVResult<()> {
        if !self.header_writed {
            self.ctx.write_header(Some(&self.format_options))?;
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
            pkt.flags = if is_key_frame { AV_PKT_FLAG_KEY } else { 0 };
            pkt.duration = av_rescale_q(duration, in_time_base, out_time_base);
            pkt.pos = -1;
            self.ctx.write_frame_interleaved(&mut pkt)?;
            self.ctx.flush();
            Ok(())
        }
    }

    /// Write the trailer to finish the muxing.
    fn write_trailer(&mut self) -> AVResult<()> {
        if self.header_writed && !self.trailer_writed {
            self.ctx.write_trailer()?;
            self.trailer_writed = true;
            self.flush();
        }
        Ok(())
    }

    /// Close all resouces accessed by the muxer.
    fn close(&mut self) {
        self.write_trailer().unwrap();
        self.ctx.flush();
    }

    /// Flush all buffered data to stream destionation.
    fn flush(&mut self) {
        self.ctx.flush();
    }

    /// Returns the size of the stream processed.
    fn size(&self) -> u64 {
        self.ctx.size()
    }
}

impl SimpleWriter {
    /// Create a new simple writer.
    /// # Arguments
    /// * `path` - Path of the output file.
    /// * `descs` - Media description of input streams.
    /// * `format` - The format to muxing，like: mp4, mpegts.
    /// * `format_options` - The options for muxing format，like: movfragement.
    pub fn new<P>(
        path: P,
        descs: &[&dyn MediaDesc],
        format: Option<&str>,
        format_options: Option<&str>,
    ) -> AVResult<Self>
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
                    if let Some(par) = st.codecpar_mut() {
                        par.codec_type = AVMEDIA_TYPE_VIDEO;
                        par.codec_id = codec_id;
                        par.bit_rate = desc.bit_rate;
                        par.width = desc.width;
                        par.height = desc.height;
                        par.field_order = AV_FIELD_UNKNOWN;
                        par.sample_aspect_ratio = AVRational::new(0, 1);
                        par.profile = FF_PROFILE_UNKNOWN;
                        par.level = FF_LEVEL_UNKNOWN;
                    }
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
            format_options: format_options.unwrap_or("").to_owned(),
            streams,
            header_writed: false,
            trailer_writed: false,
        })
    }
}

/// The Callback for returns the the fragment file name.
/// # Arguments
/// * `index` - Current Fragment Index.
pub type FormatLocationCallback = dyn Fn(usize) -> String;

/// The Callback for before and after split fragment.
/// # Arguments
/// * `index` - Current Fragment Index.
pub type SplitNotifier = dyn Fn(usize);

/// Options for SplitWriter.
#[derive(Default)]
pub struct SplitOptions {
    output_path: Option<PathBuf>,
    format_location: Option<Box<FormatLocationCallback>>,
    before_split: Option<Box<SplitNotifier>>,
    after_split: Option<Box<SplitNotifier>>,
    max_files: Option<usize>,
    max_size_bytes: Option<u64>,
    max_size_time: Option<u64>,
    max_overhead: Option<f32>,
    split_at_keyframe: Option<bool>,
    start_index: Option<usize>,
}

impl Debug for SplitOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SplitOptions")
            .field("output_path", &self.output_path)
            .field("max_files", &self.max_files)
            .field("max_size_bytes", &self.max_size_bytes)
            .field("max_size_time", &self.max_size_time)
            .field("max_overhead", &self.max_overhead)
            .field("split_at_keyframe", &self.split_at_keyframe)
            .field("start_index", &self.start_index)
            .finish()
    }
}

/// Split Writer for Muxing Audio and Video.
pub struct SplitWriter {
    /// Media descriptions.
    medias: Vec<Box<dyn MediaDesc>>,
    /// The format to muxing.
    format: Option<String>,
    /// The options of muxing format.
    format_options: Option<String>,
    /// The underly writer.
    writer: Option<Box<dyn Writer>>,
    /// The location of the files to write.
    output_path: PathBuf,
    /// Callback for returns the location to be used for the next output file.
    format_location: Option<Box<FormatLocationCallback>>,
    /// Callback on before split fragment.
    before_split: Option<Box<SplitNotifier>>,
    /// Callback on after split fragment.
    after_split: Option<Box<SplitNotifier>>,
    /// Maximum number of files to keep on disk. Once the maximum is reached,
    /// old files start to be deleted to make room for new ones.
    max_files: usize,
    /// Max amount of data per file (in bytes, 0=disable).
    max_size_bytes: u64,
    /// Max amount of time per file (in ns, 0=disable).
    max_size_time: u64,
    /// Extra size/time overhead of muxing.
    max_overhead: f32,
    /// Split at key frame input.
    split_at_keyframe: bool,
    /// Start value of fragment index.
    start_index: usize,
    /// Current value of fragment index.
    current_index: usize,
    /// Start time of the current fragment.
    start_time: Instant,
    /// The data flow started,
    started: bool,
    ///
    need_key_frame: bool,
    split_wait_for_key_frame: bool,
}

impl Debug for SplitWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SplitWriter @ 0x{:p}", self)
    }
}

impl Writer for SplitWriter {
    fn write_header(&mut self) -> AVResult<()> {
        if let Some(writer) = &mut self.writer {
            writer.write_header()
        } else {
            Err("The underly writer does not ready".into())
        }
    }

    fn write_bytes(
        &mut self,
        bytes: &[u8],
        pts: i64,
        duration: i64,
        is_key_frame: bool,
        stream_index: usize,
    ) -> AVResult<()> {
        if self.can_split_now(is_key_frame, stream_index) {
            self.split_now();
        }

        if self.writer.is_none() {
            let writer = SimpleWriter::new(
                self.format_location(self.current_index).to_str().unwrap(),
                &self
                    .medias
                    .iter()
                    .map(Deref::deref)
                    .collect::<Vec<&dyn MediaDesc>>(),
                self.format.as_deref(),
                self.format_options.as_deref(),
            )?;
            self.writer = Some(Box::new(writer));
            self.start_time = Instant::now();
            self.started = true;
        }

        if let Some(ref mut writer) = self.writer {
            writer.write_bytes(bytes, pts, duration, is_key_frame, stream_index)?;
        }

        Ok(())
    }

    fn write_trailer(&mut self) -> AVResult<()> {
        if let Some(writer) = &mut self.writer {
            writer.write_trailer()
        } else {
            Err("The underly writer does not ready".into())
        }
    }

    fn close(&mut self) {
        if let Some(writer) = &mut self.writer {
            writer.close();
        }
    }

    fn flush(&mut self) {
        if let Some(writer) = &mut self.writer {
            writer.flush();
        }
    }

    fn size(&self) -> u64 {
        if let Some(writer) = &self.writer {
            writer.size()
        } else {
            0
        }
    }
}

impl SplitWriter {
    /// Create a new writer with multipart files.
    /// # Arguments
    /// * `descs` - Media description of input streams.
    /// * `format` - The format to muxing，like: mp4, mpegts.
    /// * `format_options` - The options for muxing format，like: movfragement.
    /// * `split_options` - The options for multipart files.
    /// # Panics
    /// The `output_path` must be set.
    pub fn new(
        descs: Vec<Box<dyn MediaDesc>>,
        format: Option<&str>,
        format_options: Option<&str>,
        split_options: SplitOptions,
    ) -> AVResult<Self> {
        let mut need_key_frame = false;
        for d in descs.iter() {
            if d.codec_id().has_gop() {
                need_key_frame = true;
            }
        }
        Ok(Self {
            medias: descs,
            format: format.map(String::from),
            format_options: format_options.map(String::from),
            writer: None,
            output_path: split_options.output_path.unwrap(),
            format_location: split_options.format_location,
            before_split: split_options.before_split,
            after_split: split_options.after_split,
            max_files: split_options.max_files.unwrap_or(0),
            max_size_bytes: split_options.max_size_bytes.unwrap_or(0),
            max_size_time: split_options.max_size_time.unwrap_or(0),
            max_overhead: split_options.max_overhead.unwrap_or(0.1f32),
            split_at_keyframe: split_options.split_at_keyframe.unwrap_or(true),
            start_index: split_options.start_index.unwrap_or(0),
            current_index: split_options.start_index.unwrap_or(0),
            start_time: Instant::now(),
            started: false,
            need_key_frame,
            split_wait_for_key_frame: false,
        })
    }

    /// Returns `true` if `writer.size() >= max_size_bytes`.
    pub(crate) fn is_bytes_overrun(&mut self) -> bool {
        let mut exceeded = false;
        if let Some(ref writer) = self.writer {
            if self.max_size_bytes > 0 && writer.size() >= self.max_size_bytes {
                exceeded = true
            }
        }
        exceeded
    }

    /// Returns `true` if `writer.size() >= max_size_bytes * (1.0 + max_overhead)`.
    pub(crate) fn is_bytes_overflow(&mut self) -> bool {
        let mut exceeded = false;
        if let Some(ref writer) = self.writer {
            let overhead_bytes = self.max_size_bytes * (self.max_overhead * 100.0) as u64 / 100;
            if self.max_size_bytes > 0 && writer.size() >= self.max_size_bytes + overhead_bytes {
                exceeded = true
            }
        }
        exceeded
    }

    /// Returns `true` if `time >= max_size_time`.
    pub(crate) fn is_time_overrun(&mut self) -> bool {
        self.max_size_time > 0
            && self.start_time.elapsed() >= Duration::from_nanos(self.max_size_time)
    }

    /// Returns `true` if `time >= max_size_time * (1.0 + max_overhead)`.
    pub(crate) fn is_time_overflow(&mut self) -> bool {
        let overhead_time = self.max_size_time * (self.max_overhead * 100.0) as u64 / 100;
        self.max_size_time > 0
            && self.start_time.elapsed() >= Duration::from_nanos(self.max_size_time + overhead_time)
    }

    /// Return `true` if can split fragment now.
    pub fn can_split_now(&mut self, is_key_frame: bool, stream_index: usize) -> bool {
        let mut split_now: bool = false;
        if self.split_wait_for_key_frame {
            split_now = self.stream_has_key_frame(stream_index) && is_key_frame;
            self.split_wait_for_key_frame = false;
        } else {
            let overrun = self.is_bytes_overrun() || self.is_time_overrun();
            if overrun && self.split_at_keyframe && self.need_key_frame {
                self.split_wait_for_key_frame = true;
            } else {
                split_now = overrun;
            }
        }
        let overflow = self.is_bytes_overflow() || self.is_time_overflow();
        split_now || overflow
    }

    /// Clean older files.
    pub fn clean_files(&self) {
        if self.max_files > 0 && (self.current_index - self.start_index) >= self.max_files - 1 {
            let index = self.current_index - (self.max_files - 1);
            if index >= self.start_index {
                let old_file = self.format_location(index);
                std::fs::remove_file(old_file).unwrap();
            }
        }
    }

    /// Returns the extension of the format.
    pub fn ext_of_format(format: Option<&str>) -> &'static str {
        format
            .map(|s| match s {
                "mp4" => ".mp4",
                "mpegts" => ".ts",
                _ => "dat",
            })
            .unwrap_or("dat")
    }

    /// Returns the fragment file location.
    pub fn format_location(&self, index: usize) -> PathBuf {
        let loc = if let Some(ref cb) = self.format_location {
            cb(index)
        } else {
            format!(
                "MED{:06}{}",
                index,
                Self::ext_of_format(self.format.as_deref())
            )
        };
        let path = self.output_path.join(loc);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        path
    }

    /// Close the output file and create a new one.
    pub fn split_now(&mut self) {
        if let Some(ref cb) = self.before_split {
            cb(self.current_index);
        }
        let _ = self.writer.take();
        self.clean_files();
        self.current_index += 1;
        if let Some(ref cb) = self.after_split {
            cb(self.current_index);
        }
    }

    /// Return `true` if the stream has `key_frame` props.
    pub fn stream_has_key_frame(&self, stream_index: usize) -> bool {
        self.medias[stream_index].codec_id().has_gop()
    }
}

/// Options Builder for the SimpleWriter.
#[derive(Default)]
pub struct OpenOptions {
    medias: Vec<Box<dyn MediaDesc>>,
    format: Option<String>,
    format_options: Option<String>,
    format_location: Option<Box<FormatLocationCallback>>,
    before_split: Option<Box<SplitNotifier>>,
    after_split: Option<Box<SplitNotifier>>,
    max_files: Option<usize>,
    max_size_bytes: Option<u64>,
    max_size_time: Option<u64>,
    max_overhead: Option<f32>,
    split_at_keyframe: Option<bool>,
    start_index: Option<usize>,
}

impl Debug for OpenOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OpenOptions @ 0x{:p}", self)
    }
}

impl OpenOptions {
    /// Create an new Options Builder for the SimpleWriter.
    pub fn new() -> Self {
        Default::default()
    }

    /// Add a media description to the output format.
    pub fn media<T>(mut self, media: T) -> Self
    where
        T: MediaDesc + Sized + 'static,
    {
        self.medias.push(Box::new(media));
        self
    }

    /// Specified the muxing format of the output format.
    pub fn format<S>(mut self, format: S) -> Self
    where
        S: Into<String>,
    {
        self.format = Some(format.into());
        self
    }

    /// Specified the muxing format options of the output format.
    pub fn format_options<S>(mut self, format_options: S) -> Self
    where
        S: Into<String>,
    {
        self.format_options = Some(format_options.into());
        self
    }

    /// Callback for returns the location to be used for the next output file.
    pub fn format_location<F>(mut self, format_location: F) -> Self
    where
        F: Fn(usize) -> String + 'static,
    {
        self.format_location = Some(Box::new(format_location));
        self
    }

    /// Callback before split fragment.
    pub fn before_split<F>(mut self, before_split: F) -> Self
    where
        F: Fn(usize) + 'static,
    {
        self.before_split = Some(Box::new(before_split));
        self
    }

    /// Callback after split fragment.
    pub fn after_split<F>(mut self, after_split: F) -> Self
    where
        F: Fn(usize) + 'static,
    {
        self.after_split = Some(Box::new(after_split));
        self
    }

    /// Maximum number of files to keep on disk.
    pub fn max_files(mut self, max_files: usize) -> Self {
        self.max_files = Some(max_files);
        self
    }

    /// Max amount of data per file (in bytes, 0=disable).
    pub fn max_size_bytes(mut self, max_size_bytes: u64) -> Self {
        self.max_size_bytes = Some(max_size_bytes);
        self
    }

    /// Max amount of time per file (in ns, 0=disable).
    pub fn max_size_time(mut self, max_size_time: u64) -> Self {
        self.max_size_time = Some(max_size_time);
        self
    }

    /// Extra size/time overhead of muxing (0.02 = 2%).
    pub fn max_overhead(mut self, max_overhead: f32) -> Self {
        self.max_overhead = Some(max_overhead);
        self
    }

    /// Split immediately if `split_at_keyframe = false`.
    /// The option ignored when `size > max_size_bytes + max_size_bytes * max_overhead`
    /// or `time > max_size_time + max_size_time * max_overhead`.
    pub fn split_at_keyframe(mut self, split_at_keyframe: bool) -> Self {
        self.split_at_keyframe = Some(split_at_keyframe);
        self
    }

    /// Start value of fragment index.
    pub fn start_index(mut self, start_index: usize) -> Self {
        self.start_index = Some(start_index);
        self
    }

    /// Open the output file and returns the SimpleWriter.
    pub fn open<P>(self, path: P) -> AVResult<Box<dyn Writer>>
    where
        P: AsRef<Path> + Sized,
    {
        if self.format_location.is_some() || self.max_files.is_some() {
            let split_options = SplitOptions {
                output_path: Some(AsRef::<Path>::as_ref(&path).to_path_buf()),
                format_location: self.format_location,
                before_split: self.before_split,
                after_split: self.after_split,
                max_files: self.max_files,
                max_size_bytes: self.max_size_bytes,
                max_size_time: self.max_size_time,
                max_overhead: self.max_overhead,
                split_at_keyframe: self.split_at_keyframe,
                start_index: self.start_index,
            };
            let writer = SplitWriter::new(
                self.medias,
                self.format.as_deref(),
                self.format_options.as_deref(),
                split_options,
            )?;
            Ok(Box::new(writer))
        } else {
            let medias: Vec<&dyn MediaDesc> = self.medias.iter().map(Deref::deref).collect();
            let writer = SimpleWriter::new(
                path,
                &medias[..],
                self.format.as_deref(),
                self.format_options.as_deref(),
            )?;
            Ok(Box::new(writer))
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
            let mut mp4_writer = SimpleWriter::new(
                "/tmp/envivio-352x288.264.mp4",
                &[&a_desc, &v_desc],
                None,
                Some("movflags=frag_keyframe"),
            )
            .unwrap();
            let mut ts_writer = SimpleWriter::new(
                "/tmp/envivio-352x288.264.ts",
                &[&a_desc, &v_desc],
                Some("mpegts"),
                Some("mpegts_copyts=1"),
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
                mp4_writer
                    .write_bytes(frame_bytes, pts, 40000, false, 0)
                    .unwrap();
                ts_writer
                    .write_bytes(frame_bytes, pts, 40000, false, 0)
                    .unwrap();
                pts += 40000;
            }
        }
    }
}
