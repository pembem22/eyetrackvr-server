use imgui::*;
use imgui_wgpu::{Renderer, RendererConfig};
use imgui_winit_support::WinitPlatform;
use pollster::block_on;
use std::{sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    platform::windows::EventLoopBuilderExtWindows,
    window::Window,
};

use crate::ui::{AppRenderer, AppRendererContext};

struct ImguiState {
    context: imgui::Context,
    platform: WinitPlatform,
    renderer: Renderer,
    clear_color: wgpu::Color,
    last_frame: Instant,
    last_cursor: Option<MouseCursor>,
}

struct AppWindowContainer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    hidpi_factor: f64,
    imgui: Option<ImguiState>,
}

pub(crate) struct AppWindow {
    window: Option<AppWindowContainer>,

    paused: bool,

    renderer: Option<AppRenderer>,
    renderer_context: AppRendererContext,
}

impl AppWindow {
    pub(crate) fn new(renderer_context: AppRendererContext) -> Self {
        AppWindow {
            window: None,

            paused: false,

            renderer: None,
            renderer_context,
        }
    }
}

impl AppWindow {
    pub(crate) fn new(renderer_context: AppRendererContext) -> Self {
        AppWindow {
            window: None,

            paused: false,

            renderer: None,
            renderer_context,
        }
    }
}

impl AppWindowContainer {
    fn setup_gpu(event_loop: &ActiveEventLoop) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let window = {
            let size = LogicalSize::new(1280.0, 720.0);

            let attributes = Window::default_attributes()
                .with_inner_size(size)
                .with_title("eyetrackvr-server".to_string());
            Arc::new(event_loop.create_window(attributes).unwrap())
        };

        let size = window.inner_size();
        let hidpi_factor = window.scale_factor();
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

        // Set up swap chain
        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
        };

        surface.configure(&device, &surface_desc);

        let imgui = None;
        Self {
            device,
            queue,
            window,
            surface_desc,
            surface,
            hidpi_factor,
            imgui,
        }
    }

    fn setup_imgui(&mut self) {
        let mut context = imgui::Context::create();
        let mut platform = imgui_winit_support::WinitPlatform::new(&mut context);
        platform.attach_window(
            context.io_mut(),
            &self.window,
            imgui_winit_support::HiDpiMode::Default,
        );
        context.set_ini_filename(None);

        let font_size = (13.0 * self.hidpi_factor) as f32;
        context.io_mut().font_global_scale = (1.0 / self.hidpi_factor) as f32;

        context.fonts().add_font(&[FontSource::DefaultFontData {
            config: Some(imgui::FontConfig {
                oversample_h: 1,
                pixel_snap_h: true,
                size_pixels: font_size,
                ..Default::default()
            }),
        }]);

        //
        // Set up dear imgui wgpu renderer
        //
        let clear_color = wgpu::Color {
            r: 0.1,
            g: 0.2,
            b: 0.3,
            a: 1.0,
        };

        let renderer_config = RendererConfig {
            texture_format: self.surface_desc.format,
            ..Default::default()
        };

        let renderer = Renderer::new(&mut context, &self.device, &self.queue, renderer_config);
        let last_frame = Instant::now();
        let last_cursor = None;

        self.imgui = Some(ImguiState {
            context,
            platform,
            renderer,
            clear_color,
            last_frame,
            last_cursor,
        })
    }

    fn new(event_loop: &ActiveEventLoop) -> Self {
        let mut window = Self::setup_gpu(event_loop);
        window.setup_imgui();
        window
    }
}

impl ApplicationHandler for AppWindow {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut window = AppWindowContainer::new(event_loop);
        self.renderer = Some(AppRenderer::new(
            &mut window.device,
            &mut window.imgui.as_mut().unwrap().renderer,
        ));
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let window = self.window.as_mut().unwrap();
        let imgui = window.imgui.as_mut().unwrap();

        match &event {
            WindowEvent::Resized(size) => {
                if size.width == 0 || size.height == 0 {
                    self.paused = true;
                    return;
                }

                self.paused = false;

                window.surface_desc = wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    width: size.width,
                    height: size.height,
                    present_mode: wgpu::PresentMode::Fifo,
                    desired_maximum_frame_latency: 2,
                    alpha_mode: wgpu::CompositeAlphaMode::Auto,
                    view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
                };

                window
                    .surface
                    .configure(&window.device, &window.surface_desc);
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
                // TODO: find a better way?
                std::process::exit(0);
            }
            // WindowEvent::KeyboardInput { event, .. } => {
            //     if let Key::Named(NamedKey::Escape) = event.logical_key {
            //         if event.state.is_pressed() {
            //             event_loop.exit();
            //         }
            //     }
            // }
            WindowEvent::RedrawRequested => {
                // let delta_s = imgui.last_frame.elapsed();
                let now = Instant::now();
                imgui
                    .context
                    .io_mut()
                    .update_delta_time(now - imgui.last_frame);
                imgui.last_frame = now;

                let frame = match window.surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(e) => {
                        eprintln!("dropped frame: {e:?}");
                        return;
                    }
                };
                imgui
                    .platform
                    .prepare_frame(imgui.context.io_mut(), &window.window)
                    .expect("Failed to prepare frame");
                let ui = imgui.context.frame();

                let renderer = self.renderer.as_mut().unwrap();
                renderer.update(
                    &mut self.renderer_context,
                    &window.queue,
                    &mut imgui.renderer,
                );
                renderer.render(ui);

                let mut encoder: wgpu::CommandEncoder = window
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                if imgui.last_cursor != ui.mouse_cursor() {
                    imgui.last_cursor = ui.mouse_cursor();
                    imgui.platform.prepare_render(ui, &window.window);
                }

                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(imgui.clear_color),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                imgui
                    .renderer
                    .render(
                        imgui.context.render(),
                        &window.queue,
                        &window.device,
                        &mut rpass,
                    )
                    .expect("Rendering failed");

                drop(rpass);

                window.queue.submit(Some(encoder.finish()));

                frame.present();
            }
            _ => (),
        }

        imgui.platform.handle_event::<()>(
            imgui.context.io_mut(),
            &window.window,
            &Event::WindowEvent { window_id, event },
        );
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: ()) {
        let window = self.window.as_mut().unwrap();
        let imgui = window.imgui.as_mut().unwrap();
        imgui.platform.handle_event::<()>(
            imgui.context.io_mut(),
            &window.window,
            &Event::UserEvent(event),
        );
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        let window = self.window.as_mut().unwrap();
        let imgui = window.imgui.as_mut().unwrap();
        imgui.platform.handle_event::<()>(
            imgui.context.io_mut(),
            &window.window,
            &Event::DeviceEvent { device_id, event },
        );
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let window = self.window.as_mut().unwrap();
        let imgui = window.imgui.as_mut().unwrap();
        window.window.request_redraw();
        imgui.platform.handle_event::<()>(
            imgui.context.io_mut(),
            &window.window,
            &Event::AboutToWait,
        );
    }
}

pub fn start_ui(renderer_context: AppRendererContext) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(|| {
        let event_loop = EventLoop::builder().with_any_thread(true).build().unwrap();
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop
            .run_app(&mut AppWindow::new(renderer_context))
            .unwrap();
    })
}
