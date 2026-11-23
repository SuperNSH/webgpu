//! wgpu 29: a vertex buffer layout with no attributes before a used one
//! shifts the used one's slot.
//!
//! The pipeline's `vertex.buffers` is `[slot 0: no attributes, slot 1: one
//! attribute at @location(0)]`. Slot 0 is bound to a buffer of red, slot 1
//! to a buffer of green. The attribute belongs to slot 1, so the triangle
//! must be green. wgpu-core leaves slot 0 out of the HAL pipeline
//! (`device/resource.rs`, `create_render_pipeline`: `if
//! vb_state.attributes.is_empty() { continue; }`) without renumbering the
//! slots after it, so the HAL's buffer 0 is slot 1's layout, fed from the
//! buffer bound at slot 0: the triangle comes out red.
//!
//! WebGPU allows such a hole (`GPUVertexState.buffers` is
//! `sequence<GPUVertexBufferLayout?>`; a null entry or one with no
//! attributes keeps the slot numbers of the entries after it). The control
//! draw puts the green buffer's layout at slot 0 alone and is green.
//!
//! `cargo run`: prints the pixel of each draw; exits 1 when the bug shows.

const SHADER: &str = "
struct Out { @builtin(position) position: vec4f, @location(0) color: vec4f };
@vertex fn vs(@builtin(vertex_index) i: u32, @location(0) color: vec4f) -> Out {
  var pos = array<vec2f, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
  return Out(vec4f(pos[i], 0.0, 1.0), color);
}
@fragment fn fs(in: Out) -> @location(0) vec4f { return in.color; }";

const SIZE: u32 = 4;

fn main() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("an adapter");
    println!(
        "adapter: {:?} ({:?})",
        adapter.get_info().name,
        adapter.get_info().backend
    );
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("a device");

    let colors = |rgba: [f32; 4]| -> wgpu::Buffer {
        let bytes: Vec<u8> = (0..3)
            .flat_map(|_| rgba)
            .flat_map(f32::to_le_bytes)
            .collect();
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&buffer, 0, &bytes);
        buffer
    };
    let red = colors([1.0, 0.0, 0.0, 1.0]);
    let green = colors([0.0, 1.0, 0.0, 1.0]);

    let color_attribute = [wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 0,
        shader_location: 0,
    }];
    let used = wgpu::VertexBufferLayout {
        array_stride: 16,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &color_attribute,
    };
    // WebGPU's `null` slot: no attributes.
    let hole = wgpu::VertexBufferLayout {
        array_stride: 0,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[],
    };

    // Control: the green layout alone at slot 0.
    let control = draw(&device, &queue, &[used.clone()], &[&green]);
    // The bug: a hole at slot 0, the green layout at slot 1.
    let with_hole = draw(&device, &queue, &[hole, used], &[&red, &green]);

    println!("control   (slot 0 = green layout):             {control:?}");
    println!("with hole (slot 0 = no attributes, slot 1 = green): {with_hole:?}");
    let green_px = [0, 255, 0, 255];
    assert_eq!(control, green_px, "the control draw is not green");
    if with_hole == green_px {
        println!("OK: slot 1's attribute read slot 1's buffer");
    } else {
        println!("BUG: slot 1's attribute read the buffer bound at slot 0");
        std::process::exit(1);
    }
}

/// Draws a full-target triangle with `layouts`, `buffers[i]` bound at slot
/// `i`; the centre pixel as RGBA8.
fn draw(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layouts: &[wgpu::VertexBufferLayout<'_>],
    buffers: &[&wgpu::Buffer],
) -> [u8; 4] {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: layouts,
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(format.into())],
        }),
        multiview_mask: None,
        cache: None,
    });
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256 * u64::from(SIZE),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let view = target.create_view(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        for (slot, buffer) in buffers.iter().enumerate() {
            pass.set_vertex_buffer(slot as u32, buffer.slice(..));
        }
        pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: None,
            },
        },
        target.size(),
    );
    queue.submit([encoder.finish()]);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    let data = readback.slice(..).get_mapped_range();
    let at = 256 * (SIZE as usize / 2) + 4 * (SIZE as usize / 2);
    [data[at], data[at + 1], data[at + 2], data[at + 3]]
}
