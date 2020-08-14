use super::AVResult;
use crate::ffi::*;
use std::ffi::{CStr, CString};
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};
use std::os::raw::c_char;
use std::path::Path;
use std::str::FromStr;

/// Wrap an owned AVDictionary pointer.
#[repr(transparent)]
#[derive(Debug)]
pub struct AVDictionaryOwned {
    ptr: *mut AVDictionary,
}

impl Default for AVDictionaryOwned {
    fn default() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
        }
    }
}

impl Drop for AVDictionaryOwned {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                av_dict_free(&mut self.ptr);
            }
        }
    }
}

impl Deref for AVDictionaryOwned {
    type Target = AVDictionary;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl DerefMut for AVDictionaryOwned {
    fn deref_mut(&mut self) -> &mut AVDictionary {
        unsafe { &mut *self.ptr }
    }
}

impl FromStr for AVDictionaryOwned {
    type Err = Box<dyn Error>;
    /// Create an an owned AVDictionary from string.
    ///
    /// The format of the string like: "key1=value1:key2=value2"
    fn from_str(options: &str) -> Result<Self, Self::Err> {
        unsafe {
            let mut ptr: *mut AVDictionary = std::ptr::null_mut();
            let options = CString::new(options).unwrap();
            let kv_sep = CString::new("=").unwrap();
            let pair_sep = CString::new(":").unwrap();
            let err = av_dict_parse_string(
                &mut ptr,
                options.as_ptr(),
                kv_sep.as_ptr(),
                pair_sep.as_ptr(),
                0,
            );
            if err < 0 {
                Err(av_err2str(err).into())
            } else {
                Ok(Self { ptr })
            }
        }
    }
}

impl AVDictionaryOwned {
    pub fn as_ptr(&self) -> *const AVDictionary {
        self.ptr as *const AVDictionary
    }

    pub fn as_mut_ptr(&mut self) -> *mut AVDictionary {
        self.ptr
    }

    pub fn as_mut_ptr_ref(&mut self) -> &mut *mut AVDictionary {
        &mut self.ptr
    }
}

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
    pub fn write_header(&mut self, options: Option<&str>) -> AVResult<()> {
        unsafe {
            let mut opt = AVDictionaryOwned::from_str(options.unwrap_or("")).unwrap();
            let err = avformat_write_header(self.ptr, opt.as_mut_ptr_ref());
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

#[repr(transparent)]
pub struct AVPacketBoxed {
    ptr: *mut AVPacket,
}

impl Debug for AVPacketBoxed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unsafe {
            if f.alternate() {
                write!(f, "{:#?}", *self.ptr)
            } else {
                write!(f, "{:?}", *self.ptr)
            }
        }
    }
}

impl Drop for AVPacketBoxed {
    fn drop(&mut self) {
        println!("Drop for AVPacketBoxed({:p})", self.ptr);
        unsafe {
            av_packet_free(&mut self.ptr);
        }
    }
}

impl Deref for AVPacketBoxed {
    type Target = AVPacket;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl DerefMut for AVPacketBoxed {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}

impl AVPacketBoxed {
    pub fn from_ptr(ptr: *mut AVPacket) -> Self {
        Self { ptr }
    }

    pub fn as_ptr(&self) -> *const AVPacket {
        self.ptr as *const AVPacket
    }

    pub fn as_mut_ptr(&mut self) -> *mut AVPacket {
        self.ptr
    }

    pub fn as_mut_ptr_ref(&mut self) -> &mut *mut AVPacket {
        &mut self.ptr
    }
}

#[repr(transparent)]
#[derive(Default)]
pub struct AVPacketOwned {
    inner: AVPacket,
}

impl Debug for AVPacketOwned {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            write!(f, "{:#?}", self.inner)
        } else {
            write!(f, "{:?}", self.inner)
        }
    }
}

impl Drop for AVPacketOwned {
    fn drop(&mut self) {
        unsafe {
            av_packet_unref(&mut self.inner);
        }
    }
}

impl Deref for AVPacketOwned {
    type Target = AVPacket;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for AVPacketOwned {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl AVPacketOwned {
    pub fn new() -> Self {
        Self {
            inner: Default::default(),
        }
    }

    pub fn as_ptr(&self) -> *const AVPacket {
        &self.inner as *const AVPacket
    }

    pub fn as_mut_ptr(&mut self) -> *mut AVPacket {
        &mut self.inner
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

/// Representation of a managed C string.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AVBoxedCStr<'a> {
    inner: &'a CStr,
}

impl<'a> Debug for AVBoxedCStr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

impl<'a> Drop for AVBoxedCStr<'a> {
    fn drop(&mut self) {
        unsafe {
            let mut ptr = self.inner.as_ptr() as *mut core::ffi::c_void;
            av_freep(std::mem::transmute::<
                &mut *mut core::ffi::c_void,
                *mut core::ffi::c_void,
            >(&mut ptr));
        }
    }
}

impl<'a> Deref for AVBoxedCStr<'a> {
    type Target = CStr;

    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

impl<'a> AVBoxedCStr<'a> {
    /// Wraps a raw C string with a safe C string wrapper.
    ///
    /// The ownership of the ptr is transfered to the AVBoxedCStr.
    pub unsafe fn from_ptr(ptr: *const c_char) -> Self {
        Self {
            inner: CStr::from_ptr(ptr),
        }
    }
}
