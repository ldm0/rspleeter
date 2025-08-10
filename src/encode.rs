use anyhow::{anyhow, Context, Result};
use camino::Utf8Path as Path;
use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext},
    avformat::{AVFormatContextOutput, AVOutputFormat},
    avutil::{AVFrame, AVRational},
    error::RsmpegError,
    ffi::{self},
    swresample::SwrContext,
    UnsafeDerefMut,
};
use std::{ffi::CString, slice};

use crate::utils::AudioInfo;
use crate::utils::AudioParameters;

/// Change pcm samples to original format.
fn init_resample_context(
    audio_parameters: &AudioParameters,
    pcm_data_info: &AudioInfo,
) -> Result<SwrContext> {
    let mut resample_context = SwrContext::new(
        &audio_parameters.codecpar.ch_layout(),
        audio_parameters.codecpar.format,
        audio_parameters.codecpar.sample_rate,
        &pcm_data_info.ch_layout,
        pcm_data_info.sample_fmt,
        pcm_data_info.sample_rate as i32,
    )
    .context("SwrContext parameters incorrect.")?;
    resample_context
        .init()
        .context("Init resample context failed.")?;
    Ok(resample_context)
}

fn init_encode_context(
    encoder: &AVCodec,
    audio_parameters: &AudioParameters,
) -> Result<AVCodecContext> {
    let mut encode_context = AVCodecContext::new(&encoder);
    encode_context
        .apply_codecpar(&audio_parameters.codecpar)
        .context("Apply codecpar failed.")?;
    // Strange: set time_base to audio_parameters.time_base(which `den` is way
    // larger than sample_rate) leads to `Queue input is backward in time`
    // error.
    encode_context.set_time_base(AVRational {
        num: 1,
        den: audio_parameters.codecpar.sample_rate,
    });
    encode_context
        .open(None)
        .context("Open codec context failed.")?;
    Ok(encode_context)
}

fn write_frame(
    output_format_context: &mut AVFormatContextOutput,
    encode_context: &mut AVCodecContext,
    frame: Option<&AVFrame>,
) -> Result<()> {
    encode_context
        .send_frame(frame)
        .context("Send frame failed.")?;

    loop {
        let mut packet = match encode_context.receive_packet() {
            Ok(packet) => packet,
            Err(RsmpegError::EncoderDrainError) | Err(RsmpegError::EncoderFlushedError) => {
                break;
            }
            Err(e) => return Err(e).context("receive packet failed."),
        };
        packet.rescale_ts(
            encode_context.time_base,
            output_format_context.streams().get(0).unwrap().time_base,
        );
        output_format_context
            .write_frame(&mut packet)
            .context("Write frame failed.")?;
    }
    Ok(())
}

fn create_input_frame(
    process_samples: usize,
    pcm_audio_info: &AudioInfo,
    pcm_data: &[u8],
) -> AVFrame {
    let mut input_frame = AVFrame::new();
    input_frame.set_nb_samples(process_samples as i32);
    input_frame.set_ch_layout(pcm_audio_info.ch_layout.clone().into_inner());
    input_frame.set_format(pcm_audio_info.sample_fmt);
    input_frame.set_sample_rate(pcm_audio_info.sample_rate as i32);
    input_frame.alloc_buffer().unwrap();
    let data =
        unsafe { slice::from_raw_parts_mut(input_frame.deref_mut().data[0], pcm_data.len()) };
    data.copy_from_slice(pcm_data);
    input_frame
}

fn create_output_frame(audio_parameters: &AudioParameters) -> AVFrame {
    let mut output_frame = AVFrame::new();
    output_frame.set_ch_layout(audio_parameters.codecpar.ch_layout().clone().into_inner());
    output_frame.set_format(audio_parameters.codecpar.format);
    output_frame.set_sample_rate(audio_parameters.codecpar.sample_rate as i32);
    output_frame
}

pub fn encode_pcm_data(
    pcm_data: &[u8],
    pcm_audio_info: &AudioInfo,
    audio_parameters: &AudioParameters,
    output_path: &Path,
) -> Result<()> {
    let output_path = CString::new(output_path.as_str()).unwrap();

    let encoder = AVCodec::find_encoder(audio_parameters.codecpar.codec_id)
        .with_context(|| anyhow!("encoder({}) not found.", audio_parameters.codecpar.codec_id))?;
    let mut encode_context =
        init_encode_context(&encoder, &audio_parameters).context("Init encode context failed.")?;

    let mut output_format_context = AVFormatContextOutput::create(&output_path)
        .context("Create output format context failed.")?;

    if let Some(output_format) = AVOutputFormat::guess_format(None, Some(&output_path), None) {
        output_format_context.set_oformat(output_format);
    }

    // Some container formats (like MP4) require global headers to be present.
    // Mark the encoder so that it behaves accordingly.
    if output_format_context.oformat().flags & ffi::AVFMT_GLOBALHEADER as i32 != 0 {
        encode_context.set_flags(encode_context.flags | ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32);
    }

    {
        let mut new_audio_stream = output_format_context.new_stream();
        // Use extracted codecpar from encode_context since it contains
        // extradata(adts header when encoding aac), while codecpar from
        // AVStream of input_format_context doesn't.
        new_audio_stream.set_codecpar(encode_context.extract_codecpar());
        new_audio_stream.set_time_base(audio_parameters.time_base);
    }
    output_format_context
        .write_header(&mut None)
        .context("Write header failed.")?;

    let resample_context = init_resample_context(audio_parameters, pcm_audio_info)
        .context("Init encode resample context failed.")?;

    let samples_per_batch = encode_context.frame_size as usize;
    let sample_size = pcm_audio_info.sample_size * pcm_audio_info.ch_layout.nb_channels as usize;
    let num_samples = pcm_data.len() / sample_size;
    let num_batches = (num_samples + samples_per_batch - 1) / samples_per_batch;
    let size_per_batch = samples_per_batch * sample_size;

    let mut sample_offset = 0;
    let mut pts = 0;

    for i in 0..num_batches {
        let process_samples = samples_per_batch.min(num_samples - sample_offset);
        let begin = i * size_per_batch;
        let len = process_samples * sample_size;

        let input_frame = create_input_frame(
            process_samples,
            pcm_audio_info,
            &pcm_data[begin..begin + len],
        );
        let mut output_frame = create_output_frame(audio_parameters);

        resample_context
            .convert_frame(Some(&input_frame), &mut output_frame)
            .context("Convert pcm frame to output frame failed.")?;

        output_frame.set_pts(pts);

        if output_frame.nb_samples > 0 {
            write_frame(
                &mut output_format_context,
                &mut encode_context,
                Some(&output_frame),
            )
            .context("Write frame failed.")?;
        }

        pts += output_frame.nb_samples as i64;
        sample_offset += process_samples;
    }

    // Flushing resample context
    {
        let mut output_frame = create_output_frame(audio_parameters);

        resample_context
            .convert_frame(None, &mut output_frame)
            .context("Flushing resample context failed.")?;

        output_frame.set_pts(pts);

        if output_frame.nb_samples > 0 {
            write_frame(
                &mut output_format_context,
                &mut encode_context,
                Some(&output_frame),
            )
            .context("Write frame failed.")?;
        }
    }

    write_frame(&mut output_format_context, &mut encode_context, None)
        .context("Flush encode_context failed.")?;

    output_format_context
        .write_trailer()
        .context("Write trailer failed.")?;

    Ok(())
}
