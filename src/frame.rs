use anyhow::{Context as _, Result, bail};
use regex::Regex;
use std::path::{Path, PathBuf};

use crate::FRAME_NR_MARKER;

pub(crate) struct InputFramePaths {
    /// Frames are grouped sequentially, as specified by the processing logic.
    pub(crate) grouped_frame_paths: Vec<Vec<PathBuf>>,
    pub(crate) total_frames: u64,
}

/// Collect all recognised image files from `dir`, sorted by filename.
///
/// Groups images into groups of `pattern_group_frames` elements.  
pub(crate) fn collect_frames(
    input_template: &Path,
    frame_start: u64,
    frame_end: Option<u64>,
    pattern_frame_group: u32,
) -> Result<InputFramePaths> {
    if let Some(frame_end) = frame_end {
        if frame_start > frame_end {
            bail!(
                "`frame_end` must be at or after `frame_start`.\n\
                 `frame_start`: {frame_start}\n\
                 `frame_end`: {frame_end}",
            );
        }
    }

    let file_name = input_template
        .file_name()
        .with_context(|| {
            format!(
                "`input_template`: File name must be present.  Got: {}",
                input_template.to_string_lossy()
            )
        })?
        .to_string_lossy();

    let file_name_re = {
        let pos = file_name.find(FRAME_NR_MARKER).with_context(|| {
            format!(
                "`input_template`: File name must contain the \"{}\".  Got: {}",
                FRAME_NR_MARKER,
                input_template.to_string_lossy()
            )
        })?;
        Regex::new(&format!(
            "{}([[:digit:]]+){}",
            regex::escape(&file_name[0..pos]),
            regex::escape(&file_name[(pos + FRAME_NR_MARKER.len())..])
        ))
        .expect("Regex is valid")
    };

    let current_dir = PathBuf::from(".");
    let input_dir = match input_template.parent() {
        Some(v) => v,
        None => &current_dir,
    };

    let mut total_frames = 0;
    let mut grouped_frame_paths: Vec<Vec<Option<PathBuf>>> = match frame_end {
        Some(frame_end) => {
            let total_frames = frame_end + 1 - frame_start;
            Vec::with_capacity(total_frames as usize)
        }
        None => Vec::new(),
    };
    for entry in std::fs::read_dir(input_dir)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }

        let path_str = path.to_string_lossy();
        let Some(caps) = file_name_re.captures(&path_str) else {
            continue;
        };

        let frame_nr = {
            let frame_nr_str = &caps[1];
            let res = frame_nr_str.trim_start_matches('0').parse::<u64>();
            res.with_context(|| {
                format!(
                    "Path contains a digit sequence that does not fit into `u64`: {path_str}\n\
                     Failed to parse: \"{frame_nr_str}\""
                )
            })?
        };

        if frame_nr < frame_start {
            continue;
        }
        if let Some(frame_end) = frame_end
            && frame_nr > frame_end
        {
            continue;
        }

        let group_idx: u64 = (frame_nr - frame_start) / u64::from(pattern_frame_group);
        let index_in_group: u64 = (frame_nr - frame_start) % u64::from(pattern_frame_group);

        if group_idx as usize >= grouped_frame_paths.len() {
            grouped_frame_paths.resize(group_idx as usize + 1, Vec::new());
        }
        let grouped_paths = &mut grouped_frame_paths[group_idx as usize];

        if index_in_group as usize >= grouped_paths.len() {
            grouped_paths.resize(index_in_group as usize + 1, None);
        }
        if let Some(existing_path) = &grouped_paths[index_in_group as usize] {
            bail!(
                "Two file path for the same frame:\n\
                 First:  {}\n\
                 Second: {}",
                existing_path.to_string_lossy(),
                path_str
            );
        }

        grouped_paths[index_in_group as usize] = Some(path);
        total_frames += 1;
    }

    let grouped_frame_paths = {
        let groups = grouped_frame_paths.into_iter();
        let mut frame_nr = frame_start;

        let mut res = Vec::with_capacity(total_frames as usize);

        for group in groups {
            let mut group_frames = Vec::with_capacity(pattern_frame_group as usize);

            for frame_path in group {
                match frame_path {
                    None => {
                        bail!(
                            "Missing frame file for frame {}\n\
                             Path template: {}",
                            frame_nr,
                            input_template.to_string_lossy(),
                        );
                    }
                    Some(frame_path) => {
                        group_frames.push(frame_path);
                        frame_nr += 1;
                    }
                }
            }

            res.push(group_frames);
        }

        res
    };

    Ok(InputFramePaths {
        grouped_frame_paths,
        total_frames,
    })
}
