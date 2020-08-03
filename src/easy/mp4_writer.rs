use crate::ffi::*;

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
            }
        }
    }
}

pub trait MediaOptions {
    fn codec_id(&self) -> AVCodecID;
}

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

pub struct VideoOptions {
    codec_id: AVCodecID,
    bit_rate: usize,
    width: usize,
    height: usize,
    gop_size: usize,
    pix_fmt: AVPixelFormat,
}

pub struct Mp4Writer {
    dummy: usize,
}

impl Mp4Writer {
    pub fn new(options: &[&dyn MediaOptions]) -> Result<Self, String> {
        Ok(Self {
            dummy: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mp4_writer() {

    }
}