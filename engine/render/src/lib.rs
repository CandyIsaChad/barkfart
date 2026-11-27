//! GPU renderer for the engine's draw lists (wgpu: Metal on Apple, Vulkan or
//! GLES on Android and Linux, DX12 on Windows, WebGPU in browsers).
//!
//! Mirrors barkfart_core::fx (the reference): every isolate draws into its
//! own target (multisampled, with a resolved copy), its effects run as
//! full-screen passes limited to the isolate's padded rectangle, and the
//! result is composited into its parent with its opacity and blend mode.
//! Frames are read back as pixels (sharing the GPU texture with Flutter
//! directly comes later).

pub mod glyphs;

use barkfart_core::eval::{ParamValue, ResolvedEffect};
use barkfart_core::fx as rfx;
use barkfart_core::model::{Blend, Color};
use barkfart_core::tess::{IsoItem, Isolate};
use barkfart_core::DrawList;
use wgpu::util::DeviceExt;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const BLENDS: [Blend; 4] = [Blend::Normal, Blend::Add, Blend::Multiply, Blend::Screen];

/// Which GPU and API the renderer ended up on.
#[derive(Debug, Clone)]
pub struct AdapterReport {
    pub name: String,
    pub backend: String,
    pub driver: String,
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

#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Samples per pixel for shapes: 4 for smooth edges, 1 to match the reference renderer.
    pub samples: u32,
}

impl Default for Options {
    fn default() -> Self {
        Options { samples: 4 }
    }
}

fn blend_state(b: Blend) -> wgpu::BlendState {
    use wgpu::{BlendComponent as C, BlendFactor as F, BlendOperation as O};
    let alpha = C { src_factor: F::One, dst_factor: F::OneMinusSrcAlpha, operation: O::Add };
    match b {
        Blend::Normal => wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
        Blend::Add => wgpu::BlendState {
            color: C { src_factor: F::One, dst_factor: F::One, operation: O::Add },
            alpha: C { src_factor: F::One, dst_factor: F::One, operation: O::Add },
        },
        Blend::Multiply => wgpu::BlendState { color: C { src_factor: F::Dst, dst_factor: F::OneMinusSrcAlpha, operation: O::Add }, alpha },
        Blend::Screen => wgpu::BlendState { color: C { src_factor: F::One, dst_factor: F::OneMinusSrc, operation: O::Add }, alpha },
    }
}

fn blend_index(b: Blend) -> usize {
    BLENDS.iter().position(|x| *x == b).unwrap_or(0)
}

/// Pixel rectangle (x, y, w, h), top-left origin.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

/// An isolate's target: multisampled drawing surface plus its resolved copy.
struct Target {
    msaa: Option<wgpu::Texture>,
    color: wgpu::Texture,
}

struct Pipelines {
    batch: Vec<wgpu::RenderPipeline>,
    composite: Vec<wgpu::RenderPipeline>,
    replace: wgpu::RenderPipeline,
    final_over: wgpu::RenderPipeline,
    fx: std::collections::HashMap<&'static str, wgpu::RenderPipeline>,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    options: Options,
    shape_layout: wgpu::BindGroupLayout,
    fx_layout: wgpu::BindGroupLayout,
    pipes: Pipelines,
    white: wgpu::TextureView,
    sampler: wgpu::Sampler,
    /// Free textures by (width, height, samples), reused between frames.
    pool: std::sync::Mutex<Vec<(u32, u32, u32, wgpu::Texture)>>,
    /// Images by id (sprite assets, "glyphs:<page>"); missing ids draw in plain colour.
    textures: std::sync::Mutex<std::collections::HashMap<String, wgpu::TextureView>>,
    /// K2: plugin effects' pipelines by plugin id, with the plugin list's
    /// generation they were made for (None: the shader didn't compile).
    plugin_pipes: std::sync::Mutex<std::collections::HashMap<String, (u64, Option<wgpu::RenderPipeline>)>>,
    fx_pl: wgpu::PipelineLayout,
    pub adapter: AdapterReport,
}

/// Work recorded for one frame.
struct Frame<'a> {
    r: &'a Renderer,
    enc: wgpu::CommandEncoder,
    w: u32,
    h: u32,
    /// View to pixels: px = (x - x0) * s, py = (y0 - y) * s.
    x0: f32,
    y0: f32,
    s: f32,
    view_bind: wgpu::BindGroup,
    /// Bind groups for textured batches, by texture id.
    tex_binds: std::collections::HashMap<String, wgpu::BindGroup>,
    vbuf: wgpu::Buffer,
    /// First vertex of each batch in vbuf.
    starts: Vec<u32>,
}

impl Renderer {
    /// A headless renderer. Tries the given backends one at a time in order
    /// (e.g. Vulkan, then GL) and keeps the first that gives a device.
    pub fn headless(order: &[wgpu::Backends]) -> Result<Renderer, RenderError> {
        Self::headless_with(order, Options::default())
    }

    pub fn headless_with(order: &[wgpu::Backends], options: Options) -> Result<Renderer, RenderError> {
        let mut errors = Vec::new();
        for &backends in order {
            match pollster::block_on(Self::try_backends(backends, options)) {
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

    async fn try_backends(backends: wgpu::Backends, options: Options) -> Result<Renderer, RenderError> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = backends;
        let instance = wgpu::Instance::new(desc);
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await.map_err(|e| RenderError(e.to_string()))?;
        let info = adapter.get_info();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("barkfart"),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|e| RenderError(e.to_string()))?;
        let samples = if options.samples >= 4 { 4 } else { 1 };
        let options = Options { samples };
        let shape_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shapes"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let fx_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("fx"), source: wgpu::ShaderSource::Wgsl(include_str!("fx.wgsl").into()) });
        let tex_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let uniform_entry = |vis| wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: vis,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let shape_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shapes"),
            entries: &[
                uniform_entry(wgpu::ShaderStages::VERTEX),
                tex_entry(1),
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let fx_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fx"),
            entries: &[uniform_entry(wgpu::ShaderStages::FRAGMENT), tex_entry(1), tex_entry(2)],
        });
        let shape_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&shape_layout)], immediate_size: 0 });
        let fx_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&fx_layout)], immediate_size: 0 });
        let shape_pipe = |blend: Blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("batch"),
                layout: Some(&shape_pl),
                vertex: wgpu::VertexState {
                    module: &shape_mod,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 20,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Unorm8x4, 2 => Float32x2],
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shape_mod,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: FORMAT, blend: Some(blend_state(blend)), write_mask: wgpu::ColorWrites::ALL })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
                multiview_mask: None,
                cache: None,
            })
        };
        let full_pipe = |entry: &str, blend: Option<wgpu::BlendState>, count: u32| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&fx_pl),
                vertex: wgpu::VertexState { module: &fx_mod, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
                fragment: Some(wgpu::FragmentState {
                    module: &fx_mod,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: FORMAT, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState { count, ..Default::default() },
                multiview_mask: None,
                cache: None,
            })
        };
        let mut fx = std::collections::HashMap::new();
        for e in [
            "fs_copy", "fs_lerp", "fs_blur", "fs_shift", "fs_under", "fs_invert", "fs_adjust", "fs_tint", "fs_pixelate", "fs_outline", "fs_motion",
            "fs_chromatic", "fs_vignette", "fs_wave", "fs_grain", "fs_grade", "fs_scanlines",
        ] {
            fx.insert(e, full_pipe(e, None, 1));
        }
        let pipes = Pipelines {
            batch: BLENDS.iter().map(|b| shape_pipe(*b)).collect(),
            composite: BLENDS.iter().map(|b| full_pipe("fs_copy", Some(blend_state(*b)), samples)).collect(),
            replace: full_pipe("fs_lerp", None, samples),
            final_over: full_pipe("fs_copy", Some(blend_state(Blend::Normal)), 1),
            fx,
        };
        let white_tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("white"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[255, 255, 255, 255],
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Ok(Renderer {
            white: white_tex.create_view(&Default::default()),
            sampler,
            device,
            queue,
            options,
            shape_layout,
            fx_layout,
            pipes,
            pool: std::sync::Mutex::new(Vec::new()),
            textures: std::sync::Mutex::new(std::collections::HashMap::new()),
            plugin_pipes: std::sync::Mutex::new(std::collections::HashMap::new()),
            fx_pl: fx_pl.clone(),
            adapter: AdapterReport { name: info.name, backend: format!("{:?}", info.backend), driver: format!("{} {}", info.driver, info.driver_info) },
        })
    }

    /// K2: an effect plugin's pipeline, made once per plugin list generation.
    /// A shader that doesn't compile switches the plugin off with the
    /// compiler's first line, and draws nothing new (the layer passes through).
    fn plugin_pipeline(&self, id: &str) -> Option<(barkfart_script::Definition, wgpu::RenderPipeline)> {
        let (def, generation) = barkfart_core::plugins::effect(id)?;
        let mut cache = self.plugin_pipes.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((g, pipe)) = cache.get(id) {
            if *g == generation {
                return pipe.clone().map(|p| (def, p));
            }
        }
        let source = plugin_wgsl(&def);
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(id), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let pipe = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(id),
            layout: Some(&self.fx_pl),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_plugin"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let failed = pollster::block_on(scope.pop());
        let pipe = match failed {
            None => Some(pipe),
            Some(e) => {
                let text = e.to_string();
                let line = text.lines().find(|l| !l.trim().is_empty() && !l.contains("Validation Error")).unwrap_or("").trim().to_string();
                barkfart_core::plugins::report_failure(id, format!("Its shader didn't compile: {line}"));
                None
            }
        };
        cache.insert(id.to_string(), (generation, pipe.clone()));
        pipe.map(|p| (def, p))
    }

    pub fn options(&self) -> Options {
        self.options
    }

    /// Uploads an image (straight RGBA8, rows top-down) under an id; batches
    /// whose texture is that id sample it. Replaces any image with the id.
    pub fn set_texture(&self, id: &str, w: u32, h: u32, rgba: &[u8]) {
        if w == 0 || h == 0 || rgba.len() < (w * h * 4) as usize {
            return;
        }
        let max = self.device.limits().max_texture_dimension_2d;
        if w > max || h > max {
            return; // too large for this GPU: drawn in plain colour
        }
        let tex = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some(id),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &rgba[..(w * h * 4) as usize],
        );
        self.textures.lock().unwrap_or_else(|e| e.into_inner()).insert(id.to_string(), tex.create_view(&Default::default()));
    }

    pub fn remove_texture(&self, id: &str) {
        self.textures.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    }

    pub fn has_texture(&self, id: &str) -> bool {
        self.textures.lock().unwrap_or_else(|e| e.into_inner()).contains_key(id)
    }

    /// Draws the list (flat or layered) over the background into a w x h
    /// image; straight RGBA8, rows top-down.
    pub fn render(&self, list: &DrawList, w: u32, h: u32, view: View) -> Vec<u8> {
        let mut px = self.render_raw(list, w, h, view);
        unpremultiply(&mut px);
        px
    }

    /// An export frame: drawn `supersample` times larger (capped at 4096 px)
    /// and box-filtered down, so edges are smooth; straight RGBA8 rows
    /// top-down, over the view's background or transparent.
    pub fn render_export(&self, list: &DrawList, w: u32, h: u32, view: View, supersample: u32, transparent: bool) -> Vec<u8> {
        let (w, h) = (w.max(1), h.max(1));
        let mut k = supersample.clamp(1, 4);
        while k > 1 && (w * k > 4096 || h * k > 4096) {
            k -= 1;
        }
        let bg = view.background;
        let big = self.render_raw(list, w * k, h * k, View { background: [0.0; 4], ..view });
        let (bw, n) = ((w * k) as usize, (k * k) as u32);
        let mut out = vec![0u8; (w * h * 4) as usize];
        for y in 0..h as usize {
            for x in 0..w as usize {
                let mut acc = [0u32; 4];
                for dy in 0..k as usize {
                    let row = ((y * k as usize + dy) * bw + x * k as usize) * 4;
                    for dx in 0..k as usize {
                        for c in 0..4 {
                            acc[c] += big[row + dx * 4 + c] as u32;
                        }
                    }
                }
                let px: [u32; 4] = std::array::from_fn(|c| (acc[c] + n / 2) / n);
                let o = (y * w as usize + x) * 4;
                let a = px[3];
                if transparent {
                    for c in 0..3 {
                        out[o + c] = if a > 0 { (px[c] * 255 / a).min(255) as u8 } else { 0 };
                    }
                    out[o + 3] = a as u8;
                } else {
                    for c in 0..3 {
                        let b = (bg[c] * 255.0).round() as u32;
                        out[o + c] = (px[c] + b * (255 - a) / 255).min(255) as u8;
                    }
                    out[o + 3] = 255;
                }
            }
        }
        out
    }

    /// Premultiplied RGBA8 rows top-down (the reference's format; use a
    /// transparent background to compare isolates).
    pub fn render_raw(&self, list: &DrawList, w: u32, h: u32, view: View) -> Vec<u8> {
        let (w, h) = (w.max(1), h.max(1));
        let s = w as f32 / (view.x1 - view.x0);
        let sy = h as f32 / (view.y1 - view.y0);
        let uniform: [f32; 4] = [2.0 / (view.x1 - view.x0), 2.0 / (view.y1 - view.y0), -1.0 - view.x0 * 2.0 / (view.x1 - view.x0), -1.0 - view.y0 * 2.0 / (view.y1 - view.y0)];
        let _ = sy;
        let ubuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("view"), contents: bytemuck::cast_slice(&uniform), usage: wgpu::BufferUsages::UNIFORM });
        let bind_with = |view: &wgpu::TextureView| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.shape_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            })
        };
        let view_bind = bind_with(&self.white);
        let mut tex_binds = std::collections::HashMap::new();
        {
            let textures = self.textures.lock().unwrap_or_else(|e| e.into_inner());
            for b in &list.batches {
                if let Some(id) = &b.texture {
                    if !tex_binds.contains_key(id) {
                        if let Some(v) = textures.get(id) {
                            tex_binds.insert(id.clone(), bind_with(v));
                        }
                    }
                }
            }
        }
        // all vertices in one buffer; textured batches use the white texture for now
        let mut bytes = Vec::new();
        let mut starts = Vec::with_capacity(list.batches.len());
        let mut count = 0u32;
        for b in &list.batches {
            starts.push(count);
            for i in 0..b.vertex_count() {
                bytes.extend_from_slice(&b.xy[i * 2].to_le_bytes());
                bytes.extend_from_slice(&b.xy[i * 2 + 1].to_le_bytes());
                bytes.extend_from_slice(&b.rgba[i].to_be_bytes());
                bytes.extend_from_slice(&b.uv[i * 2].to_le_bytes());
                bytes.extend_from_slice(&b.uv[i * 2 + 1].to_le_bytes());
                count += 1;
            }
        }
        if bytes.is_empty() {
            bytes.resize(20, 0);
        }
        let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("vertices"), contents: &bytes, usage: wgpu::BufferUsages::VERTEX });
        let enc = self.device.create_command_encoder(&Default::default());
        let mut f = Frame { r: self, enc, w, h, x0: view.x0, y0: view.y1, s, view_bind, tex_binds, vbuf, starts };
        let full = Rect { x: 0, y: 0, w, h };
        let root = f.target();
        f.clear(&root);
        if list.isolates.is_empty() {
            // a flat list: every batch straight into the root
            let all: Vec<usize> = (0..list.batches.len()).collect();
            f.batches(list, &all, &root, full);
        } else {
            f.isolate(list, 0, &root, full);
        }
        // over the background
        let out = f.texture(1);
        {
            let bind = f.fx_bind(&[[1.0, 0.0, 0.0, 0.0]], &root.color, None);
            let bg = view.background;
            let v = out.create_view(&Default::default());
            let mut pass = f.enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("final"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &v,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: (bg[0] * bg[3]) as f64, g: (bg[1] * bg[3]) as f64, b: (bg[2] * bg[3]) as f64, a: bg[3] as f64 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipes.final_over);
            pass.set_bind_group(0, &bind, &[]);
            pass.draw(0..3, 0..1);
        }
        let row = w * 4;
        let padded = row.div_ceil(256) * 256;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        f.enc.copy_texture_to_buffer(
            out.as_image_copy(),
            wgpu::TexelCopyBufferInfo { buffer: &readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: None } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        f.release_target(root);
        f.release(out, 1);
        let Frame { enc, .. } = f;
        self.queue.submit([enc.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let mut px = Vec::with_capacity((row * h) as usize);
        {
            let data = match slice.get_mapped_range() {
                Ok(d) => d,
                Err(e) => panic!("readback failed: {e:?}"),
            };
            for y in 0..h {
                let start = (y * padded) as usize;
                px.extend_from_slice(&data[start..start + row as usize]);
            }
        }
        readback.unmap();
        px
    }
}

impl<'a> Frame<'a> {
    fn texture(&mut self, samples: u32) -> wgpu::Texture {
        let (w, h) = (self.w, self.h);
        let mut pool = self.r.pool.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = pool.iter().position(|(pw, ph, ps, _)| *pw == w && *ph == h && *ps == samples) {
            return pool.swap_remove(i).3;
        }
        drop(pool);
        let usage = if samples > 1 {
            wgpu::TextureUsages::RENDER_ATTACHMENT
        } else {
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC
        };
        self.r.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage,
            view_formats: &[],
        })
    }

    /// Back to the pool at once: later passes in this frame may reuse it
    /// (wgpu runs the recorded passes in order, so earlier reads still see
    /// the old contents).
    fn release(&mut self, t: wgpu::Texture, samples: u32) {
        let mut pool = self.r.pool.lock().unwrap_or_else(|e| e.into_inner());
        pool.push((self.w, self.h, samples, t));
        if pool.len() > 64 {
            pool.remove(0);
        }
    }

    fn target(&mut self) -> Target {
        let s = self.r.options.samples;
        Target { msaa: if s > 1 { Some(self.texture(s)) } else { None }, color: self.texture(1) }
    }

    fn release_target(&mut self, t: Target) {
        let s = self.r.options.samples;
        if let Some(m) = t.msaa {
            self.release(m, s);
        }
        self.release(t.color, 1);
    }

    /// A render pass on a target (multisampled, resolving into its colour).
    fn pass<'e>(enc: &'e mut wgpu::CommandEncoder, t: &Target, clear: bool) -> wgpu::RenderPass<'e> {
        let cv = t.color.create_view(&Default::default());
        let mv = t.msaa.as_ref().map(|m| m.create_view(&Default::default()));
        let load = if clear { wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT) } else { wgpu::LoadOp::Load };
        let attachment = match &mv {
            Some(m) => wgpu::RenderPassColorAttachment { view: m, depth_slice: None, resolve_target: Some(&cv), ops: wgpu::Operations { load, store: wgpu::StoreOp::Store } },
            None => wgpu::RenderPassColorAttachment { view: &cv, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load, store: wgpu::StoreOp::Store } },
        };
        enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(attachment)],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }

    fn clear(&mut self, t: &Target) {
        let _ = Self::pass(&mut self.enc, t, true);
    }

    fn fx_bind(&self, params: &[[f32; 4]], t0: &wgpu::Texture, t1: Option<&wgpu::Texture>) -> wgpu::BindGroup {
        let mut v = [[0f32; 4]; 40];
        for (i, p) in params.iter().enumerate().take(40) {
            v[i] = *p;
        }
        let buf = self.r.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("fx"), contents: bytemuck::cast_slice(&v), usage: wgpu::BufferUsages::UNIFORM });
        let v0 = t0.create_view(&Default::default());
        let v1 = t1.unwrap_or(t0).create_view(&Default::default());
        self.r.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.r.fx_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&v0) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&v1) },
            ],
        })
    }

    /// One full-screen effect pass into a new single-sample texture, within rect.
    fn fx_pass(&mut self, entry: &str, params: &[[f32; 4]], t0: &wgpu::Texture, t1: Option<&wgpu::Texture>, rect: Rect) -> wgpu::Texture {
        let out = self.texture(1);
        let bind = self.fx_bind(params, t0, t1);
        let ov = out.create_view(&Default::default());
        let mut pass = self.enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(entry),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &ov,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.r.pipes.fx[entry]);
        pass.set_bind_group(0, &bind, &[]);
        pass.set_scissor_rect(rect.x, rect.y, rect.w, rect.h);
        pass.draw(0..3, 0..1);
        drop(pass);
        out
    }

    /// K2: a plugin effect: its shader over src with its settings (in the
    /// order the plugin lists them, one vec4 each, after the size and time);
    /// missing, off or not compiling, src passes through.
    fn plugin(&mut self, e: &ResolvedEffect, id: &str, src: &wgpu::Texture, rect: Rect) -> wgpu::Texture {
        use barkfart_script::PropKind as K;
        let Some((def, pipe)) = self.r.plugin_pipeline(id) else { return self.copy(src, rect) };
        let num = |n: &str| match e.params.iter().find(|(k, _)| k == n).map(|(_, v)| v) {
            Some(ParamValue::F(x)) => *x,
            Some(ParamValue::D(x)) => *x as f32,
            Some(ParamValue::B(b)) => *b as i32 as f32,
            _ => 0.0,
        };
        let mut v: Vec<[f32; 4]> = vec![[self.w as f32, self.h as f32, num("__seconds"), 0.0]];
        for p in &def.props {
            v.push(match &p.kind {
                K::Colour { .. } => match e.params.iter().find(|(k, _)| *k == p.name).map(|(_, v)| v) {
                    Some(ParamValue::C(c)) => [c.r, c.g, c.b, c.a],
                    _ => [1.0; 4],
                },
                K::Point { .. } => [num(&format!("{}.x", p.name)), num(&format!("{}.y", p.name)), 0.0, 0.0],
                _ => [num(&p.name), 0.0, 0.0, 0.0],
            });
        }
        let out = self.texture(1);
        let bind = self.fx_bind(&v, src, None);
        let ov = out.create_view(&Default::default());
        let mut pass = self.enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(id),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &ov,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipe);
        pass.set_bind_group(0, &bind, &[]);
        pass.set_scissor_rect(rect.x, rect.y, rect.w, rect.h);
        pass.draw(0..3, 0..1);
        drop(pass);
        out
    }

    /// Draws batches (by index) into a target with their blend modes.
    fn batches(&mut self, list: &DrawList, idx: &[usize], t: &Target, clip: Rect) {
        let mut pass = Self::pass(&mut self.enc, t, false);
        pass.set_vertex_buffer(0, self.vbuf.slice(..));
        pass.set_scissor_rect(clip.x, clip.y, clip.w, clip.h);
        for &b in idx {
            let n = list.batches[b].vertex_count() as u32;
            if n == 0 {
                continue;
            }
            let bind = list.batches[b].texture.as_ref().and_then(|id| self.tex_binds.get(id)).unwrap_or(&self.view_bind);
            pass.set_bind_group(0, bind, &[]);
            pass.set_pipeline(&self.r.pipes.batch[blend_index(list.batches[b].blend())]);
            let s = self.starts[b];
            pass.draw(s..s + n, 0..1);
        }
    }

    /// Renders an isolate's items into t (already cleared), within clip.
    fn isolate(&mut self, list: &DrawList, iso: usize, t: &Target, clip: Rect) {
        let items = &list.isolates[iso].items;
        let mut run: Vec<usize> = Vec::new();
        for item in items {
            match *item {
                IsoItem::Batch(b) => run.push(b),
                IsoItem::Isolate(ci) => {
                    if !run.is_empty() {
                        self.batches(list, &run, t, clip);
                        run.clear();
                    }
                    let child = &list.isolates[ci];
                    if child.adjustment {
                        // effects on what is drawn so far, mixed back in by the layer's opacity.
                        // Keep `before` separate from the texture consumed by `effects()` so
                        // multi-pass / stacked effects never recycle `before` from the pool!
                        // When the adjustment isolate has explicit camera-shot bounds (!empty()),
                        // clip tightly to that shot rect; otherwise affect full `clip`.
                        let has_active = child.effects.iter().any(|e| rfx::mix(e) > 0.0 && (rfx::known(&e.ty) || e.ty.starts_with("plugin:")));
                        let op = if child.opacity.is_nan() { 1.0 } else { child.opacity.clamp(0.0, 1.0) };
                        if !has_active || op <= 0.0 {
                            continue;
                        }
                        let adj_rect = if !child.empty() {
                            let Some(r) = self.tight_rect(child, clip) else { continue };
                            r
                        } else {
                            clip
                        };
                        let before = self.copy(&t.color, adj_rect);
                        let start = self.copy(&before, adj_rect);
                        let fx = self.effects(start, &child.effects, adj_rect, true, adj_rect);
                        let bind = self.fx_bind(&[[op, 0.0, 0.0, 0.0]], &before, Some(&fx));
                        {
                            let mut pass = Self::pass(&mut self.enc, t, false);
                            pass.set_pipeline(&self.r.pipes.replace);
                            pass.set_bind_group(0, &bind, &[]);
                            pass.set_scissor_rect(adj_rect.x, adj_rect.y, adj_rect.w, adj_rect.h);
                            pass.draw(0..3, 0..1);
                        }
                        self.release(fx, 1);
                        self.release(before, 1);
                        continue;
                    }
                    let Some(rect) = self.rect(list, child, clip) else { continue };
                    let ct = self.target();
                    self.clear(&ct);
                    self.isolate(list, ci, &ct, rect);
                    let Target { msaa, color } = ct;
                    if let Some(m) = msaa {
                        self.release(m, self.r.options.samples);
                    }
                    let full_rect = Rect { x: 0, y: 0, w: self.w, h: self.h };
                    let done = self.effects(color, &child.effects, rect, false, full_rect);
                    let op = if child.opacity.is_nan() { 1.0 } else { child.opacity.clamp(0.0, 1.0) };
                    let bind = self.fx_bind(&[[op, 0.0, 0.0, 0.0]], &done, None);
                    {
                        let mut pass = Self::pass(&mut self.enc, t, false);
                        pass.set_pipeline(&self.r.pipes.composite[blend_index(child.blend)]);
                        pass.set_bind_group(0, &bind, &[]);
                        pass.set_scissor_rect(rect.x, rect.y, rect.w, rect.h);
                        pass.draw(0..3, 0..1);
                    }
                    self.release(done, 1);
                }
            }
        }
        if !run.is_empty() {
            self.batches(list, &run, t, clip);
        }
    }

    fn copy(&mut self, src: &wgpu::Texture, rect: Rect) -> wgpu::Texture {
        self.fx_pass("fs_copy", &[[1.0, 0.0, 0.0, 0.0]], src, None, rect)
    }

    /// Runs effects in order on src (consumed); returns the result.
    fn effects(&mut self, src: wgpu::Texture, list: &[ResolvedEffect], rect: Rect, full_frame: bool, frame_rect: Rect) -> wgpu::Texture {
        let mut cur = src;
        for e in list {
            let m = rfx::mix(e);
            if m <= 0.0 || !(rfx::known(&e.ty) || e.ty.starts_with("plugin:")) {
                continue;
            }
            let out = self.one(e, &cur, rect, full_frame, frame_rect);
            let out = if m < 1.0 {
                let mixed = self.fx_pass("fs_lerp", &[[m, 0.0, 0.0, 0.0]], &cur, Some(&out), rect);
                self.release(out, 1);
                mixed
            } else {
                out
            };
            let old = std::mem::replace(&mut cur, out);
            self.release(old, 1);
        }
        cur
    }

    fn blurred(&mut self, src: &wgpu::Texture, sigma: f32, rect: Rect) -> wgpu::Texture {
        if !(sigma > 0.05) {
            return self.copy(src, rect);
        }
        let sigma = sigma.min(rfx::MAX_SIGMA);
        let r = (3.0 * sigma).ceil();
        let h = self.fx_pass("fs_blur", &[[1.0, 0.0, sigma, r]], src, None, rect);
        let v = self.fx_pass("fs_blur", &[[0.0, 1.0, sigma, r]], &h, None, rect);
        self.release(h, 1);
        v
    }

    /// One effect: a new texture from src.
    fn one(&mut self, e: &ResolvedEffect, src: &wgpu::Texture, rect: Rect, full_frame: bool, frame_rect: Rect) -> wgpu::Texture {
        let s = self.s;
        let f = |n: &str, d: f32, lo: f32, hi: f32| rfx::f(e, n, d, lo, hi);
        let col = |c: Color| [c.r, c.g, c.b, c.a];
        let ff = if full_frame { 1.0 } else { 0.0 };
        let fr = [frame_rect.x as f32, frame_rect.y as f32, frame_rect.w as f32, frame_rect.h as f32];
        match e.ty.as_str() {
            plugin if plugin.starts_with("plugin:") => self.plugin(e, &plugin["plugin:".len()..], src, rect),
            rfx::BLUR => self.blurred(src, rfx::sigma(f("radius", 6.0, 0.0, rfx::MAX_LENGTH)) * s, rect),
            rfx::GLOW | rfx::SHADOW => {
                let glow = e.ty == rfx::GLOW;
                let c = if glow { rfx::c(e, "color", Color::WHITE) } else { rfx::c(e, "color", rfx::SHADOW_COLOR) };
                let radius = if glow { f("radius", 8.0, 0.0, rfx::MAX_LENGTH) } else { f("radius", 6.0, 0.0, rfx::MAX_LENGTH) };
                let (dx, dy) = if glow {
                    (0.0, 0.0)
                } else {
                    (f("dx", 8.0, -rfx::MAX_LENGTH, rfx::MAX_LENGTH) * s, -f("dy", -8.0, -rfx::MAX_LENGTH, rfx::MAX_LENGTH) * s)
                };
                let strength = if glow { f("strength", 1.0, 0.0, 4.0) } else { 1.0 };
                let (ix, iy) = (barkfart_core::model::java_round(dx) as f32, barkfart_core::model::java_round(dy) as f32);
                let alpha = self.fx_pass("fs_shift", &[[ix, iy, 0.0, 0.0]], src, None, rect);
                let soft = self.blurred(&alpha, rfx::sigma(radius) * s, rect);
                self.release(alpha, 1);
                let out = self.fx_pass("fs_under", &[col(c), [strength, 0.0, 0.0, 0.0]], src, Some(&soft), rect);
                self.release(soft, 1);
                out
            }
            rfx::INVERT => self.fx_pass("fs_invert", &[], src, None, rect),
            rfx::ADJUST => {
                let m = rfx::hue_saturation(f("hue", 0.0, -3600.0, 3600.0), f("saturation", 1.0, 0.0, 4.0));
                let b = f("brightness", 0.0, -1.0, 1.0);
                self.fx_pass("fs_adjust", &[[b, 0.0, 0.0, 0.0], [m[0], m[1], m[2], 0.0], [m[3], m[4], m[5], 0.0], [m[6], m[7], m[8], 0.0]], src, None, rect)
            }
            rfx::TINT => {
                let c = rfx::c(e, "color", rfx::TINT_COLOR);
                self.fx_pass("fs_tint", &[col(c), [f("amount", 0.5, 0.0, 1.0) * c.a, 0.0, 0.0, 0.0]], src, None, rect)
            }
            rfx::PIXELATE => {
                let size = f("size", 8.0, 1.0, rfx::MAX_LENGTH);
                if !(size * s >= 1.5) {
                    return self.copy(src, rect);
                }
                self.fx_pass("fs_pixelate", &[[self.x0, self.y0, s, size]], src, None, rect)
            }
            rfx::OUTLINE => {
                let c = rfx::c(e, "color", Color::BLACK);
                let o = rfx::ring_offsets(f("width", 4.0, 0.0, 64.0) * s);
                let mut p = vec![col(c)];
                for k in 0..16 {
                    p.push([o[k * 4] as f32, o[k * 4 + 1] as f32, o[k * 4 + 2] as f32, o[k * 4 + 3] as f32]);
                }
                self.fx_pass("fs_outline", &p, src, None, rect)
            }
            rfx::MOTION => {
                let o = rfx::smear_offsets(f("angle", 0.0, -3600.0, 3600.0), f("distance", 30.0, 0.0, rfx::MAX_LENGTH) * s);
                let mut p = vec![[0.0; 4]];
                for k in 0..8 {
                    p.push([o[k * 4] as f32, o[k * 4 + 1] as f32, o[k * 4 + 2] as f32, o[k * 4 + 3] as f32]);
                }
                self.fx_pass("fs_motion", &p, src, None, rect)
            }
            rfx::CHROMATIC => {
                let amount_px = f("amount", 8.0, 0.0, 120.0) * s;
                if !(amount_px > 0.01) {
                    return self.copy(src, rect);
                }
                let rad = (f("angle", 0.0, -3600.0, 3600.0) as f64).to_radians();
                let dx = (rad.cos() * amount_px as f64) as f32;
                let dy = -(rad.sin() * amount_px as f64) as f32;
                self.fx_pass("fs_chromatic", &[[dx, dy, 0.0, 0.0]], src, None, rect)
            }
            rfx::VIGNETTE => {
                let c = rfx::c(e, "color", Color::BLACK);
                let amount = f("amount", 0.65, 0.0, 1.0);
                if !(amount > 0.0) || !(c.a > 0.0) {
                    return self.copy(src, rect);
                }
                let softness = f("softness", 0.55, 0.05, 1.0);
                self.fx_pass("fs_vignette", &[col(c), [amount, softness, ff, 0.0], fr], src, None, rect)
            }
            rfx::WAVE => {
                let amp_px = f("amplitude", 12.0, 0.0, 160.0) * s;
                if !(amp_px > 0.01) {
                    return self.copy(src, rect);
                }
                let freq = f("frequency", 3.0, 0.1, 40.0);
                let phase_rad = (f("phase", 0.0, -3600.0, 3600.0) as f64).to_radians() as f32;
                let rad = (f("angle", 0.0, -3600.0, 3600.0) as f64).to_radians();
                let k_freq = freq * std::f32::consts::TAU / 400.0;
                self.fx_pass("fs_wave", &[[self.x0, self.y0, s, amp_px], [k_freq, phase_rad, rad.cos() as f32, rad.sin() as f32]], src, None, rect)
            }
            rfx::GRAIN => {
                let amount = f("amount", 0.25, 0.0, 1.0);
                if !(amount > 0.0) {
                    return self.copy(src, rect);
                }
                let size = f("size", 2.0, 1.0, 24.0);
                let seed_i = barkfart_core::model::java_round(f("seed", 0.0, -10000.0, 10000.0)) as f32;
                self.fx_pass("fs_grain", &[[amount, size, seed_i, ff]], src, None, rect)
            }
            rfx::GRADE => {
                let exp_mul = 2.0f32.powf(f("exposure", 0.0, -4.0, 4.0));
                let contrast = f("contrast", 1.0, 0.0, 4.0);
                let inv_g = 1.0 / f("gamma", 1.0, 0.1, 4.0).max(0.1);
                self.fx_pass("fs_grade", &[[exp_mul, contrast, inv_g, 0.0]], src, None, rect)
            }
            rfx::SCANLINES => {
                let amount = f("amount", 0.45, 0.0, 1.0);
                let offset_px = f("offset", 6.0, 0.0, 80.0) * s;
                if !(amount > 0.0) && !(offset_px > 0.0) {
                    return self.copy(src, rect);
                }
                let lines = f("lines", 80.0, 8.0, 400.0);
                let phase = f("phase", 0.0, -10000.0, 10000.0);
                let phase_i = barkfart_core::model::java_round(phase) as f32;
                self.fx_pass("fs_scanlines", &[[amount, lines, offset_px, phase_i], [phase, ff, fr[1], fr[3]]], src, None, rect)
            }
            _ => self.copy(src, rect),
        }
    }

    /// Tight pixel rectangle for an adjustment isolate's explicit world bounds
    /// (camera shot), intersected with `clip` (None when outside `clip`).
    fn tight_rect(&self, iso: &Isolate, clip: Rect) -> Option<Rect> {
        let (x0, x1) = ((iso.min_x - self.x0) * self.s, (iso.max_x - self.x0) * self.s);
        let (y0, y1) = ((self.y0 - iso.max_y) * self.s, (self.y0 - iso.min_y) * self.s);
        let rx0 = (clip.x as i64).max(x0.floor() as i64);
        let ry0 = (clip.y as i64).max(y0.floor() as i64);
        let rx1 = ((clip.x + clip.w) as i64).min(x1.ceil() as i64);
        let ry1 = ((clip.y + clip.h) as i64).min(y1.ceil() as i64);
        if rx1 <= rx0 || ry1 <= ry0 {
            return None;
        }
        Some(Rect { x: rx0 as u32, y: ry0 as u32, w: (rx1 - rx0) as u32, h: (ry1 - ry0) as u32 })
    }

    /// An isolate's padded bounds in pixels within clip (None when empty).
    fn rect(&self, list: &DrawList, iso: &Isolate, clip: Rect) -> Option<Rect> {
        let b = bounds(list, iso)?;
        let (x0, x1) = ((b[0] - self.x0) * self.s, (b[2] - self.x0) * self.s);
        let (y0, y1) = ((self.y0 - b[3]) * self.s, (self.y0 - b[1]) * self.s);
        let rx0 = (clip.x as i64).max(x0.floor() as i64 - 2);
        let ry0 = (clip.y as i64).max(y0.floor() as i64 - 2);
        let rx1 = ((clip.x + clip.w) as i64).min(x1.ceil() as i64 + 2);
        let ry1 = ((clip.y + clip.h) as i64).min(y1.ceil() as i64 + 2);
        if rx1 <= rx0 || ry1 <= ry0 {
            return None;
        }
        Some(Rect { x: rx0 as u32, y: ry0 as u32, w: (rx1 - rx0) as u32, h: (ry1 - ry0) as u32 })
    }
}

/// Padded view bounds of an isolate (its effects' reach and its children's).
fn bounds(list: &DrawList, iso: &Isolate) -> Option<[f32; 4]> {
    let (mut x0, mut y0, mut x1, mut y1) = (iso.min_x, iso.min_y, iso.max_x, iso.max_y);
    for item in &iso.items {
        if let IsoItem::Isolate(k) = item {
            if let Some(b) = bounds(list, &list.isolates[*k]) {
                x0 = x0.min(b[0]);
                y0 = y0.min(b[1]);
                x1 = x1.max(b[2]);
                y1 = y1.max(b[3]);
            }
        }
    }
    if !(x0 <= x1 && y0 <= y1) {
        return None;
    }
    let mut pad = 0f32;
    for e in &iso.effects {
        pad += rfx::padding(e);
        if e.ty == rfx::PIXELATE {
            // blocks centred inside the content reach past it (not in the reference's padding)
            pad += rfx::f(e, "size", 8.0, 1.0, rfx::MAX_LENGTH) + 1.0;
        }
    }
    Some([x0 - pad, y0 - pad, x1 + pad, y1 + pad])
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


/// K2: an effect plugin's whole WGSL: the uniform block and textures the
/// built-in effects use, `Props` (size, time, then each setting by name),
/// `layer(uv)` (the layer's colour, premultiplied, uv 0..1 from the top
/// left, smooth between pixels), the plugin's own code with its
/// `fn effect(uv: vec2<f32>, u: Props) -> vec4<f32>`, and the entry points.
fn plugin_wgsl(def: &barkfart_script::Definition) -> String {
    use barkfart_script::PropKind as K;
    let mut fields = String::from("    size: vec2<f32>,\n    time: f32,\n");
    let mut init = String::from("vec2<f32>(p.v[0].x, p.v[0].y), p.v[0].z");
    for (i, p) in def.props.iter().enumerate() {
        let slot = i + 1;
        let (ty, val) = match &p.kind {
            K::Colour { .. } => ("vec4<f32>", format!("p.v[{slot}]")),
            K::Point { .. } => ("vec2<f32>", format!("p.v[{slot}].xy")),
            _ => ("f32", format!("p.v[{slot}].x")),
        };
        fields.push_str(&format!("    {}: {ty},\n", p.name));
        init.push_str(&format!(", {val}"));
    }
    let src = def.shader.as_ref().map(|s| s.source.as_str()).unwrap_or("");
    format!(
        r"struct P {{ v: array<vec4<f32>, 40> }};
@group(0) @binding(0) var<uniform> p: P;
@group(0) @binding(1) var t0: texture_2d<f32>;
@group(0) @binding(2) var t1: texture_2d<f32>;

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {{
    let x = f32((i << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(i & 2u) * 2.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}}

struct Props {{
{fields}}};

fn bf_texel(q: vec2<i32>) -> vec4<f32> {{
    let d = vec2<i32>(textureDimensions(t0));
    if (q.x < 0 || q.y < 0 || q.x >= d.x || q.y >= d.y) {{ return vec4<f32>(0.0); }}
    return textureLoad(t0, q, 0);
}}

fn layer(uv: vec2<f32>) -> vec4<f32> {{
    let x = uv * vec2<f32>(textureDimensions(t0)) - 0.5;
    let b = floor(x);
    let f = x - b;
    let q = vec2<i32>(b);
    let top = mix(bf_texel(q), bf_texel(q + vec2<i32>(1, 0)), f.x);
    let bottom = mix(bf_texel(q + vec2<i32>(0, 1)), bf_texel(q + vec2<i32>(1, 1)), f.x);
    return mix(top, bottom, f.y);
}}

{src}

@fragment
fn fs_plugin(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {{
    let u = Props({init});
    return effect(pos.xy / u.size, u);
}}
"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use barkfart_core::eval::{Group, Item, Scene, Shape, ShapeKind};
    use barkfart_core::tess;

    fn rect_shape(x0: f32, y0: f32, x1: f32, y1: f32, color: Color) -> Item {
        Item::Shape(Shape {
            kind: ShapeKind::Polygon { points: vec![x0, y0, x1, y0, x1, y1, x0, y1], convex: true },
            color,
            gradient: None,
            source: "r".into(),
        })
    }

    fn compare(gpu: &[u8], cpu: &[u8], label: &str) {
        assert_eq!(gpu.len(), cpu.len());
        let n = gpu.len() / 4;
        let (mut sum, mut bad, mut covered) = (0u64, 0usize, 0usize);
        for i in 0..n {
            if gpu[i * 4 + 3] == 0 && cpu[i * 4 + 3] == 0 {
                continue;
            }
            covered += 1;
            let mut worst = 0u8;
            for c in 0..4 {
                let d = gpu[i * 4 + c].abs_diff(cpu[i * 4 + c]);
                sum += d as u64;
                worst = worst.max(d);
            }
            if worst > 12 {
                bad += 1;
            }
        }
        assert!(covered > 0, "{label}: nothing rendered");
        let mean = sum as f64 / (4.0 * covered as f64);
        let bad_pct = bad as f64 / covered as f64;
        assert!(mean < 0.8 && bad_pct < 0.008, "{label}: mean={mean:.3}, bad={:.3}%", bad_pct * 100.0);
    }

    fn dl_with_effects(effects: Vec<ResolvedEffect>, adjustment: bool, adj_bounds: Option<[f32; 4]>, opacity: f32) -> DrawList {
        let s1 = rect_shape(-30.0, -30.0, 30.0, 30.0, Color { r: 0.2, g: 0.67, b: 0.93, a: 1.0 });
        let s2 = rect_shape(-12.0, -18.0, 18.0, 22.0, Color { r: 1.0, g: 0.4, b: 0.2, a: 1.0 });
        let root = if adjustment {
            Group {
                source: "root".into(),
                opacity: 1.0,
                blend: Blend::Normal,
                adjustment: false,
                bounds: None,
                effects: vec![],
                children: vec![
                    s1,
                    s2,
                    Item::Group(Group {
                        source: "adj".into(),
                        opacity,
                        blend: Blend::Normal,
                        adjustment: true,
                        bounds: adj_bounds,
                        effects,
                        children: vec![],
                    }),
                ],
            }
        } else {
            Group {
                source: "root".into(),
                opacity: 1.0,
                blend: Blend::Normal,
                adjustment: false,
                bounds: None,
                effects: vec![],
                children: vec![Item::Group(Group {
                    source: "fig".into(),
                    opacity,
                    blend: Blend::Normal,
                    adjustment: false,
                    bounds: None,
                    effects,
                    children: vec![s1, s2],
                })],
            }
        };
        let scene = Scene { root, diagnostics: vec![], primitives: 2, time: 0.0 };
        tess::layered(&scene, None)
    }

    #[test]
    fn new_effects_and_stacked_adjustment_match_cpu_reference() {
        let Ok(r) = Renderer::headless_with(&[wgpu::Backends::PRIMARY, wgpu::Backends::GL], Options { samples: 1 }) else {
            return;
        };
        let (w, h, scale) = (128usize, 128usize, 1.0f32);
        let (x0, y0) = (-64.0f32, 64.0f32);
        let view = View { x0, y0: y0 - h as f32 / scale, x1: x0 + w as f32 / scale, y1: y0, background: [0.0; 4] };

        let cases: Vec<(&str, Vec<ResolvedEffect>, bool, Option<[f32; 4]>, f32)> = vec![
            ("chromatic", vec![ResolvedEffect { ty: rfx::CHROMATIC.into(), mix: 1.0, params: vec![("amount".into(), ParamValue::F(7.0)), ("angle".into(), ParamValue::F(20.0))] }], false, None, 1.0),
            ("vignette_layer", vec![ResolvedEffect { ty: rfx::VIGNETTE.into(), mix: 1.0, params: vec![("amount".into(), ParamValue::F(0.7)), ("softness".into(), ParamValue::F(0.5))] }], false, None, 1.0),
            ("vignette_adj", vec![ResolvedEffect { ty: rfx::VIGNETTE.into(), mix: 1.0, params: vec![("amount".into(), ParamValue::F(0.7)), ("softness".into(), ParamValue::F(0.55)), ("color".into(), ParamValue::C(Color { r: 0.05, g: 0.0, b: 0.1, a: 1.0 }))] }], true, None, 0.85),
            ("wave", vec![ResolvedEffect { ty: rfx::WAVE.into(), mix: 1.0, params: vec![("amplitude".into(), ParamValue::F(8.0)), ("frequency".into(), ParamValue::F(5.0)), ("phase".into(), ParamValue::F(45.0)), ("angle".into(), ParamValue::F(15.0))] }], false, None, 1.0),
            ("grain_layer", vec![ResolvedEffect { ty: rfx::GRAIN.into(), mix: 1.0, params: vec![("amount".into(), ParamValue::F(0.35)), ("size".into(), ParamValue::F(2.0)), ("seed".into(), ParamValue::F(5.0))] }], false, None, 1.0),
            ("grain_adj", vec![ResolvedEffect { ty: rfx::GRAIN.into(), mix: 1.0, params: vec![("amount".into(), ParamValue::F(0.35)), ("size".into(), ParamValue::F(3.0)), ("seed".into(), ParamValue::F(9.0))] }], true, None, 1.0),
            ("grade", vec![ResolvedEffect { ty: rfx::GRADE.into(), mix: 1.0, params: vec![("exposure".into(), ParamValue::F(0.4)), ("contrast".into(), ParamValue::F(1.35)), ("gamma".into(), ParamValue::F(0.85))] }], false, None, 1.0),
            ("scanlines_adj", vec![ResolvedEffect { ty: rfx::SCANLINES.into(), mix: 1.0, params: vec![("amount".into(), ParamValue::F(0.55)), ("lines".into(), ParamValue::F(40.0)), ("offset".into(), ParamValue::F(5.0)), ("phase".into(), ParamValue::F(3.0))] }], true, None, 1.0),
            // Stacked multi-pass adjustment layer with partial mix and opacity + camera-shot clip!
            ("stacked_adj_clipped", vec![
                ResolvedEffect { ty: rfx::BLUR.into(), mix: 0.6, params: vec![("radius".into(), ParamValue::F(6.0))] },
                ResolvedEffect { ty: rfx::CHROMATIC.into(), mix: 0.8, params: vec![("amount".into(), ParamValue::F(5.0)), ("angle".into(), ParamValue::F(0.0))] },
                ResolvedEffect { ty: rfx::GRADE.into(), mix: 1.0, params: vec![("exposure".into(), ParamValue::F(0.2)), ("contrast".into(), ParamValue::F(1.3)), ("gamma".into(), ParamValue::F(1.0))] },
                ResolvedEffect { ty: rfx::VIGNETTE.into(), mix: 0.9, params: vec![("amount".into(), ParamValue::F(0.6)), ("softness".into(), ParamValue::F(0.5))] },
            ], true, Some([-44.0, -40.0, 44.0, 40.0]), 0.75),
        ];

        for (name, effects, adj, bounds, opacity) in cases {
            let dl = dl_with_effects(effects, adj, bounds, opacity);
            let mut im = rfx::Image::new(w, h, x0, y0, scale);
            rfx::render(&dl, &mut im);
            let cpu = im.to_premultiplied_u8();
            let gpu = r.render_raw(&dl, w as u32, h as u32, view);
            compare(&gpu, &cpu, name);
        }
    }

    #[test]
    fn stock_wgsl_effect_plugins_compile_cleanly() {
        let Ok(r) = Renderer::headless_with(&[wgpu::Backends::PRIMARY, wgpu::Backends::GL], Options { samples: 1 }) else {
            return;
        };
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/barkfart/plugins");
        let mut checked = 0;
        for entry in std::fs::read_dir(&dir).expect("plugins dir") {
            let path = entry.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("luau") {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            let def = barkfart_core::plugins::install(&src, path.file_name().unwrap().to_str().unwrap()).unwrap();
            if def.kind == barkfart_script::Kind::Effect {
                assert!(r.plugin_pipeline(&def.id).is_some(), "stock shader failed to compile: {}", def.id);
                checked += 1;
            }
            barkfart_core::plugins::uninstall(&def.id);
        }
        assert!(checked >= 3, "expected at least 3 stock WGSL effect plugins, checked {checked}");
    }
}
