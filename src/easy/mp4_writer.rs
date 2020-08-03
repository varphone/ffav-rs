use crate::ffi::{
    av_err2str, avcodec_find_encoder, avformat_alloc_output_context2, avformat_close_input,
    avformat_free_context, avformat_new_stream, avio_close, avio_open, AVCodecID, AVCodecID::*,
    AVFormatContext, AVPixelFormat, AVSampleFormat, AVIO_FLAG_WRITE,
};
use std::error::Error;
use std::ffi::CString;
use std::ops::{Deref, DerefMut};
use std::path::Path;

enum AVFormatContextMode {
    Input,
    Output,
}

pub struct AVFormatContextOwned {
    ptr: *mut AVFormatContext,
    mode: AVFormatContextMode,
}

impl Drop for AVFormatContextOwned {
    fn drop(&mut self) {
        match self.mode {
            AVFormatContextMode::Input => unsafe {
                avformat_close_input(&mut self.ptr);
            },

            AVFormatContextMode::Output => unsafe {
                avio_close((*self.ptr).pb);
                avformat_free_context(self.ptr);
            },
        }
    }
}

impl Deref for AVFormatContextOwned {
    type Target = AVFormatContext;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl DerefMut for AVFormatContextOwned {
    fn deref_mut(&mut self) -> &mut AVFormatContext {
        unsafe { &mut *self.ptr }
    }
}

impl AVFormatContextOwned {
    pub fn input(ptr: *mut AVFormatContext) -> Self {
        Self {
            ptr,
            mode: AVFormatContextMode::Input,
        }
    }

    pub fn output(ptr: *mut AVFormatContext) -> Self {
        Self {
            ptr,
            mode: AVFormatContextMode::Output,
        }
    }
}

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
    codec_id: AVCodecID,
    sample_fmt: AVSampleFormat,
    bit_rate: usize,
    sample_rate: usize,
    channels: usize,
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
    codec_id: AVCodecID,
    bit_rate: usize,
    width: usize,
    height: usize,
    gop_size: usize,
    pix_fmt: AVPixelFormat,
}

impl MediaOptions for VideoOptions {
    fn codec_id(&self) -> AVCodecID {
        self.codec_id
    }
}

impl VideoOptions {
    pub fn new() -> Self {
        Default::default()
    }
}

pub struct Mp4Writer {
    fmt_ctx: AVFormatContextOwned,
}

impl Mp4Writer {
    pub fn new<P: AsRef<Path> + Sized>(
        path: P,
        options: &[&dyn MediaOptions],
    ) -> Result<Self, Box<dyn Error>> {
        unsafe {
            let mut ps = std::ptr::null_mut();
            let format = CString::new("mp4")?;
            let path = CString::new(path.as_ref().as_os_str().to_str().unwrap()).unwrap();
            let err = avformat_alloc_output_context2(
                &mut ps,
                std::ptr::null_mut(),
                format.as_ptr(),
                path.as_ptr(),
            );
            if err < 0 {
                return Err(av_err2str(err).into());
            }
            let fmt_ctx = AVFormatContextOwned::output(ps);
            let err = avio_open(&mut (*ps).pb, path.as_ptr(), AVIO_FLAG_WRITE);
            if err < 0 {
                return Err(av_err2str(err).into());
            }
            for o in options {
                let codec_id = o.codec_id();
                match codec_id {
                    AV_CODEC_ID_H264 | AV_CODEC_ID_HEVC => {
                        let codec = avcodec_find_encoder(codec_id);
                        let stream = avformat_new_stream(ps, codec);
                    }
                    _ => {}
                }
            }
            Ok(Self { fmt_ctx })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mp4_writer() {
        let a_opts = AudioOptions::new();
        let v_opts = VideoOptions::new();
        let writer = Mp4Writer::new("example.mp4", &[&a_opts, &v_opts]).unwrap();
    }
}
