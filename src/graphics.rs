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
    pub ui_depth_texture_view: wgpu::TextureView,
    pub ui: egui_wgpu::Renderer,
    pub targets: Vec<RenderTarget>,
}

pub struct RenderTarget {
    pub color_texture: wgpu::Texture,
    pub color_texture_view: wgpu::TextureView,
    pub depth_texture: wgpu::Texture,
    pub depth_texture_view: wgpu::TextureView,
    pub grid: Grid,
    pub sky: Sky,
    pub lines: Lines,
    pub quads: Quads,
}

/// Low-level wgpu handles
pub struct Gpu {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_config: wgpu::SurfaceConfiguration,
}

/// Receives the renderer from the async task that creates it on wasm, injecting it as a resource
#[cfg(target_arch = "wasm32")]
pub fn receive_renderer_system(context: &mut crate::context::Context) {
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

pub fn resize_renderer(context: &mut crate::context::Context, width: u32, height: u32) {
    let Some(renderer) = context.resources.graphics.renderer.as_mut() else {
        return;
    };

    // Update surface config
    renderer.gpu.surface_config.width = width;
    renderer.gpu.surface_config.height = height;
    renderer
        .gpu
        .surface
        .configure(&renderer.gpu.device, &renderer.gpu.surface_config);

    let ui_depth_view = {
        let device: &wgpu::Device = &renderer.gpu.device;
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
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
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
    };
    renderer.ui_depth_texture_view = ui_depth_view;

    let mut targets = Vec::new();
    (0..renderer.targets.len()).for_each(|_| {
        let color_texture = renderer
            .gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Viewport Texture"),
                size: wgpu::Extent3d {
                    width: renderer.gpu.surface_config.width,
                    height: renderer.gpu.surface_config.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: renderer.gpu.surface_config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[renderer.gpu.surface_config.format],
            });
        let color_texture_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let device: &wgpu::Device = &renderer.gpu.device;
        let width = renderer.gpu.surface_config.width;
        let height = renderer.gpu.surface_config.height;
        let depth_texture = device.create_texture(
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
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }),
        );
        let depth_texture_view = depth_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(wgpu::TextureFormat::Depth32Float),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: None,
            mip_level_count: None,
        });
        let grid = create_grid(
            &renderer.gpu.device,
            renderer.gpu.surface_config.format,
            DEPTH_FORMAT,
        );
        let sky = create_sky(
            &renderer.gpu.device,
            &renderer.gpu.queue,
            renderer.gpu.surface_config.format,
            DEPTH_FORMAT,
        );
        let lines = create_line_renderer(&renderer.gpu.device, renderer.gpu.surface_config.format);
        let quads = create_quad_renderer(
            &renderer.gpu.device,
            renderer.gpu.surface_config.format,
            DEPTH_FORMAT,
        );
        let target = RenderTarget {
            color_texture,
            color_texture_view,
            depth_texture,
            depth_texture_view,
            grid,
            sky,
            lines,
            quads,
        };
        targets.push(target);
    });
    renderer.targets = targets;

    context.resources.graphics.viewport_size = (width, height);
}

/// This synchronizes the viewport uniforms with the world
fn update_panes_system(context: &mut crate::context::Context) {
    use crate::context::*;

    let viewports = context
        .resources
        .user_interface
        .tile_tree_context
        .viewport_tiles
        .values()
        .copied()
        .collect::<Vec<_>>();

    let mut camera_matrices = Vec::new();
    for (kind, viewport) in &viewports {
        let matrices = if let crate::ui::PaneKind::Scene {
            scene_index: _,
            active_camera_index,
        } = kind
        {
            let cameras = query_entities(context, CAMERA);

            if let Some(camera_entity) = cameras.get(*active_camera_index) {
                if let Some(camera) = get_component::<Camera>(context, *camera_entity, CAMERA) {
                    if let Some(transform) =
                        get_component::<GlobalTransform>(context, *camera_entity, GLOBAL_TRANSFORM)
                    {
                        let view = nalgebra_glm::inverse(&transform.0);
                        let projection =
                            camera.projection_matrix(viewport.width() / viewport.height());

                        Some(CameraMatrices {
                            view,
                            projection,
                            camera_position: transform.0.column(3).xyz(),
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        camera_matrices.push(matrices);
    }

    let lines: Vec<_> = query_entities(context, LINES | GLOBAL_TRANSFORM)
        .into_iter()
        .filter_map(|entity| {
            let Lines(lines) = get_component::<Lines>(context, entity, LINES)?;
            let global_transform =
                get_component::<GlobalTransform>(context, entity, GLOBAL_TRANSFORM)?;

            Some(
                lines
                    .iter()
                    .map(|line| {
                        let start_world = (global_transform.0
                            * nalgebra_glm::vec4(line.start.x, line.start.y, line.start.z, 1.0))
                        .xyz();
                        let end_world = (global_transform.0
                            * nalgebra_glm::vec4(line.end.x, line.end.y, line.end.z, 1.0))
                        .xyz();

                        LineInstance {
                            start: nalgebra_glm::vec4(
                                start_world.x,
                                start_world.y,
                                start_world.z,
                                1.0,
                            ),
                            end: nalgebra_glm::vec4(end_world.x, end_world.y, end_world.z, 1.0),
                            color: line.color,
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect();

    let quads: Vec<_> = query_entities(context, QUADS | GLOBAL_TRANSFORM)
        .into_iter()
        .filter_map(|entity| {
            let Quads(quads) = get_component::<Quads>(context, entity, QUADS)?;
            let global_transform =
                get_component::<GlobalTransform>(context, entity, GLOBAL_TRANSFORM)?;
            Some(
                quads
                    .iter()
                    .map(|quad| {
                        let scale = nalgebra_glm::scaling(&nalgebra_glm::vec3(
                            quad.size.x,
                            quad.size.y,
                            1.0,
                        ));
                        let offset = nalgebra_glm::translation(&nalgebra_glm::vec3(
                            quad.offset.x,
                            quad.offset.y,
                            quad.offset.z,
                        ));
                        let final_transform = global_transform.0 * offset * scale;
                        QuadInstance {
                            model_matrix_0: final_transform.column(0).into(),
                            model_matrix_1: final_transform.column(1).into(),
                            model_matrix_2: final_transform.column(2).into(),
                            model_matrix_3: final_transform.column(3).into(),
                            color: quad.color,
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect();

    let Some(renderer) = context.resources.graphics.renderer.as_mut() else {
        return;
    };

    for ((target, (kind, _viewport)), matrices) in renderer
        .targets
        .iter_mut()
        .zip(viewports.iter())
        .zip(camera_matrices.iter())
    {
        match kind {
            crate::ui::PaneKind::Scene {
                scene_index: _,
                active_camera_index: _,
            } => {
                if let Some(matrices) = matrices {
                    update_grid(matrices, &renderer.gpu.queue, &target.grid);
                    update_sky(matrices, &renderer.gpu.queue, &target.sky);
                    update_lines_uniform(
                        matrices,
                        &renderer.gpu.device,
                        &renderer.gpu.queue,
                        &mut target.lines,
                        lines.to_vec(),
                    );
                    update_quads_uniform(
                        matrices,
                        &renderer.gpu.device,
                        &renderer.gpu.queue,
                        &mut target.quads,
                        quads.to_vec(),
                    );
                }
            }
            crate::ui::PaneKind::Color(_) => {}
        }
    }
}

/// This system renders and presents the next frame
pub fn render_frame_system(context: &mut crate::context::Context) {
    update_panes_system(context);

    let viewports = context
        .resources
        .user_interface
        .tile_tree_context
        .viewport_tiles
        .values()
        .copied()
        .collect::<Vec<_>>();

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

    ensure_viewports(context, viewports.len());

    let Some(renderer) = context.resources.graphics.renderer.as_mut() else {
        return;
    };

    for (id, image_delta) in &textures_delta.set {
        renderer
            .ui
            .update_texture(&renderer.gpu.device, &renderer.gpu.queue, *id, image_delta);
    }

    for id in &textures_delta.free {
        renderer.ui.free_texture(id);
    }

    let mut encoder = renderer
        .gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

    renderer.ui.update_buffers(
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

    let surface_texture_view = surface_texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Clear Pass"),
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
                view: &renderer.ui_depth_texture_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    }

    viewports
        .iter()
        .zip(renderer.targets.iter())
        .for_each(|((kind, viewport), target)| {
            let viewport_size = (viewport.width() as u32, viewport.height() as u32);
            render_pane(&mut encoder, kind, target, viewport_size);

            let source_origin = wgpu::Origin3d { x: 0, y: 0, z: 0 };
            let destination_origin = wgpu::Origin3d {
                x: viewport.min.x as u32,
                y: viewport.min.y as u32,
                z: 0,
            };

            encoder.copy_texture_to_texture(
                wgpu::ImageCopyTexture {
                    texture: &target.color_texture,
                    mip_level: 0,
                    origin: source_origin,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::ImageCopyTexture {
                    texture: &surface_texture.texture,
                    mip_level: 0,
                    origin: destination_origin,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: viewport_size.0,
                    height: viewport_size.1,
                    depth_or_array_layers: 1,
                },
            );
        });

    {
        let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("GUI Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &surface_texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &renderer.ui_depth_texture_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        renderer.ui.render(
            &mut render_pass.forget_lifetime(),
            &paint_jobs,
            &screen_descriptor,
        );
    }

    renderer.gpu.queue.submit(std::iter::once(encoder.finish()));
    surface_texture.present();
}

fn render_pane(
    encoder: &mut wgpu::CommandEncoder,
    pane_kind: &crate::prelude::PaneKind,
    target: &RenderTarget,
    viewport_size: (u32, u32),
) {
    let clear_color = match pane_kind {
        crate::ui::PaneKind::Scene { .. } => wgpu::Color::BLACK,
        crate::ui::PaneKind::Color(color) => wgpu::Color {
            r: (color.r() as f64 / 255.0),
            g: (color.g() as f64 / 255.0),
            b: (color.b() as f64 / 255.0),
            a: 1.0,
        },
    };

    // Create viewport-sized render pass
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Viewport Render Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &target.color_texture_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear_color),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &target.depth_texture_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
    });

    // Set viewport to match pane size
    let viewport = egui::Rect::from_min_size(
        egui::pos2(0.0, 0.0),
        egui::vec2(viewport_size.0 as f32, viewport_size.1 as f32),
    );

    if viewport.width() <= 0.0 || viewport.height() <= 0.0 {
        return;
    }

    render_pass.set_viewport(
        viewport.min.x,
        viewport.min.y,
        viewport.width().max(1.0),
        viewport.height().max(1.0),
        0.0,
        1.0,
    );

    if matches!(pane_kind, crate::ui::PaneKind::Scene { .. }) {
        render_sky(&mut render_pass, &target.sky);
        render_lines(&mut render_pass, &target.lines);
        render_quads(&mut render_pass, &target.quads);
        render_grid(&mut render_pass, &target.grid);
    }
}

fn ensure_viewports(context: &mut crate::Context, viewport_count: usize) {
    let Some(renderer) = context.resources.graphics.renderer.as_mut() else {
        return;
    };

    if renderer.targets.len() >= viewport_count {
        return;
    }
    (0..(viewport_count - renderer.targets.len())).for_each(|_| {
        let color_texture = renderer
            .gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Viewport Texture"),
                size: wgpu::Extent3d {
                    width: renderer.gpu.surface_config.width,
                    height: renderer.gpu.surface_config.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: renderer.gpu.surface_config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[renderer.gpu.surface_config.format],
            });
        let color_texture_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let device: &wgpu::Device = &renderer.gpu.device;
        let width = renderer.gpu.surface_config.width;
        let height = renderer.gpu.surface_config.height;
        let depth_texture = device.create_texture(
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
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }),
        );
        let depth_texture_view = depth_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(wgpu::TextureFormat::Depth32Float),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            base_array_layer: 0,
            array_layer_count: None,
            mip_level_count: None,
        });
        let grid = create_grid(
            &renderer.gpu.device,
            renderer.gpu.surface_config.format,
            DEPTH_FORMAT,
        );
        let sky = create_sky(
            &renderer.gpu.device,
            &renderer.gpu.queue,
            renderer.gpu.surface_config.format,
            DEPTH_FORMAT,
        );
        let lines = create_line_renderer(&renderer.gpu.device, renderer.gpu.surface_config.format);
        let quads = create_quad_renderer(
            &renderer.gpu.device,
            renderer.gpu.surface_config.format,
            DEPTH_FORMAT,
        );
        renderer.targets.push(RenderTarget {
            color_texture,
            color_texture_view,
            depth_texture,
            depth_texture_view,
            grid,
            sky,
            lines,
            quads,
        });
    });
}

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub async fn create_renderer_async(
    window: impl Into<wgpu::SurfaceTarget<'static>>,
    width: u32,
    height: u32,
) -> crate::graphics::Renderer {
    let gpu = create_gpu_async(window, width, height).await;
    let ui_depth_texture_view = {
        let device: &wgpu::Device = &gpu.device;
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
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
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
    };
    let egui_renderer = egui_wgpu::Renderer::new(
        &gpu.device,
        gpu.surface_config.format,
        Some(DEPTH_FORMAT),
        1,
        false,
    );

    crate::graphics::Renderer {
        gpu,
        ui_depth_texture_view,
        ui: egui_renderer,
        targets: Vec::new(),
    }
}

/// This creates the low-level GPU resources needed for rendering
async fn create_gpu_async(
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
                    required_features: adapter.features(),
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
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
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
    }
}

pub use sky::*;
mod sky {
    pub struct Sky {
        pub uniform_buffer: wgpu::Buffer,
        pub texture: wgpu::Texture,
        pub texture_view: wgpu::TextureView,
        pub sampler: wgpu::Sampler,
        pub bind_group: wgpu::BindGroup,
        pub pipeline: wgpu::RenderPipeline,
    }

    #[repr(C)]
    #[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
    struct SkyUniform {
        proj: nalgebra_glm::Mat4,
        proj_inv: nalgebra_glm::Mat4,
        view: nalgebra_glm::Mat4,
        cam_pos: nalgebra_glm::Vec4,
    }

    pub fn create_sky(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Sky {
        let sky_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Sky Uniform Buffer"),
            size: std::mem::size_of::<SkyUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sky_texture = load_sky_texture(device, queue);
        let sky_texture_view = sky_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        let sky_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let sky_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Sky Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::Cube,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let sky_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &sky_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: sky_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&sky_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sky_sampler),
                },
            ],
            label: Some("Sky Bind Group"),
        });

        let sky_shader = device.create_shader_module(wgpu::include_wgsl!("shaders/sky.wgsl"));

        let sky_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sky Pipeline Layout"),
            bind_group_layouts: &[&sky_bind_group_layout],
            push_constant_ranges: &[],
        });

        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sky Pipeline"),
            layout: Some(&sky_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &sky_shader,
                entry_point: Some("vs_sky"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &sky_shader,
                entry_point: Some("fs_sky"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
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
        Sky {
            uniform_buffer: sky_uniform_buffer,
            texture: sky_texture,
            texture_view: sky_texture_view,
            sampler: sky_sampler,
            bind_group: sky_bind_group,
            pipeline: sky_pipeline,
        }
    }

    fn load_sky_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
        // TODO:
        // This HDR is big and we statically build it in,
        // so this contributes significantly to the final binary's filesize
        // and can be reduced by using a compressed format (like a ktx8)
        let hdr_data = include_bytes!("hdr/kloppenheim.hdr");
        let cursor = std::io::Cursor::new(hdr_data);
        let decoder =
            image::codecs::hdr::HdrDecoder::new(cursor).expect("Failed to create HDR decoder");
        let metadata = decoder.metadata();
        let decoded = decoder
            .read_image_hdr()
            .expect("Failed to decode HDR image");

        // Create source texture for equirectangular image
        let equirect_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Equirectangular Source Texture"),
            size: wgpu::Extent3d {
                width: metadata.width,
                height: metadata.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload HDR data
        let data: Vec<f32> = decoded
            .into_iter()
            .flat_map(|pixel| [pixel.0[0], pixel.0[1], pixel.0[2], 1.0])
            .collect();

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &equirect_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&data),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(metadata.width * 16), // 4 x f32
                rows_per_image: Some(metadata.height),
            },
            wgpu::Extent3d {
                width: metadata.width,
                height: metadata.height,
                depth_or_array_layers: 1,
            },
        );

        // Create destination cubemap texture
        let cubemap = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Sky Cubemap Texture"),
            size: wgpu::Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        // Create compute pipeline for cubemap generation
        let shader =
            device.create_shader_module(wgpu::include_wgsl!("shaders/equirect_to_cube.wgsl"));

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Cubemap Generation Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba32Float,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Cubemap Generation Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Cubemap Generation Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Cubemap Generation Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &equirect_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&device.create_sampler(
                        &wgpu::SamplerDescriptor {
                            label: Some("Equirect Sampler"),
                            address_mode_u: wgpu::AddressMode::ClampToEdge,
                            address_mode_v: wgpu::AddressMode::ClampToEdge,
                            address_mode_w: wgpu::AddressMode::ClampToEdge,
                            mag_filter: wgpu::FilterMode::Linear,
                            min_filter: wgpu::FilterMode::Linear,
                            mipmap_filter: wgpu::FilterMode::Linear,
                            ..Default::default()
                        },
                    )),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &cubemap.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
            ],
        });

        // Execute compute shader
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Cubemap Generation Encoder"),
        });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Cubemap Generation Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&compute_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);

            // Dispatch compute shader (64x64 workgroups for 1024x1024 faces, 6 faces)
            compute_pass.dispatch_workgroups(64, 64, 6);
        }

        queue.submit(Some(encoder.finish()));

        cubemap
    }

    pub fn update_sky(matrices: &crate::context::CameraMatrices, queue: &wgpu::Queue, sky: &Sky) {
        let uniform = SkyUniform {
            proj: matrices.projection,
            proj_inv: nalgebra_glm::inverse(&matrices.projection),
            view: matrices.view,
            cam_pos: nalgebra_glm::vec4(
                matrices.camera_position.x,
                matrices.camera_position.y,
                matrices.camera_position.z,
                1.0,
            ),
        };
        queue.write_buffer(&sky.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }

    pub fn render_sky(render_pass: &mut wgpu::RenderPass<'_>, sky: &Sky) {
        render_pass.set_pipeline(&sky.pipeline);
        render_pass.set_bind_group(0, &sky.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

pub use grid::*;
mod grid {
    pub struct Grid {
        pub uniform_buffer: wgpu::Buffer,
        pub bind_group: wgpu::BindGroup,
        pub pipeline: wgpu::RenderPipeline,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct GridUniform {
        pub view_proj: nalgebra_glm::Mat4,
        pub camera_world_pos: nalgebra_glm::Vec3,
        pub grid_size: f32,
        pub grid_min_pixels: f32,
        pub grid_cell_size: f32,
        pub _padding: [f32; 2],
    }

    pub fn create_grid(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
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
                    format: color_format,
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
                depth_write_enabled: true,
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

    pub fn render_grid(render_pass: &mut wgpu::RenderPass<'_>, grid: &Grid) {
        render_pass.set_pipeline(&grid.pipeline);
        render_pass.set_bind_group(0, &grid.bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }

    pub fn update_grid(
        matrices: &crate::context::CameraMatrices,
        queue: &wgpu::Queue,
        grid: &Grid,
    ) {
        let uniform = GridUniform {
            view_proj: matrices.projection * matrices.view,
            camera_world_pos: matrices.camera_position.xyz(),
            grid_size: 100.0,
            grid_min_pixels: 2.0,
            grid_cell_size: 0.025,
            _padding: [0.0; 2],
        };
        queue.write_buffer(&grid.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }
}

pub use lines::*;
mod lines {
    use super::*;

    pub struct Lines {
        pub vertex_buffer: wgpu::Buffer,
        pub instance_buffer: wgpu::Buffer,
        pub uniform_buffer: wgpu::Buffer,
        pub bind_group: wgpu::BindGroup,
        pub pipeline: wgpu::RenderPipeline,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct LineVertex {
        pub position: nalgebra_glm::Vec3,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct LineInstance {
        pub start: nalgebra_glm::Vec4,
        pub end: nalgebra_glm::Vec4,
        pub color: nalgebra_glm::Vec4,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct LineUniform {
        pub view_proj: nalgebra_glm::Mat4,
    }

    pub fn create_line_renderer(device: &wgpu::Device, format: wgpu::TextureFormat) -> Lines {
        let vertices = [
            LineVertex {
                position: nalgebra_glm::vec3(0.0, 0.0, 0.0),
            },
            LineVertex {
                position: nalgebra_glm::vec3(1.0, 0.0, 0.0),
            },
        ];

        let vertex_buffer = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("Line Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            },
        );

        let initial_instance_capacity = 1024;
        let instance_buffer_size = std::mem::size_of::<LineInstance>() * initial_instance_capacity;

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Line Instance Buffer"),
            size: instance_buffer_size as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Line Uniform Buffer"),
            size: std::mem::size_of::<LineUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

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
            label: Some("Line Bind Group Layout"),
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("Line Bind Group"),
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/line.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Line Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Line Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<LineVertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<LineInstance>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![
                            1 => Float32x4,
                            2 => Float32x4,
                            3 => Float32x4
                        ],
                    },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: -1, // Small negative bias to avoid z-fighting
                    slope_scale: 0.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Lines {
            vertex_buffer,
            instance_buffer,
            uniform_buffer,
            bind_group,
            pipeline,
        }
    }

    pub fn update_lines_uniform(
        matrices: &crate::context::CameraMatrices,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        lines: &mut Lines,
        instances: Vec<LineInstance>,
    ) {
        // Create the data that will be sent to the GPU
        let gpu_data = if instances.is_empty() {
            vec![LineInstance {
                start: nalgebra_glm::vec4(0.0, 0.0, 0.0, 0.0),
                end: nalgebra_glm::vec4(0.0, 0.0, 0.0, 0.0),
                color: nalgebra_glm::vec4(0.0, 0.0, 0.0, 0.0),
            }]
        } else {
            instances
        };

        let uniform = LineUniform {
            view_proj: matrices.projection * matrices.view,
        };

        queue.write_buffer(&lines.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));

        // Always recreate the buffer with the exact size needed
        lines.instance_buffer = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("Debug Line Instance Buffer"),
                contents: bytemuck::cast_slice(&gpu_data),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            },
        );
    }

    pub fn render_lines(render_pass: &mut wgpu::RenderPass, lines: &Lines) {
        let instance_size = std::mem::size_of::<LineInstance>();
        let debug_line_instance_count =
            (lines.instance_buffer.size() as usize / instance_size) as u32;
        if debug_line_instance_count > 0 {
            render_pass.set_pipeline(&lines.pipeline);
            render_pass.set_bind_group(0, &lines.bind_group, &[]);
            render_pass.set_vertex_buffer(0, lines.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, lines.instance_buffer.slice(..));
            render_pass.draw(0..2, 0..debug_line_instance_count);
        }
    }
}

pub use quads::*;
mod quads {
    use wgpu::util::DeviceExt as _;

    pub struct Quads {
        pub vertex_buffer: wgpu::Buffer,
        pub index_buffer: wgpu::Buffer,
        pub instance_buffer: wgpu::Buffer,
        pub uniform_buffer: wgpu::Buffer,
        pub bind_group: wgpu::BindGroup,
        pub pipeline: wgpu::RenderPipeline,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct QuadVertex {
        pub position: nalgebra_glm::Vec3,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct QuadInstance {
        pub model_matrix_0: nalgebra_glm::Vec4,
        pub model_matrix_1: nalgebra_glm::Vec4,
        pub model_matrix_2: nalgebra_glm::Vec4,
        pub model_matrix_3: nalgebra_glm::Vec4,
        pub color: nalgebra_glm::Vec4,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct QuadUniform {
        pub view_proj: nalgebra_glm::Mat4,
    }

    pub fn create_quad_renderer(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Quads {
        // Create a unit quad centered at origin in XY plane
        let vertices = [
            QuadVertex {
                position: nalgebra_glm::vec3(-0.5, -0.5, 0.0),
            },
            QuadVertex {
                position: nalgebra_glm::vec3(0.5, -0.5, 0.0),
            },
            QuadVertex {
                position: nalgebra_glm::vec3(0.5, 0.5, 0.0),
            },
            QuadVertex {
                position: nalgebra_glm::vec3(-0.5, 0.5, 0.0),
            },
        ];

        let indices: &[u16] = &[0, 1, 2, 2, 3, 0];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let initial_instance_capacity = 1024;
        let instance_buffer_size = std::mem::size_of::<QuadInstance>() * initial_instance_capacity;

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Quad Instance Buffer"),
            size: instance_buffer_size as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Quad Uniform Buffer"),
            size: std::mem::size_of::<QuadUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

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
            label: Some("Quad Bind Group Layout"),
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("Quad Bind Group"),
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/quad.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Quad Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Quad Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    // Vertex buffer
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<QuadVertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                    },
                    // Instance buffer
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<QuadInstance>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![
                            1 => Float32x4,
                            2 => Float32x4,
                            3 => Float32x4,
                            4 => Float32x4,
                            5 => Float32x4
                        ],
                    },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
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
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Quads {
            vertex_buffer,
            index_buffer,
            instance_buffer,
            uniform_buffer,
            bind_group,
            pipeline,
        }
    }

    pub fn update_quads_uniform(
        matrices: &crate::context::CameraMatrices,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        quads: &mut Quads,
        instances: Vec<QuadInstance>,
    ) {
        let uniform = QuadUniform {
            view_proj: matrices.projection * matrices.view,
        };

        queue.write_buffer(&quads.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));

        // Create the data that will be sent to the GPU
        let gpu_data = if instances.is_empty() {
            vec![QuadInstance {
                model_matrix_0: nalgebra_glm::vec4(0.0, 0.0, 0.0, 0.0),
                model_matrix_1: nalgebra_glm::vec4(0.0, 0.0, 0.0, 0.0),
                model_matrix_2: nalgebra_glm::vec4(0.0, 0.0, 0.0, 0.0),
                model_matrix_3: nalgebra_glm::vec4(0.0, 0.0, 0.0, 0.0),
                color: nalgebra_glm::vec4(0.0, 0.0, 0.0, 0.0),
            }]
        } else {
            instances
        };

        // Always recreate the buffer with the exact size needed
        quads.instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Instance Buffer"),
            contents: bytemuck::cast_slice(&gpu_data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
    }

    pub fn render_quads(render_pass: &mut wgpu::RenderPass, quads: &Quads) {
        let instance_size = std::mem::size_of::<QuadInstance>();
        let instance_count = (quads.instance_buffer.size() as usize / instance_size) as u32;
        if instance_count > 0 {
            render_pass.set_pipeline(&quads.pipeline);
            render_pass.set_bind_group(0, &quads.bind_group, &[]);
            render_pass.set_vertex_buffer(0, quads.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, quads.instance_buffer.slice(..));
            render_pass.set_index_buffer(quads.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..6, 0, 0..instance_count);
        }
    }
}
