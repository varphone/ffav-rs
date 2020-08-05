use super::AVResult;
use crate::ffi::*;
use std::ffi::CString;
use std::ops::{Deref, DerefMut};
use std::path::Path;

/// Format context I/O mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd)]
pub enum AVFormatContextMode {
    Input,
    Output,
}

/// Format I/O context.
#[derive(Debug)]
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
    /// Wrap an exists AVFormatContext ptr.
    pub fn from_ptr(ptr: *mut AVFormatContext, mode: AVFormatContextMode) -> Self {
        Self { ptr, mode }
    }

    /// Create a new AVFormatContext for output.
    pub fn with_output<P>(
        path: P,
        format: Option<&str>,
        oformat: Option<&AVOutputFormat>,
    ) -> AVResult<Self>
    where
        P: AsRef<Path>,
    {
        unsafe {
            let mut ps = std::ptr::null_mut();
            let path = CString::new(path.as_ref().as_os_str().to_str().unwrap()).unwrap();
            let mut format_ptr = std::ptr::null();
            let cformat = CString::new(format.unwrap_or(""))?;
            if format.is_some() {
                format_ptr = cformat.as_ptr();
            }
            let err = avformat_alloc_output_context2(
                &mut ps,
                oformat.map_or(std::ptr::null_mut(), |x| {
                    x as *const AVOutputFormat as *mut AVOutputFormat
                }),
                format_ptr,
                path.as_ptr(),
            );
            if err < 0 {
                return Err(av_err2str(err).into());
            }
            let ofmt = AVOutputFormatOwned::from_ptr((*ps).oformat);
            if (ofmt.flags & AVFMT_NOFILE) != AVFMT_NOFILE {
                let err = avio_open(&mut (*ps).pb, path.as_ptr(), AVIO_FLAG_WRITE);
                if err < 0 {
                    avformat_free_context(ps);
                    return Err(av_err2str(err).into());
                }
            }
            Ok(Self {
                ptr: ps,
                mode: AVFormatContextMode::Output,
            })
        }
    }

    /// Add a new stream to a media file.
    pub fn new_stream(&mut self, codec_id: AVCodecID) -> AVResult<AVStreamOwned> {
        unsafe {
            // The codec is optional
            let codec = avcodec_find_encoder(codec_id);
            let stream = avformat_new_stream(self.ptr, codec);
            if stream.is_null() {
                Err(format!("Failed to create new stream for {:?}", codec_id).into())
            } else {
                let stream = AVStreamOwned::from_ptr(stream);
                Ok(stream)
            }
        }
    }

    /// Allocate the stream private data and write the stream header to an output media file.
    pub fn write_header(&mut self) -> AVResult<()> {
        unsafe {
            let err = avformat_write_header(self.ptr, std::ptr::null_mut());
            if err < 0 {
                Err(av_err2str(err).into())
            } else {
                Ok(())
            }
        }
    }

    /// Write the stream trailer to an output media file and free the file private data.
    pub fn write_trailer(&mut self) -> AVResult<()> {
        unsafe {
            av_write_trailer(self.ptr);
        }
        Ok(())
    }

    /// Write a packet to an output media file ensuring correct interleaving.
    pub fn write_frame_interleaved(&mut self, packet: &mut AVPacket) -> AVResult<()> {
        unsafe {
            let err = av_interleaved_write_frame(self.ptr, packet);
            if err < 0 {
                Err(av_err2str(err).into())
            } else {
                Ok(())
            }
        }
    }
}

#[derive(Debug)]
pub struct AVOutputFormatOwned {
    ptr: *mut AVOutputFormat,
}

impl Deref for AVOutputFormatOwned {
    type Target = AVOutputFormat;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl DerefMut for AVOutputFormatOwned {
    fn deref_mut(&mut self) -> &mut AVOutputFormat {
        unsafe { &mut *self.ptr }
    }
}

impl AVOutputFormatOwned {
    /// Wrap an exists AVOutputFormat ptr.
    pub fn from_ptr(ptr: *mut AVOutputFormat) -> Self {
        Self { ptr }
    }
}

#[derive(Debug)]
pub struct AVStreamOwned {
    ptr: *mut AVStream,
}

impl Deref for AVStreamOwned {
    type Target = AVStream;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl DerefMut for AVStreamOwned {
    fn deref_mut(&mut self) -> &mut AVStream {
        unsafe { &mut *self.ptr }
    }
}

impl AVStreamOwned {
    /// Wrap an exists AVStream ptr.
    pub fn from_ptr(ptr: *mut AVStream) -> Self {
        Self { ptr }
    }
}

impl AVStreamOwned {
    pub fn codecpar(&self) -> &AVCodecParameters {
        unsafe { &*self.codecpar }
    }

    pub fn codecpar_mut(&mut self) -> &mut AVCodecParameters {
        unsafe { &mut *self.codecpar }
    }
}
