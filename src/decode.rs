use anyhow::{Context, Result};
use camino::Utf8Path as Path;
use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext, AVPacket},
    avformat::AVFormatContextInput,
    avutil::AVSamples,
    error::RsmpegError,
    ffi::{self},
    swresample::SwrContext,
};
use std::ffi::CString;
use std::slice::from_raw_parts;

use crate::utils::AudioInfo;
use crate::utils::AudioParameters;

fn samples_to_pcm(samples: &AVSamples, sample_size: usize) -> Result<&[u8]> {
    let nb_samples = samples.nb_samples as usize;
    let nb_channels = samples.nb_channels as usize;

    // This is safe since we are expecting interleaved audio formats.
    Ok(unsafe {
        from_raw_parts(
            samples.audio_data[0],
            nb_channels * nb_samples * sample_size,
        )
    })
}

fn decode_resample_save(
    output_audio_info: &AudioInfo,
    decode_context: &mut AVCodecContext,
    resample_context: &mut SwrContext,
    packet: Option<&AVPacket>,
    pcm_data: &mut Vec<u8>,
) -> Result<()> {
    decode_context
        .send_packet(packet)
        .context("Send packet failed.")?;
    loop {
        let frame = match decode_context.receive_frame() {
            Ok(frame) => frame,
            Err(RsmpegError::DecoderDrainError) | Err(RsmpegError::DecoderFlushedError) => break,
            Err(e) => return Err(e).context("Receive frame failed."),
        };

        let mut output_samples = AVSamples::new(
            output_audio_info.ch_layout.nb_channels,
            frame.nb_samples,
            output_audio_info.sample_fmt,
            0,
        )
        .context("Create samples buffer failed.")?;

        unsafe {
            resample_context
                .convert(
                    output_samples.audio_data.as_mut_ptr(),
                    output_samples.nb_samples,
                    frame.extended_data as *const _,
                    frame.nb_samples,
                )
                .context("Convert sample failed.")?;
        }

        let data = samples_to_pcm(&output_samples, output_audio_info.sample_size)
            .context("Samples to pcm failed.")?;
        pcm_data.extend_from_slice(data);
    }
    Ok(())
}

/// Change samples to specified format(stereo, interleaved, 44.1khz).
fn init_resample_context(
    decode_context: &AVCodecContext,
    output_audio_info: &AudioInfo,
) -> Result<SwrContext> {
    let mut resample_context = SwrContext::new(
        &output_audio_info.ch_layout,
        output_audio_info.sample_fmt,
        output_audio_info.sample_rate as i32,
        &decode_context.ch_layout,
        decode_context.sample_fmt,
        decode_context.sample_rate,
    )
    .context("SwrContext parameters incorrect.")?;
    resample_context
        .init()
        .context("Init resample context failed.")?;
    Ok(resample_context)
}

fn init_decode_context(
    decoder: &AVCodec,
    audio_parameters: &AudioParameters,
) -> Result<AVCodecContext> {
    let mut decode_context = AVCodecContext::new(&decoder);
    decode_context
        .apply_codecpar(&audio_parameters.codecpar)
        .context("Apply codecpar failed.")?;
    decode_context.set_time_base(audio_parameters.time_base);
    decode_context
        .open(None)
        .context("Open codec context failed.")?;
    Ok(decode_context)
}

/// Result<(original_audio_info, pcm_data)>
pub fn decode_audio(
    audio_path: &Path,
    output_audio_info: &AudioInfo,
) -> Result<(AudioParameters, Vec<u8>)> {
    // unwrap: &str ensures no internal null bytes.
    let audio_path = CString::new(audio_path.as_str()).unwrap();
    let mut input_format_context =
        AVFormatContextInput::open(&audio_path).context("Open audio file failed.")?;

    input_format_context.dump(0, &audio_path)?;

    let (stream_index, decoder) = input_format_context
        .find_best_stream(ffi::AVMEDIA_TYPE_AUDIO)
        .context("Find best stream failed.")?
        .context("Cannot find audio stream in this file.")?;

    let audio_parameters = {
        let stream = input_format_context.streams().get(stream_index).unwrap();
        let codecpar = stream.codecpar().clone();
        let time_base = stream.time_base;
        AudioParameters {
            time_base,
            codecpar,
        }
    };
    let mut decode_context =
        init_decode_context(&decoder, &audio_parameters).context("Init decode context failed.")?;

    let mut resample_context = init_resample_context(&decode_context, &output_audio_info)
        .context("Init resample context failed")?;

    let mut pcm_data = Vec::new();

    while let Some(packet) = input_format_context
        .read_packet()
        .context("Read packet failed")?
    {
        if packet.stream_index == stream_index as i32 {
            decode_resample_save(
                &output_audio_info,
                &mut decode_context,
                &mut resample_context,
                Some(&packet),
                &mut pcm_data,
            )
            .context("Decode failed.")?;
        }
    }

    decode_resample_save(
        &output_audio_info,
        &mut decode_context,
        &mut resample_context,
        None,
        &mut pcm_data,
    )
    .context("Flush decode context failed.")?;

    Ok((audio_parameters, pcm_data))
}
