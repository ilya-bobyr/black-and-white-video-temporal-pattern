//! A shader for the `Regular2f2w2h` pattern.
//!
//! Uses 2 consecutive frames, with the cells size 2 by 2.  This provides 8 pixels to represent
//! levels of gray.

use bytemuck::{Pod, Zeroable};
use wgpu::{
    Buffer, BufferUsages, Device, ShaderModule, ShaderModuleDescriptor, ShaderSource,
    util::{BufferInitDescriptor, DeviceExt as _},
};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub(crate) struct Params {
    width: u32,
    height: u32,
    output_pixel_size: u32,
    _luma_upper_bounds_pad: u32,
    /// These are luma values to compare against, when deciding when to light up a pixel.
    ///
    /// We need to use different patterns across the frame, otherwise the whole frame synchronizes,
    /// causing flickering.  We are using flickering to convey grey levels, but it needs to be
    /// de-synchronized in order to avoid flickering the whole frame at the same time.
    ///
    /// TODO How may patterns do we really need here?
    ///
    /// WGSL requires array elements to be aligned on 16 bytes.  And there is no `vec16<u8>` and no
    /// `u8` type in the WGSL.  I can pack 4 luma bounds into a single `u32` - the smallest value
    /// WGSL supports.  But it probably does not save all that much, as we are looking at
    /// parameters that are constructed only once.
    ///
    /// So, for simplicity, `[u32; 4]` carries information for a single frame.
    /// Pixel coordinates are `y * 2 + x`.
    ///
    /// Next level index is the frame index.  Which we have only two.
    /// Next level is the pattern index - these vary based on the x,y coordinates of the cell, in
    /// order to avoid synchronization of the flickering of the frame as a whole.
    luma_activations: [[[u32; 4]; 2]; 4],
}

pub(crate) fn create_params_buffer(
    device: &Device,
    width: u32,
    height: u32,
    output_pixel_size: u32,
) -> Buffer {
    // Lists upper bounds for 8 pixels that form a pattern.
    // The luminosity range of 0..256 is split into 8 regions, 32 units wide each.
    #[rustfmt::skip]
    let luma_activations = [
        /* pattern 0 */
        [
            /* frame 0 */
            [
                (32 * 0) + 31, (32 * 2) + 31,
                (32 * 3) + 31, (32 * 1) + 31,
            ],
            /* frame 1 */
            [
                (32 * 4) + 31, (32 * 6) + 31,
                (32 * 7) + 31, (32 * 5) + 31,
            ],
        ],
        /* pattern 1 */
        [
            /* frame 0 */
            [
                (32 * 4) + 31, (32 * 6) + 31,
                (32 * 7) + 31, (32 * 5) + 31,
            ],
            /* frame 1 */
            [
                (32 * 0) + 31, (32 * 2) + 31,
                (32 * 3) + 31, (32 * 1) + 31,
            ],
        ],
        /* pattern 2 */
        [
            /* frame 0 */
            [
                (32 * 1) + 31, (32 * 3) + 31,
                (32 * 2) + 31, (32 * 0) + 31,
            ],
            /* frame 1 */
            [
                (32 * 5) + 31, (32 * 7) + 31,
                (32 * 6) + 31, (32 * 4) + 31,
            ],
        ],
        /* pattern 3 */
        [
            /* frame 0 */
            [
                (32 * 5) + 31, (32 * 7) + 31,
                (32 * 6) + 31, (32 * 4) + 31,
            ],
            /* frame 1 */
            [
                (32 * 1) + 31, (32 * 3) + 31,
                (32 * 2) + 31, (32 * 0) + 31,
            ],
        ],
    ];

    let params = Params {
        width,
        height,
        output_pixel_size,
        _luma_upper_bounds_pad: 0,
        luma_activations,
    };

    device.create_buffer_init(&BufferInitDescriptor {
        label: Some("params_buf"),
        contents: bytemuck::bytes_of(&params),
        usage: BufferUsages::UNIFORM,
    })
}

pub(super) fn create_shader(device: &Device) -> ShaderModule {
    device.create_shader_module(ShaderModuleDescriptor {
        label: Some("compute_shader"),
        source: ShaderSource::Wgsl(include_str!("./regular_2f2w2h.wgsl").into()),
    })
}
