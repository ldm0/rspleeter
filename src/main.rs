mod decode;
mod encode;
mod splitter;
mod utils;

use std::fs;

use anyhow::{Context, Result};
use camino::Utf8PathBuf as PathBuf;
use clap::Parser;
use tracing::info;

use crate::{
    splitter::{existing_models, SpleeterModelInfo},
    utils::{AudioData, AudioInfo},
};

#[derive(Parser)]
struct Cli {
    input: PathBuf,
    out_dir: PathBuf,
    #[clap(long, short, default_value = "2stems", value_parser = existing_models())]
    model_name: String,
    #[clap(long, short, default_value = "models/models")]
    models_dir: PathBuf,
}

fn main() -> Result<()> {
    let color = supports_color::on(supports_color::Stream::Stdout).is_some()
        && supports_color::on(supports_color::Stream::Stderr).is_some();
    tracing_subscriber::fmt()
        .with_ansi(color)
        .with_env_filter("info")
        .init();

    let cli = Cli::parse();
    fs::create_dir_all(&cli.out_dir).context("Create output dir failed.")?;

    let pcm_sample_rate = 44100;
    let audio_path = &cli.input;
    let model_name = &cli.model_name;
    let out_dir = &cli.out_dir;
    let audio_extension = audio_path
        .extension()
        .context("Audio path with no extension")?;

    let pcm_audio_info = AudioInfo::new_pcm(pcm_sample_rate);

    let (original_audio_parameters, pcm_data) =
        decode::decode_audio(audio_path, &pcm_audio_info).context("Decode audio failed.")?;

    let samples = pcm_data
        .chunks_exact(4)
        .map(|x| x.try_into().unwrap())
        .map(f32::from_le_bytes)
        .collect();
    let audio_data = AudioData::new(
        samples,
        pcm_audio_info.ch_layout.nb_channels as usize,
        pcm_audio_info.sample_rate,
    );

    let model_info =
        SpleeterModelInfo::get_by_name(model_name).context("Cannot find model info")?;

    let transformed_samples = splitter::split_pcm_audio(&audio_data, model_info, &cli.models_dir)
        .context("Split pcm audio failed.")?;

    for (track_name, pcm_data) in model_info
        .track_names
        .iter()
        .cloned()
        .zip(transformed_samples.into_iter())
    {
        let output_path = out_dir.join(format!("{}.{}", track_name, audio_extension));
        info!("Writing: {}", output_path);
        let pcm_data: Vec<u8> = pcm_data.iter().map(|x| x.to_le_bytes()).flatten().collect();
        // std::fs::write(output_path, &sample_data).context("Write pcm file failed.")?;
        encode::encode_pcm_data(
            &pcm_data,
            &pcm_audio_info,
            &original_audio_parameters,
            &output_path,
        )
        .context("Encode pcm data failed.")?;
    }

    Ok(())
}
