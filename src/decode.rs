use crossbeam_channel as cb;
use image;

use crate::messages::{DecodeWork, TransformWork};

// Decoder thread.
//
// We start multiple threads running this function.  These threads run PNG decoding and then forward
// raw image bytes to the GPU interaction thread.  Results arrive at the GPU thread out of order,
// but it does not matter, as the shaders have all the data they need.
pub(crate) fn work_thread(
    decode_queue: cb::Receiver<DecodeWork>,
    transform_queue: cb::Sender<TransformWork>,
) {
    'work_item: for DecodeWork {
        input_paths,
        output_paths,
    } in &decode_queue
    {
        let mut input_images = Vec::with_capacity(input_paths.len());
        let mut width = None;
        let mut height = None;

        if input_paths.is_empty() {
            println!("ERROR: decoding thread received DecodeWork with no input image paths.");
            continue;
        }

        for input_path in input_paths {
            let image = image::open(&input_path);
            let image = match image {
                Ok(image) => image,
                Err(err) => {
                    println!(
                        "ERROR: decoding image data in: {}\n\
                         Details: {}",
                        input_path.to_string_lossy(),
                        err,
                    );
                    continue 'work_item;
                }
            };

            if let Some(width) = width
                && width != image.width()
            {
                println!(
                    "ERROR: Mismatched width between two images.\n\
                     Processing image: {}\n\
                     Image width:    {}\n\
                     Previous width: {}",
                    input_path.to_string_lossy(),
                    width,
                    image.width(),
                );
                continue 'work_item;
            }

            if let Some(height) = height
                && height != image.height()
            {
                println!(
                    "ERROR: Mismatched height between two images.\n\
                     Processing image: {}\n\
                     Image height:    {}\n\
                     Previous height: {}",
                    input_path.to_string_lossy(),
                    height,
                    image.height(),
                );
                continue 'work_item;
            }

            width = Some(image.width());
            height = Some(image.height());

            input_images.push(image.to_rgba8().into_raw());
        }

        let send_res = transform_queue.send(TransformWork {
            width: width.expect("We checked `input_paths` to be non-empty"),
            height: height.expect("We checked `input_paths` to be non-empty"),
            input_images,
            output_paths,
        });
        match send_res {
            Ok(()) => (),
            Err(err) => {
                println!(
                    "ERROR: sending decoded data to the transformation threads.\n\
                     Details: {err}",
                );
                break;
            }
        }
    }
}
