/// A resource for graphics state
#[derive(Default)]
pub struct Graphics {
    /// The renderer context
    pub renderer: Option<Renderer>,

    /// The renderer context
    #[cfg(target_arch = "wasm32")]
    pub renderer_receiver: Option<futures::channel::oneshot::Receiver<crate::graphics::Renderer>>,

    /// The size of the display viewport
    pub viewport_size: (u32, u32),
}

/// Contains all resources required for rendering
pub struct Renderer {
    pub gpu: Gpu,
    pub depth_texture_view: wgpu::TextureView,
    pub egui_renderer: egui_wgpu::Renderer,
    pub grid: Grid,
    pub triangle: TriangleRender,
}

pub use triangle::*;
pub mod triangle {
    /// Common vertex format for all triangle mesh rendering
    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Vertex {
        pub position: [f32; 4],
        pub color: [f32; 4],
    }

    pub struct TriangleRender {
        pub model: nalgebra_glm::Mat4,
        pub vertex_buffer: wgpu::Buffer,
        pub index_buffer: wgpu::Buffer,
        pub buffer: wgpu::Buffer,
        pub bind_group: wgpu::BindGroup,
        pub pipeline: wgpu::RenderPipeline,
    }
    #[repr(C)]
    #[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct UniformBuffer {
        pub mvp: nalgebra_glm::Mat4,
    }
    pub const TRIANGLE_VERTICES: [crate::graphics::Vertex; 3] = [
        crate::graphics::Vertex {
            position: [1.0, -1.0, 0.0, 1.0],
            color: [1.0, 0.0, 0.0, 1.0],
        },
        crate::graphics::Vertex {
            position: [-1.0, -1.0, 0.0, 1.0],
            color: [0.0, 1.0, 0.0, 1.0],
        },
        crate::graphics::Vertex {
            position: [0.0, 1.0, 0.0, 1.0],
            color: [0.0, 0.0, 1.0, 1.0],
        },
    ];
    pub const TRIANGLE_INDICES: [u32; 3] = [0, 1, 2]; // Clockwise winding order

    pub fn create_triangle(
        device: &wgpu::Device,
        depth_format: wgpu::TextureFormat,
        surface_format: wgpu::TextureFormat,
    ) -> crate::graphics::triangle::TriangleRender {
        let vertex_buffer = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&crate::graphics::triangle::TRIANGLE_VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            },
        );
        let index_buffer = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("index Buffer"),
                contents: bytemuck::cast_slice(&crate::graphics::triangle::TRIANGLE_INDICES),
                usage: wgpu::BufferUsages::INDEX,
            },
        );
        let buffer = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("Uniform Buffer"),
                contents: bytemuck::cast_slice(&[
                    crate::graphics::triangle::UniformBuffer::default(),
                ]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            },
        );
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            label: Some("uniform_bind_group_layout"),
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/triangle.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let attributes: &[wgpu::VertexAttribute] =
            &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                buffers: &[{
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<crate::graphics::Vertex>()
                            as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes,
                    }
                }],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: Some(wgpu::IndexFormat::Uint32),
                front_face: wgpu::FrontFace::Cw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
                unclipped_depth: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        });
        crate::graphics::triangle::TriangleRender {
            model: nalgebra_glm::Mat4::identity(),
            pipeline,
            vertex_buffer,
            index_buffer,
            buffer,
            bind_group,
        }
    }

    pub fn render_triangle(
        triangle: &mut crate::graphics::triangle::TriangleRender,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) {
        render_pass.set_pipeline(&triangle.pipeline);
        render_pass.set_bind_group(0, &triangle.bind_group, &[]);
        render_pass.set_vertex_buffer(0, triangle.vertex_buffer.slice(..));
        render_pass.set_index_buffer(triangle.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(
            0..(crate::graphics::triangle::TRIANGLE_INDICES.len() as _),
            0,
            0..1,
        );
    }

    pub fn update_triangle(context: &mut crate::scene::Context) {
        let delta_time = context.resources.frame_timing.delta_time;
        use crate::scene::queries::*;
        let Some(camera_entity) = query_first_camera(context) else {
            return;
        };
        let Some(CameraMatrices {
            projection, view, ..
        }) = query_camera_matrices(context, camera_entity)
        else {
            return;
        };
        let Some(renderer) = context.resources.graphics.renderer.as_mut() else {
            return;
        };
        renderer.triangle.model = nalgebra_glm::rotate(
            &renderer.triangle.model,
            30_f32.to_radians() * delta_time,
            &nalgebra_glm::Vec3::y(),
        );
        renderer.gpu.queue.write_buffer(
            &renderer.triangle.buffer,
            0,
            bytemuck::cast_slice(&[crate::graphics::triangle::UniformBuffer {
                mvp: projection * view * renderer.triangle.model,
            }]),
        );
    }
}

/// Low-level wgpu handles
pub struct Gpu {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub surface_format: wgpu::TextureFormat,
}

pub use grid::*;
pub mod grid {
    use super::*;

    pub struct Grid {
        pub uniform_buffer: wgpu::Buffer,
        pub bind_group: wgpu::BindGroup,
        pub pipeline: wgpu::RenderPipeline,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    struct GridUniform {
        view_proj: nalgebra_glm::Mat4,
        camera_world_pos: nalgebra_glm::Vec3,
        grid_size: f32,
        grid_min_pixels: f32,
        grid_cell_size: f32,
        _padding: [f32; 2],
    }

    pub fn create_grid(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Grid {
        use wgpu::util::DeviceExt;

        let grid_uniform = GridUniform {
            view_proj: nalgebra_glm::Mat4::identity(),
            camera_world_pos: nalgebra_glm::Vec3::zeros(),
            grid_size: 100.0,
            grid_min_pixels: 2.0,
            grid_cell_size: 0.025,
            _padding: [0.0; 2],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Grid Uniform Buffer"),
            contents: bytemuck::cast_slice(&[grid_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Grid Layout"),
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("Grid Bind Group"),
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/grid.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Grid Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Grid Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Grid {
            uniform_buffer,
            bind_group,
            pipeline,
        }
    }

    pub fn update_grid_uniform(context: &mut crate::scene::Context) {
        use crate::scene::queries::*;
        let Some(camera_entity) = query_first_camera(context) else {
            return;
        };
        let Some(matrices) = query_camera_matrices(context, camera_entity) else {
            return;
        };
        let Some(renderer) = context.resources.graphics.renderer.as_mut() else {
            return;
        };
        let uniform = GridUniform {
            view_proj: matrices.projection * matrices.view,
            camera_world_pos: matrices.camera_position.xyz(),
            grid_size: 100.0,
            grid_min_pixels: 2.0,
            grid_cell_size: 0.025,
            _padding: [0.0; 2],
        };
        renderer.gpu.queue.write_buffer(
            &renderer.grid.uniform_buffer,
            0,
            bytemuck::cast_slice(&[uniform]),
        );
    }

    pub fn render_grid(render_pass: &mut wgpu::RenderPass<'_>, renderer: &mut Renderer) {
        render_pass.set_pipeline(&renderer.grid.pipeline);
        render_pass.set_bind_group(0, &renderer.grid.bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}

pub mod systems {
    use super::*;

    /// Receives the renderer from the async task that creates it on wasm, injecting it as a resource
    #[cfg(target_arch = "wasm32")]
    pub fn receive_renderer(context: &mut crate::scene::Context) {
        if let Some(receiver) = context.resources.graphics.renderer_receiver.as_mut() {
            if let Ok(Some(renderer)) = receiver.try_recv() {
                context.resources.graphics.renderer = Some(renderer);
                context.resources.graphics.renderer_receiver = None;
            }
        }
        if context.resources.graphics.renderer.is_none() {
            return;
        }
    }

    /// This system renders and presents the next frame
    pub fn render_frame(context: &mut crate::scene::Context) {
        update_render_buffers(context);

        let Some((egui::FullOutput { textures_delta, .. }, paint_jobs)) =
            context.resources.user_interface.frame_output.take()
        else {
            return;
        };
        let Some(window_handle) = context.resources.window.handle.as_ref() else {
            return;
        };
        let screen_descriptor = {
            let (width, height) = context.resources.graphics.viewport_size;
            egui_wgpu::ScreenDescriptor {
                size_in_pixels: [width, height],
                pixels_per_point: window_handle.scale_factor() as f32,
            }
        };

        let Some(renderer) = context.resources.graphics.renderer.as_mut() else {
            return;
        };

        for (id, image_delta) in &textures_delta.set {
            renderer.egui_renderer.update_texture(
                &renderer.gpu.device,
                &renderer.gpu.queue,
                *id,
                image_delta,
            );
        }

        for id in &textures_delta.free {
            renderer.egui_renderer.free_texture(id);
        }

        let mut encoder =
            renderer
                .gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        renderer.egui_renderer.update_buffers(
            &renderer.gpu.device,
            &renderer.gpu.queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        let surface_texture = renderer
            .gpu
            .surface
            .get_current_texture()
            .expect("Failed to get surface texture!");

        let surface_texture_view =
            surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor {
                    label: wgpu::Label::default(),
                    aspect: wgpu::TextureAspect::default(),
                    format: Some(renderer.gpu.surface_format),
                    dimension: None,
                    base_mip_level: 0,
                    mip_level_count: None,
                    base_array_layer: 0,
                    array_layer_count: None,
                });

        encoder.insert_debug_marker("Main Render Pass");

        // This scope around the crate::render_pass prevents the
        // crate::render_pass from holding a borrow to the encoder,
        // which would prevent calling `.finish()` in
        // preparation for queue submission.
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.19,
                            g: 0.24,
                            b: 0.42,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &renderer.depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_triangle(&mut renderer.triangle, &mut render_pass);
            render_grid(&mut render_pass, renderer);

            renderer.egui_renderer.render(
                &mut render_pass.forget_lifetime(),
                &paint_jobs,
                &screen_descriptor,
            );
        }

        renderer.gpu.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
    }

    fn update_render_buffers(context: &mut crate::scene::Context) {
        update_triangle(context);
        update_grid_uniform(context);
    }
}

pub async fn create_renderer_async(
    window: impl Into<wgpu::SurfaceTarget<'static>>,
    width: u32,
    height: u32,
) -> crate::graphics::Renderer {
    let depth_format = wgpu::TextureFormat::Depth32Float;
    let gpu = create_gpu_async(window, width, height).await;
    let depth_texture_view = create_depth_texture(&gpu.device, width, height);
    let egui_renderer = egui_wgpu::Renderer::new(
        &gpu.device,
        gpu.surface_config.format,
        Some(depth_format),
        1,
        false,
    );
    let grid = create_grid(&gpu.device, gpu.surface_config.format, depth_format);
    let triangle = crate::graphics::create_triangle(&gpu.device, depth_format, gpu.surface_format);
    crate::graphics::Renderer {
        gpu,
        depth_texture_view,
        egui_renderer,
        grid,
        triangle,
    }
}

/// This creates the low-level GPU resources needed for rendering
pub async fn create_gpu_async(
    window: impl Into<wgpu::SurfaceTarget<'static>>,
    width: u32,
    height: u32,
) -> crate::graphics::Gpu {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::util::backend_bits_from_env().unwrap_or_else(wgpu::Backends::all),
        ..Default::default()
    });

    let surface = instance.create_surface(window).unwrap();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .expect("Failed to request adapter!");
    let (device, queue) = {
        log::info!("WGPU Adapter Features: {:#?}", adapter.features());
        adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("WGPU Device"),
                    memory_hints: wgpu::MemoryHints::default(),
                    required_features: wgpu::Features::default(),
                    #[cfg(not(target_arch = "wasm32"))]
                    required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                    #[cfg(all(target_arch = "wasm32", feature = "webgpu"))]
                    required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                },
                None,
            )
            .await
            .expect("Failed to request a device!")
    };

    let surface_capabilities = surface.get_capabilities(&adapter);

    let surface_format = surface_capabilities
        .formats
        .iter()
        .copied()
        .find(|f| !f.is_srgb()) // egui wants a non-srgb surface texture
        .unwrap_or(surface_capabilities.formats[0]);

    let surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width,
        height,
        present_mode: surface_capabilities.present_modes[0],
        alpha_mode: surface_capabilities.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };

    surface.configure(&device, &surface_config);

    crate::graphics::Gpu {
        surface,
        device,
        queue,
        surface_config,
        surface_format,
    }
}

/// Create a depth texture for the renderer to use
pub fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let texture = device.create_texture(
        &(wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        }),
    );
    texture.create_view(&wgpu::TextureViewDescriptor {
        label: None,
        format: Some(wgpu::TextureFormat::Depth32Float),
        dimension: Some(wgpu::TextureViewDimension::D2),
        aspect: wgpu::TextureAspect::All,
        base_mip_level: 0,
        base_array_layer: 0,
        array_layer_count: None,
        mip_level_count: None,
    })
}
