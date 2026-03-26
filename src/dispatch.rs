use std::path::PathBuf;

use crossbeam_channel as cb;

use crate::messages::DecodeWork;

/// Dispatcher thread.  Enumerates frame paths and sends them as work items with sequence numbers.
pub(crate) fn work_thread(
    grouped_frame_paths: Vec<Vec<PathBuf>>,
    output_dir: PathBuf,
    decode_queue: cb::Sender<DecodeWork>,
) {
    for input_paths in grouped_frame_paths.into_iter() {
        let output_paths: Vec<_> = input_paths
            .iter()
            .map(|input_path| {
                output_dir.join(input_path.file_name().expect(
                    "collect_frames() already checked that all input path have the file name \
                 component",
                ))
            })
            .collect();

        let send_res = decode_queue.send(DecodeWork {
            input_paths,
            output_paths,
        });
        match send_res {
            Ok(()) => (),
            Err(err) => {
                println!(
                    "ERROR: sending frame path to the decoder threads.\n\
                     Details: {err}",
                );
                break;
            }
        }
    }
}
