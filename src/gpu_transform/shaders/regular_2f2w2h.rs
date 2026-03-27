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
    /// `f32` is an overkill for luma activations.  We could have easily used a single byte, by
    /// writing just the numerator of an `n / 256` fraction.  But WGSL requires array elements to be
    /// aligned on 16 bytes.  And there is no `vec16<u8>` and no `u8` type in the WGSL.  I can pack
    /// 4 luma bounds into a single `u32` - the smallest value WGSL supports.  But it probably does
    /// not save all that much, as we are looking at parameters that are constructed only once.
    ///
    /// So, for simplicity, `[f32; 4]` carries information for a single frame.
    /// Pixel coordinates are `y * 2 + x`.
    ///
    /// Next level index is the frame index.  Which we have only two.
    /// Next level is the pattern index - these vary based on the x,y coordinates of the cell, in
    /// order to avoid synchronization of the flickering of the frame as a whole.
    luma_activations: [[[f32; 4]; 2]; 4],
}

pub(crate) fn create_params_buffer(
    device: &Device,
    width: u32,
    height: u32,
    output_pixel_size: u32,
) -> Buffer {
    const LEVELS_OF_GREY: u8 = 8;

    // A helper to write luma activations with less noise.
    //
    // Returns an upper bound that is still considered the specified luma level.
    const fn on(level: u8) -> f32 {
        debug_assert!(level < LEVELS_OF_GREY);

        // TODO When https://github.com/rust-lang/rust/issues/143874 is resolved:
        //
        //   f32::from(level + 1) / f32::from(LEVELS_OF_GREY)
        //
        ((level + 1) as f32) / (LEVELS_OF_GREY as f32)
    }

    // Lists upper bounds for 8 pixels that form a pattern.
    #[rustfmt::skip]
    let luma_activations = [
        /* pattern 0 */
        [
            /* frame 0 */
            [
                on(0), on(2),
                on(3), on(1),
            ],
            /* frame 1 */
            [
                on(4), on(6),
                on(7), on(5),
            ],
        ],
        /* pattern 1 */
        [
            /* frame 0 */
            [
                on(4), on(6),
                on(7), on(5),
            ],
            /* frame 1 */
            [
                on(0), on(2),
                on(3), on(1),
            ],
        ],
        /* pattern 2 */
        [
            /* frame 0 */
            [
                on(1), on(3),
                on(2), on(0),
            ],
            /* frame 1 */
            [
                on(5), on(7),
                on(6), on(4),
            ],
        ],
        /* pattern 3 */
        [
            /* frame 0 */
            [
                on(5), on(7),
                on(6), on(4),
            ],
            /* frame 1 */
            [
                on(1), on(3),
                on(2), on(0),
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
