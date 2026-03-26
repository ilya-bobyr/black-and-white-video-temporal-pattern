mod shaders;

use std::{iter, ops::Deref, sync::Arc};

use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use crossbeam_channel as cb;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBindingType,
    BufferDescriptor, BufferUsages, COPY_BYTES_PER_ROW_ALIGNMENT, CommandEncoderDescriptor,
    ComputePassDescriptor, ComputePipeline, ComputePipelineDescriptor, Device, DeviceDescriptor,
    Extent3d, ImageCopyBuffer, ImageCopyTexture, ImageDataLayout, Origin3d,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PowerPreference, RequestAdapterOptions,
    ShaderStages, StorageTextureAccess, Texture, TextureAspect, TextureDescriptor,
    TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureViewDimension,
};

use crate::{
    messages::{EncodeWork, TransformWork},
    patterns::Pattern,
};

/// This needs to be in sync with the `@workgroup_size` specified as an attribute of the shader
/// entry point.
const WORKGROUP_SIZE: u32 = 16;

/// The output texture uses 4 byte per pixel, but we only look at the first byte of each 4 byte
/// group and treat it as luminosity.  WGSL requires atomics to operate on a single byte texels.  It
/// would be more efficient to pack 4 luminosity values per texel, but it is not done yet.
const OUTPUT_BYTES_PER_PIXEL: u32 = 4;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Params {
    width: u32,
    height: u32,
    output_pixel_size: u32,
}

pub(crate) struct GpuContext {
    device: Device,
    queue: wgpu::Queue,
    pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
}

impl GpuContext {
    pub(crate) async fn new(pattern: Pattern) -> Result<Self> {
        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("No suitable GPU adapter found"))?;

        println!("Using adapter: {}", adapter.get_info().name);

        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        let shader = shaders::create_shader(&device, pattern);

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("bgl"),
            entries: &[
                // binding 0 – input texture (read-only)
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Uint,
                        view_dimension: TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 1 – output texture (storage, write-only)
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba8Uint,
                        view_dimension: TextureViewDimension::D2Array,
                    },
                    count: None,
                },
                // binding 2 – uniform params
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("compute_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
            compilation_options: PipelineCompilationOptions::default(),
        });

        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
        })
    }
}

pub(crate) struct ImageProcessingParams {
    pub(crate) pattern: Pattern,
    pub(crate) output_pixel_size: u32,
}

pub(crate) fn work_thread(
    gpu_context: Arc<GpuContext>,
    params: ImageProcessingParams,
    transform_queue: cb::Receiver<TransformWork>,
    encode_quque: cb::Sender<EncodeWork>,
) {
    let GpuContext {
        device,
        queue,
        pipeline,
        bind_group_layout,
    } = gpu_context.deref();

    let ImageProcessingParams {
        pattern,
        output_pixel_size,
    } = params;

    let mut pending_work = None;

    'context_setup: loop {
        let (new_width, new_height) = match &pending_work {
            None => match transform_queue.recv() {
                Ok(new_work) => {
                    let new_work_width = new_work.width;
                    let new_work_height = new_work.height;
                    pending_work = Some(new_work);
                    (new_work_width, new_work_height)
                }
                Err(_err) => break,
            },
            Some(pending_work) => (pending_work.width, pending_work.height),
        };

        let ctx = prepare_image_processing_ctx(
            device,
            bind_group_layout,
            new_width,
            new_height,
            output_pixel_size,
            pattern,
        );

        loop {
            let (input_images, output_paths) = match pending_work.take() {
                Some(TransformWork {
                    input_images,
                    output_paths,
                    ..
                }) => (input_images, output_paths),
                None => {
                    let Ok(TransformWork {
                        width,
                        height,
                        input_images,
                        output_paths,
                    }) = transform_queue.recv()
                    else {
                        break 'context_setup;
                    };

                    if width != ctx.width || height != ctx.height {
                        pending_work = Some(TransformWork {
                            width,
                            height,
                            input_images,
                            output_paths,
                        });
                        continue 'context_setup;
                    }

                    (input_images, output_paths)
                }
            };

            let output_images = process_images_on_gpu(
                device,
                queue,
                pipeline,
                &ctx,
                pattern,
                output_pixel_size,
                input_images,
            );

            let send_res = encode_quque.send(EncodeWork {
                width: ctx.width,
                height: ctx.height,
                output_images,
                output_paths,
            });
            match send_res {
                Ok(()) => (),
                Err(err) => {
                    println!(
                        "ERROR: sending transformed data to the encode threads.\n\
                         Details: {err}",
                    );
                    break;
                }
            }
        }
    }
}

struct ImageProcessingCtx {
    width: u32,
    height: u32,
    input_tex: Texture,
    output_tex: Texture,
    readback_buf: Buffer,
    readback_buf_bytes_per_row: u32,
    bind_group: BindGroup,
}

fn prepare_image_processing_ctx(
    device: &Device,
    bind_group_layout: &BindGroupLayout,
    width: u32,
    height: u32,
    output_pixel_size: u32,
    pattern: Pattern,
) -> ImageProcessingCtx {
    let frame_group_size = pattern.frame_group_size();

    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: frame_group_size,
    };

    let input_tex = device.create_texture(&TextureDescriptor {
        label: Some("input_tex"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Uint,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let output_tex = device.create_texture(&TextureDescriptor {
        label: Some("output_tex"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Uint,
        usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let (readback_buf_bytes_per_row, readback_buf_size) = {
        let bytes_per_row = width * OUTPUT_BYTES_PER_PIXEL;
        let bytes_per_row = bytes_per_row.next_multiple_of(COPY_BYTES_PER_ROW_ALIGNMENT);
        let buf_size = frame_group_size * bytes_per_row * height;

        (bytes_per_row, buf_size)
    };

    let readback_buf = device.create_buffer(&BufferDescriptor {
        label: Some("readback_buf"),
        size: u64::from(readback_buf_size),
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let params_buf = shaders::shader_params(device, width, height, output_pixel_size, pattern);

    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("bind_group"),
        layout: bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&input_tex.create_view(&Default::default())),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::TextureView(
                    &output_tex.create_view(&Default::default()),
                ),
            },
            BindGroupEntry {
                binding: 2,
                resource: params_buf.as_entire_binding(),
            },
        ],
    });

    ImageProcessingCtx {
        width,
        height,
        input_tex,
        output_tex,
        readback_buf,
        readback_buf_bytes_per_row,
        bind_group,
    }
}

fn process_images_on_gpu(
    device: &Device,
    queue: &wgpu::Queue,
    pipeline: &ComputePipeline,
    ctx: &ImageProcessingCtx,
    pattern: Pattern,
    output_pixel_size: u32,
    images: Vec<Vec<u8>>,
) -> Vec<Vec<u8>> {
    let &ImageProcessingCtx {
        width,
        height,
        ref input_tex,
        ref output_tex,
        ref readback_buf,
        readback_buf_bytes_per_row,
        ref bind_group,
    } = ctx;

    let frame_group_size = pattern.frame_group_size();

    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: frame_group_size,
    };

    for (i, image) in iter::zip(0.., images.into_iter()) {
        queue.write_texture(
            ImageCopyTexture {
                texture: &input_tex,
                mip_level: 0,
                origin: Origin3d { x: 0, y: 0, z: i },
                aspect: TextureAspect::All,
            },
            &image,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: None,
            },
            Extent3d {
                width,
                height,
                // We are writing one array element at a time.
                depth_or_array_layers: 1,
            },
        );
    }

    let cell_cols = width.div_ceil(output_pixel_size * pattern.width());
    let cell_rows = height.div_ceil(output_pixel_size * pattern.height());

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(
            cell_cols.div_ceil(WORKGROUP_SIZE),
            cell_rows.div_ceil(WORKGROUP_SIZE),
            1,
        );
    }
    encoder.copy_texture_to_buffer(
        output_tex.as_image_copy(),
        ImageCopyBuffer {
            buffer: &readback_buf,
            layout: ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(readback_buf_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        size,
    );

    let submission_index = queue.submit(iter::once(encoder.finish()));

    let slice = readback_buf.slice(..);
    let (tx, rx) = oneshot::channel();

    slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });

    device.poll(wgpu::Maintain::WaitForSubmissionIndex(submission_index));

    rx.recv().unwrap().expect("mapping failed");

    let mut output_images = Vec::with_capacity(frame_group_size as usize);
    let readback_data = slice.get_mapped_range();
    for image_idx in 0..frame_group_size {
        let mut image = Vec::with_capacity((width * height) as usize);
        let image_data_offset = image_idx * readback_buf_bytes_per_row * height;
        for y in 0..height {
            for x in 0..width {
                let at = (image_data_offset
                    + y * readback_buf_bytes_per_row
                    + x * OUTPUT_BYTES_PER_PIXEL) as usize;
                image.push(readback_data[at]);
            }
        }
        output_images.push(image);
    }
    // `readback_data` needs to be destroyed before `readback_buf` is unmapped.
    drop(readback_data);

    readback_buf.unmap();

    output_images
}
