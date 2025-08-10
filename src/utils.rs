use rsmpeg::{
    avcodec::{AVCodecContext, AVCodecParameters},
    avutil::{get_bytes_per_sample, AVChannelLayout, AVRational},
    ffi::{self},
};

pub struct AudioInfo {
    pub sample_rate: usize,
    pub sample_fmt: ffi::AVSampleFormat,
    pub ch_layout: AVChannelLayout,
    pub sample_size: usize,
}

impl AudioInfo {
    #[allow(unused)]
    fn new(ctx: &AVCodecContext) -> Option<Self> {
        let sample_size = get_bytes_per_sample(ctx.sample_fmt)?;
        Some(Self {
            sample_rate: ctx.sample_rate as usize,
            sample_fmt: ctx.sample_fmt,
            ch_layout: ctx.ch_layout().clone(),
            sample_size,
        })
    }

    pub fn new_pcm(sample_rate: usize) -> Self {
        let sample_fmt = ffi::AV_SAMPLE_FMT_FLT;
        let sample_size = get_bytes_per_sample(sample_fmt).unwrap();
        Self {
            sample_rate,
            sample_fmt,
            ch_layout: AVChannelLayout::from_nb_channels(2),
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
