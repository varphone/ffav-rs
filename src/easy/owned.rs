use crate::ffi::{AVCodecID::*, *};
use std::error::Error;
use std::ffi::CString;
use std::ops::{Deref, DerefMut};
use std::path::Path;

pub type AVResult<T> = Result<T, Box<dyn Error>>;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd)]
pub enum AVFormatContextMode {
    Input,
    Output,
}

pub struct AVFormatContextOwned {
    ptr: *mut AVFormatContext,
    mode: AVFormatContextMode,
}

impl Drop for AVFormatContextOwned {
    fn drop(&mut self) {
        println!("impl Drop for AVFormatContextOwned");
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
    pub fn from_ptr(ptr: *mut AVFormatContext, mode: AVFormatContextMode) -> Self {
        Self { ptr, mode }
    }

    pub fn with_output<N, P>(
        oformat: Option<&AVOutputFormat>,
        format_name: N,
        file_name: P,
    ) -> AVResult<Self>
    where
        // O: AsRef<AVOutputFormat>,
        N: AsRef<str>,
        P: AsRef<Path>,
    {
        unsafe {
            let mut ps = std::ptr::null_mut();
            let format = CString::new(format_name.as_ref())?;
            let path = CString::new(file_name.as_ref().as_os_str().to_str().unwrap()).unwrap();
            let err = avformat_alloc_output_context2(
                &mut ps,
                oformat.map_or(std::ptr::null_mut(), |x| {
                    x as *const AVOutputFormat as *mut AVOutputFormat
                }),
                format.as_ptr(),
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

    pub fn new_stream(&mut self, codec_id: AVCodecID) -> AVResult<AVStreamOwned> {
        unsafe {
            let codec = avcodec_find_encoder(codec_id);
            if codec.is_null() {
                return Err(format!("The encoder {:?} does not exists", codec_id).into());
            }
            let stream = avformat_new_stream(self.ptr, codec);
            if stream.is_null() {
                Err(format!("Failed to create new stream for {:?}", codec_id).into())
            } else {
                let stream = AVStreamOwned::from_ptr(stream);
                Ok(stream)
            }
        }
    }

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

    pub fn write_trailer(&mut self) -> AVResult<()> {
        unsafe {
            av_write_trailer(self.ptr);
        }
        Ok(())
    }

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
    pub fn from_ptr(ptr: *mut AVOutputFormat) -> Self {
        Self { ptr }
    }
}

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
