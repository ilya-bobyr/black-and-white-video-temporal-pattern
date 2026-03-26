//! Each pattern uses a different shader for processing.
//!
//! This module helps abstract this functionality away.

use wgpu::{Buffer, Device, ShaderModule};

use crate::patterns::Pattern;

mod regular_2f2w2h;

pub(crate) fn shader_params(
    device: &Device,
    width: u32,
    height: u32,
    output_pixel_size: u32,
    pattern: Pattern,
) -> Buffer {
    match pattern {
        Pattern::Regular2f2w2h => {
            regular_2f2w2h::create_params_buffer(device, width, height, output_pixel_size)
        }
    }
}

pub(crate) fn create_shader(
    device: &Device,
    pattern: Pattern,
) -> ShaderModule {
    match pattern {
        Pattern::Regular2f2w2h => regular_2f2w2h::create_shader(device)
    }
}
