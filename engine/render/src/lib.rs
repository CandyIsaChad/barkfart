//! GPU renderer for the engine's draw lists (wgpu: Metal on Apple, Vulkan or
//! GLES on Android and Linux, DX12 on Windows, WebGPU in browsers).
//!
//! The first version renders offscreen and reads the pixels back, which is
//! all the Flutter link needs to start with; sharing the GPU texture with
//! Flutter directly comes later per platform.

use barkfart_core::DrawList;
use wgpu::util::DeviceExt;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const SAMPLES: u32 = 4;

/// Which GPU and API the renderer ended up on.
#[derive(Debug, Clone)]
pub struct AdapterReport {
    pub name: String,
    pub backend: String,
    pub driver: String,
}

/// Render targets and readback buffer for one size, kept between frames.
struct Targets {
    w: u32,
    h: u32,
    msaa: wgpu::TextureView,
    resolved: wgpu::Texture,
    readback: wgpu::Buffer,
    padded: u32,
}

pub struct Renderer {
    targets: std::sync::Mutex<Option<Targets>>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    pub adapter: AdapterReport,
}

#[derive(Debug)]
pub struct RenderError(pub String);

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RenderError {}

/// The visible area in drawing units (y up) and the background colour.
#[derive(Debug, Clone, Copy)]
pub struct View {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    /// Straight RGBA, 0..1.
    pub background: [f32; 4],
}

impl Renderer {
    /// A headless renderer. Tries the given backends one at a time in order
    /// (e.g. Vulkan, then GL) and keeps the first that gives a device.
    pub fn headless(order: &[wgpu::Backends]) -> Result<Renderer, RenderError> {
        let mut errors = Vec::new();
        for &backends in order {
            match pollster::block_on(Self::try_backends(backends)) {
                Ok(r) => return Ok(r),
                Err(e) => errors.push(format!("{backends:?}: {e}")),
            }
        }
        Err(RenderError(format!("no GPU device: {}", errors.join("; "))))
    }

    /// Vulkan / Metal / DX12 first, then GL.
    pub fn headless_default() -> Result<Renderer, RenderError> {
        Self::headless(&[wgpu::Backends::PRIMARY, wgpu::Backends::GL])
    }

    async fn try_backends(backends: wgpu::Backends) -> Result<Renderer, RenderError> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = backends;
        let instance = wgpu::Instance::new(desc);
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .map_err(|e| RenderError(e.to_string()))?;
        let info = adapter.get_info();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("barkfart"),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|e| RenderError(e.to_string()))?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shapes"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("view"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shapes"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shapes"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 12,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Unorm8x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: FORMAT,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
            multiview_mask: None,
            cache: None,
        });
        Ok(Renderer {
            targets: std::sync::Mutex::new(None),
            device,
            queue,
            pipeline,
            layout,
            adapter: AdapterReport {
                name: info.name,
                backend: format!("{:?}", info.backend),
                driver: format!("{} {}", info.driver, info.driver_info),
            },
        })
    }

    /// Draws the list over the background into a w x h image and returns
    /// straight RGBA8, rows top-down.
    pub fn render(&self, list: &DrawList, w: u32, h: u32, view: View) -> Vec<u8> {
        let size = wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 };
        let mut cache = self.targets.lock().unwrap_or_else(|e| e.into_inner());
        if !matches!(&*cache, Some(t) if t.w == size.width && t.h == size.height) {
            let target = |samples: u32, usage: wgpu::TextureUsages| {
                self.device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size,
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format: FORMAT,
                    usage,
                    view_formats: &[],
                })
            };
            let padded = (size.width * 4).div_ceil(256) * 256;
            *cache = Some(Targets {
                w: size.width,
                h: size.height,
                msaa: target(SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT).create_view(&Default::default()),
                resolved: target(1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC),
                readback: self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("readback"),
                    size: (padded * size.height) as u64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                padded,
            });
        }
        let tg = cache.as_ref().expect("targets");
        let sx = 2.0 / (view.x1 - view.x0);
        let sy = 2.0 / (view.y1 - view.y0);
        let uniform: [f32; 4] = [sx, sy, -1.0 - view.x0 * sx, -1.0 - view.y0 * sy];
        let ubuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("view"),
            contents: bytemuck::cast_slice(&uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() }],
        });
        let mut bytes = Vec::with_capacity(list.vertices.len() * 12);
        for v in &list.vertices {
            bytes.extend_from_slice(&v.pos[0].to_le_bytes());
            bytes.extend_from_slice(&v.pos[1].to_le_bytes());
            bytes.extend_from_slice(&v.rgba);
        }
        if bytes.is_empty() {
            bytes.resize(12, 0);
        }
        let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertices"),
            contents: &bytes,
            usage: wgpu::BufferUsages::VERTEX,
        });
        let row = size.width * 4;
        let padded = tg.padded;
        let readback = &tg.readback;
        let bg = view.background;
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mv = &tg.msaa;
            let rv = tg.resolved.create_view(&Default::default());
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: mv,
                    depth_slice: None,
                    resolve_target: Some(&rv),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: (bg[0] * bg[3]) as f64,
                            g: (bg[1] * bg[3]) as f64,
                            b: (bg[2] * bg[3]) as f64,
                            a: bg[3] as f64,
                        }),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if !list.vertices.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.set_vertex_buffer(0, vbuf.slice(..));
                pass.draw(0..list.vertices.len() as u32, 0..1);
            }
        }
        enc.copy_texture_to_buffer(
            tg.resolved.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: readback,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: None },
            },
            size,
        );
        self.queue.submit([enc.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let data = match slice.get_mapped_range() {
            Ok(d) => d,
            Err(e) => panic!("readback failed: {e:?}"),
        };
        let mut out = Vec::with_capacity((row * size.height) as usize);
        // GPU rows are top-down already (texture origin is top-left)
        for y in 0..size.height {
            let start = (y * padded) as usize;
            out.extend_from_slice(&data[start..start + row as usize]);
        }
        drop(data);
        readback.unmap();
        unpremultiply(&mut out);
        out
    }
}

fn unpremultiply(px: &mut [u8]) {
    for p in px.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}
