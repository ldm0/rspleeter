use anyhow::{Context, Result};
use camino::Utf8Path as Path;
use once_cell::sync::Lazy;
use tensorflow::SessionOptions;
use tensorflow::SessionRunArgs;
use tensorflow::Tensor;
use tensorflow::{Graph, SavedModelBundle};
use tracing::info;

use crate::utils::AudioData;

pub struct SpleeterModelInfo {
    pub name: &'static str,
    pub output_count: usize,
    pub output_names: Vec<&'static str>,
    pub track_names: Vec<&'static str>,
}

/// Check https://github.com/deezer/spleeter/issues/155#issuecomment-565178677
static MODEL_INFOS: Lazy<[SpleeterModelInfo; 6]> = Lazy::new(|| {
    [
        SpleeterModelInfo {
            name: "2stems",
            output_count: 2,
            output_names: vec!["strided_slice_13", "strided_slice_23"],
            track_names: vec!["vocals", "accompaniment"],
        },
        SpleeterModelInfo {
            name: "4stems",
            output_count: 4,
            output_names: vec![
                "strided_slice_13",
                "strided_slice_23",
                "strided_slice_33",
                "strided_slice_43",
            ],
            track_names: vec!["vocals", "drums", "bass", "other"],
        },
        SpleeterModelInfo {
            name: "5stems",
            output_count: 5,
            output_names: vec![
                "strided_slice_18",
                "strided_slice_38",
                "strided_slice_48",
                "strided_slice_28",
                "strided_slice_58",
            ],
            track_names: vec!["vocals", "drums", "bass", "piano", "other"],
        },
        SpleeterModelInfo {
            name: "2stems-16kHz",
            output_count: 2,
            output_names: vec!["strided_slice_13", "strided_slice_23"],
            track_names: vec!["vocals", "accompaniment"],
        },
        SpleeterModelInfo {
            name: "4stems-16kHz",
            output_count: 4,
            output_names: vec![
                "strided_slice_13",
                "strided_slice_23",
                "strided_slice_33",
                "strided_slice_43",
            ],
            track_names: vec!["vocals", "drums", "bass", "other"],
        },
        SpleeterModelInfo {
            name: "5stems-16kHz",
            output_count: 5,
            output_names: vec![
                "strided_slice_18",
                "strided_slice_38",
                "strided_slice_48",
                "strided_slice_28",
                "strided_slice_58",
            ],
            track_names: vec!["vocals", "drums", "bass", "piano", "other"],
        },
    ]
});

pub fn existing_models() -> Vec<&'static str> {
    MODEL_INFOS.iter().map(|x| x.name).collect()
}

impl SpleeterModelInfo {
    pub fn get_by_name(model_name: &str) -> Option<&'static SpleeterModelInfo> {
        MODEL_INFOS.iter().find(|info| info.name == model_name)
    }
}

pub fn split_pcm_audio(
    audio_data: &AudioData,
    model_info: &SpleeterModelInfo,
    models_dir: &Path,
) -> Result<Vec<Vec<f32>>> {
    let tensorflow_version = tensorflow::version().unwrap();
    info!(?tensorflow_version);

    let slice_length = audio_data.sample_rate * 30;
    let extend_length = audio_data.sample_rate * 5;
    let nb_channels = audio_data.nb_channels;

    let mut transformed_samples = vec![vec![]; model_info.output_count];

    let model_path = models_dir.join(model_info.name);
    let mut graph = Graph::new();
    let session = SavedModelBundle::load(&SessionOptions::new(), ["serve"], &mut graph, model_path)
        .context("Cannot load session")?
        .session;

    let input_samples_count_per_channel = audio_data.samples.len() / audio_data.nb_channels;
    let segment_count = (input_samples_count_per_channel + (slice_length - 1)) / slice_length;

    for i in 0..segment_count {
        let current_offset = slice_length * i;
        let extend_length_at_begin = if i == 0 { 0 } else { extend_length };
        let extend_length_at_end = if i == (segment_count - 1) {
            0
        } else {
            extend_length
        };

        let useful_start = extend_length_at_begin;
        let useful_length = if i == (segment_count - 1) {
            input_samples_count_per_channel - current_offset
        } else {
            slice_length
        };

        let process_start = current_offset - extend_length_at_begin;
        let process_length = (useful_length + extend_length_at_begin + extend_length_at_end)
            .min(input_samples_count_per_channel - process_start);

        info!(
            "processing: [{}, {}), using [{}, {})",
            process_start,
            process_start + process_length,
            current_offset,
            current_offset + useful_length
        );

        let oper = graph
            .operation_by_name("Placeholder")
            .context("Get operation failed")?
            .context("Get empty operation")?;
        let input_dims = [process_length as u64, nb_channels as u64];

        let input_data_length = process_length * nb_channels;
        let input_data_begin = process_start * nb_channels;
        let input_data =
            &audio_data.samples[input_data_begin..input_data_begin + input_data_length];

        let input_tensors = Tensor::new(&input_dims)
            .with_values(input_data)
            .context("Get tensor failed.")?;

        let mut output_tokens = Vec::new();

        let mut run_args = SessionRunArgs::new();
        run_args.add_feed(&oper, 0, &input_tensors);

        for i in 0..model_info.output_count {
            let oper = graph
                .operation_by_name(model_info.output_names[i])
                .context("Get operation failed")?
                .context("Get empty operation")?;
            let fetch_token = run_args.request_fetch(&oper, 0);
            output_tokens.push(fetch_token);
        }

        session.run(&mut run_args).context("Run session failed")?;

        for i in 0..model_info.output_count {
            let data: Tensor<f32> = run_args
                .fetch(output_tokens[i])
                .context("Get output failed")?;
            let begin = useful_start * nb_channels;
            let len = useful_length * nb_channels;
            transformed_samples[i].extend_from_slice(&data.as_ref()[begin..begin + len]);
        }
        info!("{}/{} done...", i + 1, segment_count);
    }
    Ok(transformed_samples)
}
