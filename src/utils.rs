use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext, AVCodecParameters},
    avutil::{get_bytes_per_sample, AVRational},
    ffi::{self},
};

#[derive(Debug)]
pub struct AudioInfo {
    pub codec_id: ffi::AVCodecID,
    pub sample_rate: usize,
    pub sample_fmt: ffi::AVSampleFormat,
    pub channel_layout: u64,
    pub nb_channels: usize,
    pub sample_size: usize,
}

impl AudioInfo {
    #[allow(unused)]
    fn new(ctx: &AVCodecContext, codec: &AVCodec) -> Option<Self> {
        let sample_size = get_bytes_per_sample(ctx.sample_fmt)?;
        Some(Self {
            codec_id: codec.id,
            sample_rate: ctx.sample_rate as usize,
            sample_fmt: ctx.sample_fmt,
            channel_layout: ctx.channel_layout,
            nb_channels: ctx.channels as usize,
            sample_size,
        })
    }

    pub fn new_pcm(sample_rate: usize) -> Self {
        let sample_fmt = ffi::AVSampleFormat_AV_SAMPLE_FMT_FLT;
        let sample_size = get_bytes_per_sample(sample_fmt).unwrap();
        Self {
            codec_id: ffi::AVCodecID_AV_CODEC_ID_NONE,
            sample_rate,
            sample_fmt,
            channel_layout: ffi::AV_CH_LAYOUT_STEREO as _,
            nb_channels: 2,
            sample_size,
        }
    }
}

#[derive(Debug)]
pub struct AudioParameters {
    pub time_base: AVRational,
    pub codecpar: AVCodecParameters,
}

pub struct AudioData {
    pub nb_channels: usize,
    pub sample_rate: usize,
    pub samples: Vec<f32>,
}

impl AudioData {
    pub fn new(samples: Vec<f32>, nb_channels: usize, sample_rate: usize) -> Self {
        Self {
            nb_channels,
            sample_rate,
            samples,
        }
    }
}
