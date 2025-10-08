use imgui::*;
use imgui_wgpu::{Renderer, RendererConfig};
use imgui_winit_support::WinitPlatform;
use pollster::block_on;
use std::{sync::Arc, time::Instant};
#[cfg(target_os = "android")]
use winit::platform::android::EventLoopBuilderExtAndroid;
#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{self, ActiveEventLoop, ControlFlow, EventLoop},
    window::Window,
};

use crate::ui::{AppRenderer, AppRendererContext, UI_WINDOW_H, UI_WINDOW_W};

struct ImguiState {
    context: Option<imgui::Context>,
    platform: WinitPlatform,
    renderer: Renderer,
    clear_color: wgpu::Color,
    last_frame: Instant,
    last_cursor: Option<MouseCursor>,
    last_finger: Option<u64>,
}

struct AppWindowContainer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    hidpi_factor: f64,
    imgui: ImguiState,

    surface_format: wgpu::TextureFormat,
}

impl AppWindowContainer {
    fn new(event_loop: &ActiveEventLoop) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let window = {
            let size = LogicalSize::new(UI_WINDOW_W as f32, UI_WINDOW_H as f32);

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

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an Srgb surface texture. Using a different
        // one will result all the colors comming out darker. If you want to support non
        // Srgb surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .filter(|f| f.is_srgb())
            .next()
            .unwrap_or(surface_caps.formats[0]);

        // Set up swap chain
        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };

        let (device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

        surface.configure(&device, &surface_desc);

        let mut context = imgui::Context::create();
        let mut platform = imgui_winit_support::WinitPlatform::new(&mut context);
        platform.attach_window(
            context.io_mut(),
            &window,
            imgui_winit_support::HiDpiMode::Default,
        );
        context.set_ini_filename(None);

        let font_size = (13.0 * hidpi_factor) as f32;
        context.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

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
            texture_format: surface_desc.format,
            ..Default::default()
        };

        let renderer = Renderer::new(&mut context, &device, &queue, renderer_config);
        let last_frame = Instant::now();
        let last_cursor = None;

        let imgui = ImguiState {
            context: Some(context),
            platform,
            renderer,
            clear_color,
            last_frame,
            last_cursor,
            last_finger: None,
        };

        Self {
            device,
            queue,
            window,
            surface_desc,
            surface,
            hidpi_factor,
            imgui,
            surface_format,
        }
    }
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
    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.renderer = None;
        self.window = None;
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut window = AppWindowContainer::new(event_loop);
        self.renderer = Some(AppRenderer::new(
            &mut window.device,
            &mut window.imgui.renderer,
        ));
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_mut() else {
            return;
        };
        let imgui = &mut window.imgui;
        let imgui_ctx = imgui.context.as_mut().unwrap();

        match &event {
            WindowEvent::Resized(size) => {
                if size.width == 0 || size.height == 0 {
                    self.paused = true;
                    return;
                }

                self.paused = false;

                window.surface_desc = wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: window.surface_format,
                    width: size.width,
                    height: size.height,
                    present_mode: wgpu::PresentMode::Fifo,
                    desired_maximum_frame_latency: 2,
                    alpha_mode: wgpu::CompositeAlphaMode::Auto,
                    view_formats: vec![],
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
                imgui_ctx.io_mut().update_delta_time(now - imgui.last_frame);
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
                    .prepare_frame(imgui_ctx.io_mut(), &window.window)
                    .expect("Failed to prepare frame");
                let ui = imgui_ctx.frame();

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
                        imgui_ctx.render(),
                        &window.queue,
                        &window.device,
                        &mut rpass,
                    )
                    .expect("Rendering failed");

                drop(rpass);

                window.queue.submit(Some(encoder.finish()));

                frame.present();
            }
            WindowEvent::Touch(touch) => match touch.phase {
                winit::event::TouchPhase::Started => {
                    if imgui.last_finger.is_none() {
                        imgui.last_finger = Some(touch.id);

                        let location = touch.location.to_logical(window.window.scale_factor());
                        let location = imgui
                            .platform
                            .scale_pos_from_winit(&window.window, location);
                        imgui_ctx
                            .io_mut()
                            .add_mouse_pos_event([location.x as f32, location.y as f32]);

                        imgui_ctx
                            .io_mut()
                            .add_mouse_button_event(MouseButton::Left, true);
                    }
                }
                winit::event::TouchPhase::Moved => {
                    if let Some(finger) = imgui.last_finger
                        && finger == touch.id
                    {
                        let location = touch.location.to_logical(window.window.scale_factor());
                        let location = imgui
                            .platform
                            .scale_pos_from_winit(&window.window, location);
                        imgui_ctx
                            .io_mut()
                            .add_mouse_pos_event([location.x as f32, location.y as f32]);
                    }
                }
                winit::event::TouchPhase::Cancelled | winit::event::TouchPhase::Ended => {
                    if let Some(finger) = imgui.last_finger
                        && finger == touch.id
                    {
                        imgui.last_finger = None;
                        imgui_ctx
                            .io_mut()
                            .add_mouse_button_event(MouseButton::Left, false);
                    }
                }
            },
            _ => (),
        }

        imgui.platform.handle_event::<()>(
            imgui_ctx.io_mut(),
            &window.window,
            &Event::WindowEvent { window_id, event },
        );
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        let Some(window) = self.window.as_mut() else {
            return;
        };
        let imgui = &mut window.imgui;
        let imgui_ctx = imgui.context.as_mut().unwrap();

        imgui.platform.handle_event::<()>(
            imgui_ctx.io_mut(),
            &window.window,
            &Event::DeviceEvent { device_id, event },
        );
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.as_mut() else {
            return;
        };
        let imgui = &mut window.imgui;
        let imgui_ctx = imgui.context.as_mut().unwrap();

        window.window.request_redraw();
        imgui
            .platform
            .handle_event::<()>(imgui_ctx.io_mut(), &window.window, &Event::AboutToWait);
    }
}

struct AppWindowWrapper {
    inner: AppWindow,
}

impl AppWindowWrapper {
    pub(crate) fn new(renderer_context: AppRendererContext) -> Self {
        Self {
            inner: AppWindow::new(renderer_context),
        }
    }
}

impl ApplicationHandler<()> for AppWindowWrapper {
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.suspended(event_loop);
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.resumed(event_loop)
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        self.inner.window_event(event_loop, window_id, event)
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: ()) {
        let window = self.inner.window.as_mut().unwrap();
        let imgui = &mut window.imgui;
        let imgui_ctx = imgui.context.as_mut().unwrap();

        imgui.platform.handle_event::<()>(
            imgui_ctx.io_mut(),
            &window.window,
            &Event::UserEvent(event),
        );
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        self.inner.device_event(event_loop, device_id, event)
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.about_to_wait(event_loop)
    }
}

impl ApplicationHandler<AndroidApp> for AppWindowWrapper {
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.suspended(event_loop);
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.resumed(event_loop)
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        self.inner.window_event(event_loop, window_id, event)
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AndroidApp) {
        let window = self.inner.window.as_mut().unwrap();
        let imgui = &mut window.imgui;
        let imgui_ctx = imgui.context.as_mut().unwrap();

        imgui.platform.handle_event::<AndroidApp>(
            imgui_ctx.io_mut(),
            &window.window,
            &Event::UserEvent(event),
        );
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        self.inner.device_event(event_loop, device_id, event)
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.about_to_wait(event_loop)
    }
}

#[cfg(not(target_os = "android"))]
pub fn start_ui(renderer_context: AppRendererContext) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(|| {
        let event_loop = EventLoop::builder().with_any_thread(true).build().unwrap();
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop
            .run_app(&mut AppWindowWrapper::new(renderer_context))
            .unwrap();
    })
}

#[cfg(target_os = "android")]
pub fn start_ui(android_app: AndroidApp, renderer_context: AppRendererContext) {
    let event_loop = EventLoop::<AndroidApp>::with_user_event()
        .with_android_app(android_app)
        .build()
        .unwrap();
    // event_loop.set_control_flow(ControlFlow::Poll);
    event_loop
        .run_app(&mut AppWindowWrapper::new(renderer_context))
        .unwrap();
}
