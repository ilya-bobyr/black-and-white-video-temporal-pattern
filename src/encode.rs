use std::{
    iter,
    path::Path,
    sync::{
        Arc,
        atomic::{self, AtomicU64},
    },
};

use crossbeam_channel as cb;
use image;

use crate::messages::EncodeWork;

pub(crate) fn work_thread(
    encode_queue: cb::Receiver<EncodeWork>,
    progress: Arc<AtomicU64>,
    total_frames: u64,
) {
    let total_frames_width = (total_frames as f64).log10().floor() as usize;

    for EncodeWork {
        width,
        height,
        output_images,
        output_paths,
    } in &encode_queue
    {

        for (image_luma, output_path) in iter::zip(output_images, output_paths) {
            try_encode_and_save(width, height, image_luma, &output_path);

            let progress = progress.fetch_add(1, atomic::Ordering::Relaxed) + 1;
            println!(
                "[{:>total_frames_width$}/{:>total_frames_width$}]: {}",
                progress,
                total_frames,
                output_path.display()
            );
        }
    }
}

fn try_encode_and_save(width: u32, height: u32, image_luma: Vec<u8>, output_path: &Path) {
    let image_luma_len = image_luma.len();
    let image = match image::GrayImage::from_raw(width, height, image_luma) {
        Some(image) => image,
        None => {
            println!(
                "ERROR: encoding image data for: {}\n\
                 Details: Image data is not big enough.\n\
                 Input data size: {}\n\
                 Specified width and height: {} x {}",
                output_path.to_string_lossy(),
                image_luma_len,
                width,
                height,
            );
            return;
        }
    };

    match image.save(&output_path) {
        Ok(()) => (),
        Err(err) => {
            println!(
                "ERROR: writing image data for: {}\n\
                 Details: {}",
                output_path.to_string_lossy(),
                err,
            );
            return;
        }
    }
}
