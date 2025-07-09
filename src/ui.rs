use crate::camera::CAMERA_FRAME_SIZE;
use crate::inference::{
    EyeState, FRAME_CROP_H, FRAME_CROP_W, FRAME_CROP_X, FRAME_CROP_Y, FRAME_RESIZE_H,
    FRAME_RESIZE_W,
};
use crate::{Frame, camera_texture::CameraTexture};
use async_broadcast::Receiver;

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
    window::Window,
};

struct ImguiState {
    context: imgui::Context,
    platform: WinitPlatform,
    renderer: Renderer,
    clear_color: wgpu::Color,
    last_frame: Instant,
    last_cursor: Option<MouseCursor>,
}

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_desc: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    hidpi_factor: f64,
    imgui: Option<ImguiState>,
}

struct App {
    window: Option<AppWindow>,

    renderer: Option<AppRenderer>,
    renderer_context: AppRendererContext,
}

impl App {
    fn new(renderer_context: AppRendererContext) -> Self {
        App {
            window: None,

            renderer: None,
            renderer_context,
        }
    }
}

impl AppWindow {
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

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut window = AppWindow::new(event_loop);
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

// fn main() {
//     // env_logger::init();

//     let event_loop = EventLoop::new().unwrap();
//     event_loop.set_control_flow(ControlFlow::Poll);
//     event_loop.run_app(&mut App::default()).unwrap();
// }

pub struct AppRendererContext {
    pub l_rx: Receiver<Frame>,
    pub r_rx: Receiver<Frame>,
    pub f_rx: Receiver<Frame>,

    pub l_raw_rx: Receiver<EyeState>,
    pub r_raw_rx: Receiver<EyeState>,
    pub filtered_eyes_rx: Receiver<(EyeState, EyeState)>,
}

pub fn start_ui(renderer_context: AppRendererContext) {
    // let event_loop = EventLoop::new().unwrap();

    // // ControlFlow::Poll continuously runs the event loop, even if the OS hasn't
    // // dispatched any events. This is ideal for games and similar applications.
    // event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

    // let proxy = event_loop.create_proxy();

    // let mut app_window = AppWindow::new(gui_receivers);
    // // tokio::task::spawn_blocking(|| {});
    // event_loop.run_app(&mut app_window).unwrap();

    // TODO: This is blocking! Run on main thread and start an async runtime on another one?
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut App::new(renderer_context)).unwrap();
}

struct AppRenderer {
    r_texture: CameraTexture,
    f_texture: CameraTexture,
    l_texture: CameraTexture,

    l_raw_eye: EyeState,
    r_raw_eye: EyeState,
    filtered_eyes: (EyeState, EyeState),
}

impl AppRenderer {
    fn new(device: &mut wgpu::Device, renderer: &mut imgui_wgpu::Renderer) -> Self {
        AppRenderer {
            l_texture: CameraTexture::new(device, renderer, Some("L texture")),
            r_texture: CameraTexture::new(device, renderer, Some("R texture")),
            f_texture: CameraTexture::new(device, renderer, Some("F texture")),

            l_raw_eye: EyeState::default(),
            r_raw_eye: EyeState::default(),
            filtered_eyes: (EyeState::default(), EyeState::default()),
        }
    }

    fn update(
        &mut self,
        renderer_context: &mut AppRendererContext,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) {
        self.l_texture
            .update_texture(&mut renderer_context.l_rx, queue, renderer);
        self.r_texture
            .update_texture(&mut renderer_context.r_rx, queue, renderer);
        self.f_texture
            .update_texture(&mut renderer_context.f_rx, queue, renderer);

        self.l_raw_eye = renderer_context
            .l_raw_rx
            .try_recv()
            .unwrap_or(self.l_raw_eye);
        self.r_raw_eye = renderer_context
            .r_raw_rx
            .try_recv()
            .unwrap_or(self.r_raw_eye);

        self.filtered_eyes = renderer_context
            .filtered_eyes_rx
            .try_recv()
            .unwrap_or(self.filtered_eyes);
    }

    fn render(&self, ui: &imgui::Ui) {
        ui.window("Camera Feeds").build(move || {
            let group = ui.begin_group();
            self.l_texture.build(ui);
            let l_fps = self.l_texture.get_fps();
            ui.text(format!("Left Eye, fps: {l_fps:03.1}"));
            group.end();

            ui.same_line();

            let group = ui.begin_group();
            self.r_texture.build(ui);
            let r_fps = self.r_texture.get_fps();
            ui.text(format!("Right Eye, fps: {r_fps:03.1}"));
            group.end();

            ui.same_line();

            let group = ui.begin_group();
            self.f_texture.build(ui);
            let f_fps = self.f_texture.get_fps();
            ui.text(format!("Face, fps: {f_fps:03.1}"));
            group.end();
        });

        ui.window("Inference").build(move || {
            // Cropped Camera Feeds

            let draw_cropped_feed = |camera_texture: CameraTexture| {
                imgui::Image::new(
                    camera_texture.get_texture_id(),
                    [FRAME_RESIZE_W as f32, FRAME_RESIZE_H as f32],
                )
                .uv0([
                    1.0 - FRAME_CROP_X as f32 / CAMERA_FRAME_SIZE as f32,
                    FRAME_CROP_Y as f32 / CAMERA_FRAME_SIZE as f32,
                ])
                .uv1([
                    1.0 - (FRAME_CROP_X + FRAME_CROP_W) as f32 / CAMERA_FRAME_SIZE as f32,
                    (FRAME_CROP_Y + FRAME_CROP_H) as f32 / CAMERA_FRAME_SIZE as f32,
                ])
                .build(ui);
            };

            ui.text(format!("Cropped Camera Feeds"));
            let group = ui.begin_group();
            draw_cropped_feed(self.l_texture);
            ui.same_line();
            draw_cropped_feed(self.r_texture);
            group.end();

            // Generic eye state drawer

            let draw_eyelid_state = |state: EyeState| {
                const WIDGET_W: f32 = 10.0;
                const WIDGET_H: f32 = 150.0;

                const COLOR_NORMAL: ImColor32 = ImColor32::from_rgb(0, 148, 255);
                const COLOR_WIDE: ImColor32 = ImColor32::from_rgb(127, 201, 255);

                const SPLIT_POINT: f32 = 0.75;

                let progress = state.eyelid;

                let draw_list = ui.get_window_draw_list();
                let position = ui.cursor_screen_pos();

                let zero_y = position[1] + WIDGET_H;
                let split_y = position[1] + WIDGET_H * (1.0 - progress.min(SPLIT_POINT));
                let one_y = position[1] + WIDGET_H * (1.0 - progress);

                draw_list
                    .add_rect(
                        [position[0], zero_y],
                        [position[0] + WIDGET_W, split_y],
                        COLOR_NORMAL,
                    )
                    .filled(true)
                    .build();
                draw_list
                    .add_rect(
                        [position[0], split_y],
                        [position[0] + WIDGET_W, one_y],
                        COLOR_WIDE,
                    )
                    .filled(true)
                    .build();

                // Advance cursor to avoid overlapping with next UI element
                ui.dummy([WIDGET_W, WIDGET_H]);
            };

            let draw_gaze_state = |state: EyeState| {
                const WIDGET_SIZE: f32 = 150.0;
                const FOV_SIZE: f32 = 0.95;
                const FOV_RANGE: f32 = 90.0;
                const FOV_RANGE_DIV_2: f32 = FOV_RANGE / 2.0;

                const GAZE_RADIUS: f32 = 5.0;

                const COLOR_BACKGROUND: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
                const COLOR_AXES: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
                const COLOR_CIRCLES: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
                const COLOR_GAZE: [f32; 4] = [0.0, 0.1, 0.4, 1.0];

                let position = ui.cursor_screen_pos();
                let size = WIDGET_SIZE;

                let draw_list = ui.get_window_draw_list();

                // Define the center of the drawing area
                let center = [position[0] + size * 0.5, position[1] + size * 0.5];

                // Define square corners
                let top_left = [center[0] - size * 0.5, center[1] - size * 0.5];
                let bottom_right = [center[0] + size * 0.5, center[1] + size * 0.5];

                // Draw white square
                draw_list
                    .add_rect(top_left, bottom_right, COLOR_BACKGROUND)
                    .filled(true)
                    .build();

                // Draw axes
                draw_list
                    .add_line(
                        [center[0], top_left[1]],
                        [center[0], bottom_right[1]],
                        COLOR_AXES,
                    )
                    .build(); // Vertical axis
                draw_list
                    .add_line(
                        [top_left[0], center[1]],
                        [bottom_right[0], center[1]],
                        COLOR_AXES,
                    )
                    .build(); // Horizontal axis

                let max_radius = size * FOV_SIZE / 2.0;
                draw_list
                    .add_circle(center, max_radius, COLOR_CIRCLES)
                    .build();

                for i in (15..FOV_RANGE_DIV_2 as i32).step_by(15) {
                    draw_list
                        .add_circle(
                            center,
                            i as f32 / FOV_RANGE_DIV_2 * max_radius,
                            COLOR_CIRCLES,
                        )
                        .build();
                }

                draw_list
                    .add_circle(
                        [
                            center[0] + state.yaw / FOV_RANGE_DIV_2 * max_radius,
                            center[1] + state.pitch / FOV_RANGE_DIV_2 * max_radius,
                        ],
                        GAZE_RADIUS,
                        COLOR_GAZE,
                    )
                    .filled(true)
                    .build();

                // Advance cursor to avoid overlapping with next UI element
                ui.dummy([size, size]);
            };

            // Raw Eye State

            ui.text(format!("Raw Eye State"));
            let group = ui.begin_group();
            draw_eyelid_state(self.l_raw_eye);
            ui.same_line();
            draw_gaze_state(self.l_raw_eye);
            ui.same_line();
            draw_gaze_state(self.r_raw_eye);
            ui.same_line();
            draw_eyelid_state(self.r_raw_eye);
            group.end();

            // Filtered Eye State

            ui.text(format!("Filtered Eye State"));
            let group = ui.begin_group();
            draw_eyelid_state(self.filtered_eyes.0);
            ui.same_line();
            draw_gaze_state(self.filtered_eyes.0);
            ui.same_line();
            draw_gaze_state(self.filtered_eyes.1);
            ui.same_line();
            draw_eyelid_state(self.filtered_eyes.1);
            group.end();
        });
    }
}
