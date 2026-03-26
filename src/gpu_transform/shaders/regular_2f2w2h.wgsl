struct Params {
    width: u32,
    height: u32,
    output_pixel_size: u32,
    _luma_upper_bounds_pad: u32,
    luma_activations: array<array<vec4<u32>, 2>, 4>,
}

@group(0) @binding(0) var input_tex : texture_2d_array<u32>;
@group(0) @binding(1) var output_tex : texture_storage_2d_array<rgba8uint, write>;
@group(0) @binding(2) var<uniform> params : Params;

// Each thread owns an cell that is (output_pixel_size * 2) pixels wide and (output_pixel_size * 2)
// pixels tall, for 2 concequitive frames ,covered by a single input frame group.  The whole frame
// group is packaged into `input_tex` as array elements, and a single shader execution owns the same
// are on all of the frames of the frame group.
//
// global_invocation_id.xy are cell coordinates, not pixel coordinates.
@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let cell_x = gid.x;
    let cell_y = gid.y;

    let width: u32 = params.width;
    let height: u32 = params.height;
    let output_pixel_size: u32 = params.output_pixel_size;
    var luma_activations: array<array<vec4<u32>, 2>, 4> = params.luma_activations;

    let cell_cols = (width + (output_pixel_size - 1u)) / output_pixel_size;
    let cell_rows = (height + (output_pixel_size - 1u)) / output_pixel_size;

    // Discard threads beyond the cell grid (happens at image edges).
    if cell_x >= cell_cols || cell_y >= cell_rows {
        return;
    }

    let cell_w = output_pixel_size * 2 /* pattern width */;
    let cell_h = output_pixel_size * 2 /* pattern height */;
    let base_x = cell_x * cell_w;
    let base_y = cell_y * cell_h;

    // Accumulate grayscale luminance over the whole 3d matrix of pixels that we are going to
    // replace.
    // Note that on the spatial axies our cell is `output_pixel_size` times bigger than just 2 by 2.
    // This is due to our output "pixels" having size of `output_pixel_size` pixels on each side.
    //
    // Unless we are at the end of the video (cutting time axis) or at the edge of the frame, we
    // expect `count` to equal `output_pixel_size * 2* output_pixel_size * 2 * 2`, or
    // `output_pixel_size * 8`.
    var count = 0u;
    var total_r = 0u;
    var total_g = 0u;
    var total_b = 0u;

    // This shader operates on 2 frames at a time.
    for (var frame_i = 0u; frame_i < 2; frame_i++) {
        // Our cells are 2 "pixels" wide, but we need to scale by
        // `output_pixel_size`.
        for (var dy = 0u; dy < cell_w; dy++) {
            // Our cells are 2 "pixels" tall, but we need to scale by
            // `output_pixel_size`.
            for (var dx = 0u; dx < cell_h; dx++) {
                let px = base_x + dx;
                let py = base_y + dy;
                if px < width && py < height {
                    let c = textureLoad(input_tex,
                        vec2<i32>(i32(px), i32(py)), frame_i, 0);
                    total_r += c.r;
                    total_g += c.g;
                    total_b += c.b;
                    count += 1u;
                }
            }
        }
    }

    /*
     * 3 decimal digits after comma.
     *
     * Max `total_{r,g,b}` should be `255 * count`.
     * Max `count` is `output_pixel_size * 8`.
     * With `output_pixel_size` of 4, maximum `count` is 32.
     *
     * (299 + 587 + 114) * (255 * 32) = 1_000 * 8_160 = 8_160_000
     *
     * Maximum value for `total_luma` is `8_160_000` which is below u32::MAX.
     *
     * Formula is mostly from
     *
     * https://stackoverflow.com/questions/596216/formula-to-determine-perceived-brightness-of-rgb-color
     */
    let total_luma = (299 * total_r + 587 * total_g + 114 * total_b);
    let avg_luma = total_luma / count;

    // Output pixles use `luma_activations` to decide when they need to light up.  But the whole
    // loop is further complicated by the need to draw larger pixles of `output_pixel_size` size.
    // This `output_pixel_size` parametrization is certainly making this logic more complex,
    // compared to if the `output_pixel_size` would have been a compile time constant.
    for (var frame_i = 0u; frame_i < 2; frame_i++) {
        for (var dy = 0u; dy < 2; dy++) {
            for (var sy = 0u; sy < output_pixel_size; sy++) {
                for (var dx = 0u; dx < 2; dx++) {
                    for (var sx = 0u; sx < output_pixel_size; sx++) {
                        let py = base_y + (dy * output_pixel_size + sy);
                        let px = base_x + (dx * output_pixel_size + sx);

                        // Rotate patterns in a 2 by 2 grid.
                        let pattern_i = cell_y % 2 * 2 + cell_x % 2;

                        // `avg_luma` uses 3 digits after comma, while `luma_upper_bound` is just
                        // whole numbers.  So we need to scale here.
                        let luma_activation =
                            luma_activations[pattern_i][frame_i][dy * 2 + dx] * 1000;

                        let out_color = select(
                            vec4(0u),        // below cutoff -> black
                            vec4(255u),      // at/above cutoff -> white
                            avg_luma >= luma_activation
                        );

                        if px < params.width && py < params.height {
                            textureStore(output_tex,
                                vec2<i32>(i32(px), i32(py)), frame_i, out_color);
                        }
                    }
                }
            }
        }
    }
}
