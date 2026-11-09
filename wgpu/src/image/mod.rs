pub(crate) mod cache;
pub(crate) use cache::Cache;

mod atlas;

#[cfg(feature = "image")]
mod raster;

#[cfg(feature = "svg")]
mod vector;

use crate::Buffer;
use crate::core::border;
use crate::core::{Rectangle, Size, Transformation};
use crate::graphics::Shell;
use crate::nudge;

use bytemuck::{Pod, Zeroable};

use std::mem;
use std::sync::Arc;

pub use crate::graphics::Image;

pub type Batch = Vec<Image>;

#[derive(Debug, Clone)]
pub struct Pipeline {
    raw: wgpu::RenderPipeline,
    backend: wgpu::Backend,
    nearest_sampler: wgpu::Sampler,
    linear_sampler: wgpu::Sampler,
    texture_layout: wgpu::BindGroupLayout,
    constant_layout: wgpu::BindGroupLayout,
}

impl Pipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, backend: wgpu::Backend) -> Self {
        let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            min_filter: wgpu::FilterMode::Nearest,
            mag_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        let constant_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("iced_wgpu::image constants layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(mem::size_of::<Uniforms>() as u64),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("iced_wgpu::image texture atlas layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            }],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("iced_wgpu::image pipeline layout"),
            bind_group_layouts: &[Some(&constant_layout), Some(&texture_layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("iced_wgpu image shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(concat!(
                include_str!("../shader/vertex.wgsl"),
                "\n",
                include_str!("../shader/color.wgsl"),
                "\n",
                include_str!("../shader/image.wgsl"),
            ))),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("iced_wgpu::image pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array!(
                        // Center
                        0 => Float32x2,
                        // Clip bounds
                        1 => Float32x4,
                        // Border radius
                        2 => Float32x4,
                        // Tile
                        3 => Float32x4,
                        // Rotation
                        4 => Float32,
                        // Opacity
                        5 => Float32,
                        // Atlas position
                        6 => Float32x2,
                        // Atlas scale
                        7 => Float32x2,
                        // Layer
                        8 => Sint32,
                    ),
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Cw,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        Pipeline {
            raw: pipeline,
            backend,
            nearest_sampler,
            linear_sampler,
            texture_layout,
            constant_layout,
        }
    }

    pub(crate) fn create_cache(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shell: &Shell,
        health: &std::sync::Arc<crate::window::compositor::DeviceHealth>,
    ) -> Cache {
        Cache::new(
            device,
            queue,
            self.backend,
            self.texture_layout.clone(),
            shell,
            health,
        )
    }
}

#[derive(Default)]
pub struct State {
    layers: Vec<Layer>,
    prepare_layer: usize,
    instances: Vec<Instance>,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn prepare(
        &mut self,
        pipeline: &Pipeline,
        device: &wgpu::Device,
        belt: &mut wgpu::util::StagingBelt,
        encoder: &mut wgpu::CommandEncoder,
        cache: &mut Cache,
        images: &Batch,
        transformation: Transformation,
        scale: f32,
    ) {
        if self.layers.len() <= self.prepare_layer {
            self.layers.push(Layer::new(
                device,
                &pipeline.constant_layout,
                &pipeline.nearest_sampler,
                &pipeline.linear_sampler,
            ));
        }

        let layer = &mut self.layers[self.prepare_layer];

        // The group being filled: its atlas and whether it samples linearly.
        // Images keep their submission order, so a new group starts whenever
        // either changes (#3512: nearest and linear images used to be drawn
        // in two passes, which reordered them).
        let mut group: Option<(Arc<wgpu::BindGroup>, bool)> = None;

        for image in images {
            match &image {
                #[cfg(feature = "image")]
                Image::Raster {
                    image,
                    bounds,
                    clip_bounds,
                } => {
                    // `bounds` and `clip_bounds` can have negative coordinates,
                    // as content can be scrolled out of view; they are clipped
                    // by the pass's scissor afterwards.
                    let bounds = nudge::round(*bounds * scale);
                    let clip_bounds = nudge::round(*clip_bounds * scale);

                    if bounds.width < 1.0 || bounds.height < 1.0 {
                        continue;
                    }

                    if let Some((atlas_entry, bind_group)) =
                        cache.upload_raster(device, encoder, belt, &image.handle)
                    {
                        let linear = match image.filter_method {
                            crate::core::image::FilterMethod::Nearest => false,
                            crate::core::image::FilterMethod::Linear => true,
                        };

                        switch_group(layer, &mut group, bind_group, linear, &self.instances);

                        add_instances(
                            bounds,
                            clip_bounds,
                            image.border_radius * scale,
                            f32::from(image.rotation),
                            image.opacity,
                            atlas_entry,
                            &mut self.instances,
                        );
                    }
                }
                #[cfg(not(feature = "image"))]
                Image::Raster { .. } => continue,

                #[cfg(feature = "svg")]
                Image::Vector {
                    svg,
                    bounds,
                    clip_bounds,
                } => {
                    // `bounds` and `clip_bounds` can have negative coordinates,
                    // as content can be scrolled out of view; they are clipped
                    // by the pass's scissor afterwards.
                    let bounds = nudge::round(*bounds * scale);
                    let clip_bounds = nudge::round(*clip_bounds * scale);

                    if bounds.width < 1.0 || bounds.height < 1.0 {
                        continue;
                    }

                    if let Some((atlas_entry, bind_group)) = cache.upload_vector(
                        device,
                        encoder,
                        belt,
                        &svg.handle,
                        svg.color,
                        svg.current_color,
                        Size::new(bounds.width as u32, bounds.height as u32),
                    ) {
                        // Vectors are rasterized at their final size, so
                        // they are sampled with the nearest filter.
                        switch_group(layer, &mut group, bind_group, false, &self.instances);

                        add_instances(
                            bounds,
                            clip_bounds,
                            border::radius(0),
                            f32::from(svg.rotation),
                            svg.opacity,
                            atlas_entry,
                            &mut self.instances,
                        );
                    }
                }
                #[cfg(not(feature = "svg"))]
                Image::Vector { .. } => continue,
            }
        }

        if let Some((atlas, linear)) = &group {
            layer.push(atlas, *linear, &self.instances);
        }

        layer.prepare(device, encoder, belt, transformation, &self.instances);

        self.prepare_layer += 1;
        self.instances.clear();
    }

    pub fn render<'a>(
        &'a self,
        pipeline: &'a Pipeline,
        layer: usize,
        bounds: Rectangle<u32>,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        if let Some(layer) = self.layers.get(layer) {
            render_pass.set_pipeline(&pipeline.raw);

            render_pass.set_scissor_rect(bounds.x, bounds.y, bounds.width, bounds.height);

            layer.render(render_pass);
        }
    }

    pub fn trim(&mut self) {
        for layer in &mut self.layers[..self.prepare_layer] {
            layer.clear();
        }

        self.prepare_layer = 0;
    }
}

#[derive(Debug)]
struct Layer {
    uniforms: wgpu::Buffer,
    instances: Buffer<Instance>,
    groups: Vec<Group>,
    total: usize,
    nearest_layout: wgpu::BindGroup,
    linear_layout: wgpu::BindGroup,
}

#[derive(Debug)]
struct Group {
    atlas: Arc<wgpu::BindGroup>,
    linear: bool,
    instance_count: usize,
}

impl Layer {
    fn new(
        device: &wgpu::Device,
        constant_layout: &wgpu::BindGroupLayout,
        nearest_sampler: &wgpu::Sampler,
        linear_sampler: &wgpu::Sampler,
    ) -> Self {
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("iced_wgpu::image uniforms buffer"),
            size: mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instances = Buffer::new(
            device,
            "iced_wgpu::image instance buffer",
            Instance::INITIAL,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );

        let nearest_layout = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("iced_wgpu::image constants bind group"),
            layout: constant_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniforms,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(nearest_sampler),
                },
            ],
        });

        let linear_layout = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("iced_wgpu::image constants bind group"),
            layout: constant_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniforms,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(linear_sampler),
                },
            ],
        });

        Self {
            uniforms,
            instances,
            groups: Vec::new(),
            total: 0,
            nearest_layout,
            linear_layout,
        }
    }

    fn prepare(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut wgpu::util::StagingBelt,
        transformation: Transformation,
        instances: &[Instance],
    ) {
        let uniforms = Uniforms {
            transform: transformation.into(),
        };

        let bytes = bytemuck::bytes_of(&uniforms);

        belt.write_buffer(
            encoder,
            &self.uniforms,
            0,
            (bytes.len() as u64).try_into().expect("Sized uniforms"),
        )
        .copy_from_slice(bytes);

        let _ = self.instances.resize(device, self.total);

        if !instances.is_empty() {
            let _ = self.instances.write(encoder, belt, 0, instances);
        }
    }

    /// Closes the group of the instances added since the last push.
    fn push(&mut self, atlas: &Arc<wgpu::BindGroup>, linear: bool, instances: &[Instance]) {
        let new = instances.len() - self.total;

        if new > 0 {
            self.groups.push(Group {
                atlas: atlas.clone(),
                linear,
                instance_count: new,
            });

            self.total = instances.len();
        }
    }

    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.instances.slice(..));

        let mut offset = 0;
        let mut sampler = None;

        for group in &self.groups {
            if sampler != Some(group.linear) {
                render_pass.set_bind_group(
                    0,
                    if group.linear {
                        &self.linear_layout
                    } else {
                        &self.nearest_layout
                    },
                    &[],
                );

                sampler = Some(group.linear);
            }

            render_pass.set_bind_group(1, group.atlas.as_ref(), &[]);
            render_pass.draw(0..6, offset..offset + group.instance_count as u32);

            offset += group.instance_count as u32;
        }
    }

    fn clear(&mut self) {
        self.groups.clear();
        self.total = 0;
    }
}

/// Starts a new group when the atlas or the filter changes, closing the
/// current one in `layer`.
fn switch_group(
    layer: &mut Layer,
    group: &mut Option<(Arc<wgpu::BindGroup>, bool)>,
    atlas: &Arc<wgpu::BindGroup>,
    linear: bool,
    instances: &[Instance],
) {
    if let Some((current, current_linear)) = group.as_ref() {
        if current == atlas && *current_linear == linear {
            return;
        }

        layer.push(current, *current_linear, instances);
    }

    *group = Some((atlas.clone(), linear));
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
struct Instance {
    _center: [f32; 2],
    _clip_bounds: [f32; 4],
    _border_radius: [f32; 4],
    _tile: [f32; 4],
    _rotation: f32,
    _opacity: f32,
    _position_in_atlas: [f32; 2],
    _size_in_atlas: [f32; 2],
    _layer: u32,
}

impl Instance {
    pub const INITIAL: usize = 20;
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
struct Uniforms {
    transform: [f32; 16],
}

fn add_instances(
    bounds: Rectangle,
    clip_bounds: Rectangle,
    border_radius: border::Radius,
    rotation: f32,
    opacity: f32,
    entry: &atlas::Entry,
    instances: &mut Vec<Instance>,
) {
    let center = [
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
    ];

    let clip_bounds = [
        clip_bounds.x,
        clip_bounds.y,
        clip_bounds.width,
        clip_bounds.height,
    ];

    let border_radius = border_radius.into();

    match entry {
        atlas::Entry::Contiguous(allocation) => {
            add_instance(
                center,
                clip_bounds,
                border_radius,
                [bounds.x, bounds.y, bounds.width, bounds.height],
                rotation,
                opacity,
                allocation,
                instances,
            );
        }
        atlas::Entry::Fragmented { fragments, size } => {
            let scaling_x = bounds.width / size.width as f32;
            let scaling_y = bounds.height / size.height as f32;

            for fragment in fragments {
                let allocation = &fragment.allocation;
                let (fragment_x, fragment_y) = fragment.position;

                let Size {
                    width: fragment_width,
                    height: fragment_height,
                } = allocation.size();

                let tile = [
                    bounds.x + fragment_x as f32 * scaling_x,
                    bounds.y + fragment_y as f32 * scaling_y,
                    fragment_width as f32 * scaling_x,
                    fragment_height as f32 * scaling_y,
                ];

                add_instance(
                    center,
                    clip_bounds,
                    border_radius,
                    tile,
                    rotation,
                    opacity,
                    allocation,
                    instances,
                );
            }
        }
    }
}

#[inline]
fn add_instance(
    center: [f32; 2],
    clip_bounds: [f32; 4],
    border_radius: [f32; 4],
    tile: [f32; 4],
    rotation: f32,
    opacity: f32,
    allocation: &atlas::Allocation,
    instances: &mut Vec<Instance>,
) {
    let (x, y) = allocation.position();
    let Size { width, height } = allocation.size();
    let layer = allocation.layer();
    let atlas_size = allocation.atlas_size();

    let instance = Instance {
        _center: center,
        _clip_bounds: clip_bounds,
        _border_radius: border_radius,
        _tile: tile,
        _rotation: rotation,
        _opacity: opacity,
        _position_in_atlas: [x as f32 / atlas_size as f32, y as f32 / atlas_size as f32],
        _size_in_atlas: [
            width as f32 / atlas_size as f32,
            height as f32 / atlas_size as f32,
        ],
        _layer: layer as u32,
    };

    instances.push(instance);
}

#[cfg(all(test, feature = "image"))]
mod tests {
    use crate::core::image::{self, FilterMethod, Renderer as _};
    use crate::core::renderer::{self, Quad, Renderer as _};
    use crate::core::{Color, Point, Rectangle, Size};

    const RED: [u8; 4] = [255, 0, 0, 255];
    const GREEN: [u8; 4] = [0, 255, 0, 255];

    fn solid(color: [u8; 4]) -> image::Handle {
        image::Handle::from_rgba(2, 2, color.repeat(4))
    }

    /// Draws with `draw` on a fresh headless renderer and returns the RGBA
    /// pixel getter of the 100x100 result, or `None` without a GPU adapter.
    fn render(
        draw: impl FnOnce(&mut crate::Renderer, Rectangle),
    ) -> Option<impl Fn(usize, usize) -> [u8; 4]> {
        let Some(mut renderer) =
            futures::executor::block_on(<crate::Renderer as crate::renderer::Headless>::new(
                renderer::Settings::default(),
                None,
            ))
        else {
            assert!(
                std::env::var_os("ICED_REQUIRE_GPU_TESTS").is_none(),
                "no GPU adapter available, but ICED_REQUIRE_GPU_TESTS is set"
            );
            eprintln!("SKIPPED: no GPU adapter available");
            return None;
        };

        let bounds = Rectangle::with_size(Size::new(100.0, 100.0));

        // Layers with the same bounds as the base one can be merged into it.
        renderer.reset(bounds);
        draw(&mut renderer, bounds);

        let pixels = renderer.screenshot(
            &crate::graphics::Viewport::with_physical_size(
                Size::new(100, 100),
                renderer::Scale {
                    window: 1.0,
                    application: 1.0,
                },
            ),
            Color::TRANSPARENT,
        );

        Some(move |x: usize, y: usize| {
            let i = (y * 100 + x) * 4;
            [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
        })
    }

    fn icon() -> Rectangle {
        Rectangle::new(Point::new(25.0, 25.0), Size::new(50.0, 50.0))
    }

    /// Upstream #3512: an image drawn in a layer created after a linear
    /// image used to end up under it once the two layers were merged,
    /// because nearest images were all drawn before linear ones.
    #[test]
    fn a_nearest_image_in_a_later_layer_is_drawn_over_a_linear_one() {
        let Some(pixel) = render(|renderer, bounds| {
            renderer.fill_quad(
                Quad {
                    bounds,
                    ..Quad::default()
                },
                Color::BLACK,
            );
            renderer.draw_image(
                image::Image::new(solid(RED)).filter_method(FilterMethod::Linear),
                bounds,
                bounds,
            );
            renderer.with_layer(bounds, |renderer| {
                renderer.draw_image(
                    image::Image::new(solid(GREEN)).filter_method(FilterMethod::Nearest),
                    icon(),
                    icon(),
                );
            });
        }) else {
            return;
        };

        assert_eq!(pixel(50, 50), GREEN, "the later image is on top");
        assert_eq!(pixel(5, 5), RED, "the linear image is still drawn");
    }

    /// The same order within one layer, without relying on layer merging.
    #[test]
    fn images_in_one_layer_keep_submission_order_across_filters() {
        let Some(pixel) = render(|renderer, bounds| {
            renderer.draw_image(
                image::Image::new(solid(RED)).filter_method(FilterMethod::Linear),
                bounds,
                bounds,
            );
            renderer.draw_image(
                image::Image::new(solid(GREEN)).filter_method(FilterMethod::Nearest),
                icon(),
                icon(),
            );
        }) else {
            return;
        };

        assert_eq!(pixel(50, 50), GREEN, "the later image is on top");
        assert_eq!(pixel(5, 5), RED, "the earlier image is still drawn");
    }
}
