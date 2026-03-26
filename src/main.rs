mod decode;
mod dispatch;
mod encode;
mod frame;
mod gpu_transform;
mod messages;
mod patterns;

use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicU64},
    thread,
};

use anyhow::{Context as _, Result};
use clap::Parser;
use crossbeam_channel as cb;

use crate::{frame::InputFramePaths, gpu_transform::GpuContext, patterns::Pattern};

const FRAME_NR_MARKER: &'static str = "{frame_nr}";

#[derive(Parser)]
#[command(name = "temporal-pattern")]
/// Creates a black and white pattern that conveys original luminosity information of the image.
///
/// Pattern is constructed by splitting the image into a grid, and using time as an extra dimension,
/// to turn the grid into a box, or a 3d matrix.
///
/// Grid size is used to define the size of the matrix, and, for now, the matrix size is identical
/// in all dimensions.
struct Cli {
    /// Path template for the input images.
    ///
    /// It must contain a "{frame_nr}" in the file name portion, to indicate frame sequence numbers.
    /// Files that match the specified file name, replacing the "{frame_nr}" portion with an
    /// arbitrary sequence of digits (leading zeros are ignored) are included in processed list.
    /// And the "{frame_nr}" portion is interpreted as the frame number.
    #[arg(long)]
    input_template: PathBuf,

    /// Start processing frames from this one.
    #[arg(long, default_value_t = 1)]
    frame_start: u64,

    /// End processing frames at this one, if specified.
    #[arg(long)]
    frame_end: Option<u64>,

    /// Directory where processed frames will be written.
    #[arg(long)]
    output: PathBuf,

    /// Time/space pattern to use to represent the shades of grey.
    #[arg(long, default_value_t = Pattern::Regular2f2w2h)]
    pattern: Pattern,

    /// Size of the "pixels" in the output image.  In order to help YouTube compression, we can
    /// produce "pixels" that are 2x2 or 3x3.  This reduces the resolution of the output image even
    /// further, on top of what the selected `pattern` is already doing.  But is reasonable for a 4K
    /// video uploaded to YouTube, for example.
    #[arg(long, default_value_t = 2)]
    output_pixel_size: u32,

    /// Number of parallel PNG decoder threads.
    ///
    /// Only affects processing speed.
    #[arg(long, default_value_t = 20)]
    decoders: usize,

    /// Number of frame groups to transform on the GPU simultaneously.
    ///
    /// For patterns that do not group frames, this would be just the number of simultaneously
    /// processed frames.
    ///
    /// Only affects processing speed.
    #[arg(long, default_value_t = 8)]
    transformers: usize,

    /// Number of parallel PNG encoder threads.
    ///
    /// Only affects processing speed.
    #[arg(long, default_value_t = 10)]
    encoders: usize,
}

fn main() -> Result<()> {
    let Cli {
        input_template: input_template_path,
        frame_start,
        frame_end,
        output: output_path,
        pattern,
        output_pixel_size,
        decoders: decoder_count,
        transformers: transformers_count,
        encoders: encoder_count,
    } = Cli::parse();

    let InputFramePaths {
        grouped_frame_paths,
        total_frames,
    } = frame::collect_frames(
        &input_template_path,
        frame_start,
        frame_end,
        pattern.frame_group_size(),
    )?;

    println!(
        "Found {total_frames} frame(s).\n\
         Running {decoder_count} decoders, {transformers_count} transformers, and {encoder_count} \
         encoders.",
    );

    std::fs::create_dir_all(&output_path)
        .with_context(|| format!("Creating directory at: {}", output_path.to_string_lossy()))?;

    let gpu_context = Arc::new(pollster::block_on(GpuContext::new(pattern))?);

    // Bound channels to cap memory usage.  Hopefully this would be enough pending work for the
    // pipeline to be running all the time.
    let (decode_work_tx, decode_work_rx) = cb::bounded::<messages::DecodeWork>(decoder_count * 4);
    let (transform_tx, transform_rx) =
        cb::bounded::<messages::TransformWork>(transformers_count * 4);
    let (encode_work_tx, encode_work_rx) = cb::bounded::<messages::EncodeWork>(encoder_count * 4);

    thread::spawn(move || {
        dispatch::work_thread(grouped_frame_paths, output_path, decode_work_tx);
    });

    for _ in 0..decoder_count {
        let work_rx = decode_work_rx.clone();
        let transform_tx = transform_tx.clone();
        thread::spawn(move || {
            decode::work_thread(work_rx, transform_tx);
        });
    }
    drop(decode_work_rx);
    drop(transform_tx);

    for _ in 0..transformers_count {
        let gpu_context = gpu_context.clone();
        let params = gpu_transform::ImageProcessingParams {
            pattern,
            output_pixel_size,
        };
        let transform_rx = transform_rx.clone();
        let encode_work_tx = encode_work_tx.clone();
        thread::spawn(move || {
            gpu_transform::work_thread(gpu_context, params, transform_rx, encode_work_tx);
        });
    }
    drop(transform_rx);
    drop(encode_work_tx);

    let progress = Arc::new(AtomicU64::new(0));

    let encoder_handles = (0..encoder_count)
        .map(move |_| {
            let work_rx = encode_work_rx.clone();
            let progress = progress.clone();
            thread::spawn(move || {
                encode::work_thread(work_rx, progress, total_frames);
            })
        })
        .collect::<Vec<_>>();

    // Wait for the pipeline to run.
    for handle in encoder_handles {
        handle.join().unwrap();
    }

    println!("Done.");
    Ok(())
}
