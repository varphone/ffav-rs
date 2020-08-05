use crate::ffi::{AVCodecID::*, *};
use std::error::Error;
use std::ffi::CString;
use std::ops::{Deref, DerefMut};
use std::path::Path;

#[repr(transparent)]
struct AVBytesPacket(AVPacket);


impl 