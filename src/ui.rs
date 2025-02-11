use crate::camera::CAMERA_FRAME_SIZE;
use crate::inference::{
    EyeState, FRAME_CROP_H, FRAME_CROP_W, FRAME_CROP_X, FRAME_CROP_Y, FRAME_RESIZE_H,
    FRAME_RESIZE_W,
};
use crate::{camera_texture::CameraTexture, ui, Frame};
use async_broadcast::Receiver;
use imgui::*;
use imgui_wgpu::{Renderer, RendererConfig};
use pollster::block_on;
use std::time::Instant;
use tokio::task::JoinHandle;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::windows::EventLoopBuilderExtWindows,
    window::Window,
};

pub(crate) struct UI {
    event_loop: winit::event_loop::EventLoop<()>,
    surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    window: winit::window::Window,
    imgui: imgui::Context,
    platform: imgui_winit_support::WinitPlatform,
    pub renderer: imgui_wgpu::Renderer,

    pause: bool,
}

impl UI {
    pub fn new() -> UI {
        env_logger::init();

        // Set up window and GPU
        let event_loop = EventLoopBuilder::new().with_any_thread(true).build();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let (window, size, surface) = {
            let version = env!("CARGO_PKG_VERSION");

            let window = Window::new(&event_loop).unwrap();
            window.set_inner_size(LogicalSize {
                width: 1280.0,
                height: 720.0,
            });
            window.set_title(&format!("imgui-wgpu {version}"));
            let size = window.inner_size();

            let surface = unsafe { instance.create_surface(&window) }.unwrap();

            (window, size, surface)
        };

        let hidpi_factor = window.scale_factor();

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) = block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::default(),
            },
            None,
        ))
        .unwrap();

        // Set up swap chain
        let surface_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
        };

        surface.configure(&device, &surface_desc);

        // Set up dear imgui
        let mut imgui = imgui::Context::create();
        let mut platform = imgui_winit_support::WinitPlatform::init(&mut imgui);
        platform.attach_window(
            imgui.io_mut(),
            &window,
            imgui_winit_support::HiDpiMode::Default,
        );
        imgui.set_ini_filename(None);

        let font_size = (13.0 * hidpi_factor) as f32;
        imgui.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

        imgui.fonts().add_font(&[FontSource::DefaultFontData {
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
        let renderer_config = RendererConfig {
            texture_format: surface_desc.format,
            ..Default::default()
        };

        let renderer = Renderer::new(&mut imgui, &device, &queue, renderer_config);

        // Set up Lenna texture
        // let lenna_bytes = include_bytes!("../resources/checker.png");
        // let image =
        //     image::load_from_memory_with_format(lenna_bytes, ImageFormat::Png).expect("invalid image");
        // let image = image.to_rgba8();
        // let (width, height) = image.dimensions();
        // let raw_data = image.into_raw();

        // let texture_config = TextureConfig {
        //     size: Extent3d {
        //         width,
        //         height,
        //         ..Default::default()
        //     },
        //     label: Some("lenna texture"),
        //     format: Some(wgpu::TextureFormat::Rgba8Unorm),
        //     ..Default::default()
        // };

        // let texture = Texture::new(&device, &renderer, texture_config);

        // texture.write(&queue, &raw_data, width, height);
        // let lenna_texture_id = renderer.textures.insert(texture);

        // init(&device, &queue, &mut renderer);

        UI {
            event_loop,
            surface,
            device,
            queue,
            window,
            imgui,
            platform,
            renderer,
            pause: false,
        }
    }

    pub fn run<F: FnMut(&imgui::Ui, &wgpu::Queue, &mut imgui_wgpu::Renderer) + 'static>(
        mut self,
        mut render: F,
    ) {
        let clear_color = wgpu::Color {
            r: 0.1,
            g: 0.2,
            b: 0.3,
            a: 1.0,
        };

        let mut last_frame = Instant::now();
        let mut last_cursor = None;

        self.event_loop.run(move |event, _, control_flow| {
            *control_flow = if cfg!(feature = "metal-auto-capture") {
                ControlFlow::Exit
            } else {
                ControlFlow::Poll
            };
            match event {
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    if size.width == 0 || size.height == 0 {
                        self.pause = true;
                        return;
                    }

                    self.pause = false;

                    let surface_desc = wgpu::SurfaceConfiguration {
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        format: wgpu::TextureFormat::Bgra8UnormSrgb,
                        width: size.width,
                        height: size.height,
                        present_mode: wgpu::PresentMode::Fifo,
                        alpha_mode: wgpu::CompositeAlphaMode::Auto,
                        view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
                    };

                    self.surface.configure(&self.device, &surface_desc);
                }
                Event::WindowEvent {
                    event:
                        WindowEvent::KeyboardInput {
                            input:
                                KeyboardInput {
                                    virtual_keycode: Some(VirtualKeyCode::Escape),
                                    state: ElementState::Pressed,
                                    ..
                                },
                            ..
                        },
                    ..
                }
                | Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    *control_flow = ControlFlow::Exit;
                }
                Event::MainEventsCleared => {
                    self.window.request_redraw();
                }
                Event::RedrawEventsCleared => {
                    let now = Instant::now();
                    self.imgui.io_mut().update_delta_time(now - last_frame);
                    last_frame = now;

                    let frame = match self.surface.get_current_texture() {
                        Ok(frame) => frame,
                        Err(e) => {
                            eprintln!("dropped frame: {e:?}");
                            return;
                        }
                    };
                    self.platform
                        .prepare_frame(self.imgui.io_mut(), &self.window)
                        .expect("Failed to prepare frame");
                    let ui = self.imgui.frame();

                    if !self.pause {
                        render(ui, &self.queue, &mut self.renderer);
                    }

                    // {
                    //     let size = [width as f32, height as f32];
                    //     let window = ui.window("Hello world");
                    //     window
                    //         .size([400.0, 600.0], Condition::FirstUseEver)
                    //         .build(|| {
                    //             ui.text("Hello textures!");
                    //             ui.text("Say hello to checker.png");
                    //             Image::new(lenna_texture_id, size).build(ui);
                    //         });
                    // }

                    let mut encoder: wgpu::CommandEncoder = self
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                    if last_cursor != Some(ui.mouse_cursor()) {
                        last_cursor = Some(ui.mouse_cursor());
                        self.platform.prepare_render(ui, &self.window);
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
                                load: wgpu::LoadOp::Clear(clear_color),
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: None,
                    });

                    self.renderer
                        .render(self.imgui.render(), &self.queue, &self.device, &mut rpass)
                        .expect("Rendering failed");

                    drop(rpass);

                    self.queue.submit(Some(encoder.finish()));
                    frame.present();
                }
                _ => (),
            }

            self.platform
                .handle_event(self.imgui.io_mut(), &self.window, &event);
        });
    }
}

pub fn start_ui(
    l_rx: Receiver<Frame>,
    r_rx: Receiver<Frame>,
    f_rx: Receiver<Frame>,

    l_raw_eye_rx: Receiver<EyeState>,
    r_raw_eye_rx: Receiver<EyeState>,
    filtered_eyes_rx: Receiver<(EyeState, EyeState)>,
) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let mut ui = ui::UI::new();

        let mut l_texture = CameraTexture::new(&mut ui, Some("L texture"));
        let mut r_texture = CameraTexture::new(&mut ui, Some("R texture"));
        let mut f_texture = CameraTexture::new(&mut ui, Some("F texture"));

        let mut l_rx = l_rx.clone();
        let mut r_rx = r_rx.clone();
        let mut f_rx = f_rx.clone();

        let mut l_raw_eye_rx = l_raw_eye_rx.clone();
        let mut r_raw_eye_rx = r_raw_eye_rx.clone();
        let mut filtered_eyes_rx = filtered_eyes_rx.clone();

        let mut l_raw_eye = EyeState::default();
        let mut r_raw_eye = EyeState::default();
        let mut filtered_eyes = (EyeState::default(), EyeState::default());

        ui.run(move |ui, queue, renderer| {
            l_texture.update_texture(&mut l_rx, queue, renderer);
            r_texture.update_texture(&mut r_rx, queue, renderer);
            f_texture.update_texture(&mut f_rx, queue, renderer);

            l_raw_eye = l_raw_eye_rx.try_recv().unwrap_or(l_raw_eye);
            r_raw_eye = r_raw_eye_rx.try_recv().unwrap_or(r_raw_eye);

            filtered_eyes = filtered_eyes_rx.try_recv().unwrap_or(filtered_eyes);

            ui.window("Camera Feeds").build(move || {
                let group = ui.begin_group();
                l_texture.build(ui);
                let l_fps = l_texture.get_fps();
                ui.text(format!("Left Eye, fps: {l_fps:03.1}"));
                group.end();

                ui.same_line();

                let group = ui.begin_group();
                r_texture.build(ui);
                let r_fps = r_texture.get_fps();
                ui.text(format!("Right Eye, fps: {r_fps:03.1}"));
                group.end();

                ui.same_line();

                let group = ui.begin_group();
                f_texture.build(ui);
                let f_fps = f_texture.get_fps();
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
                draw_cropped_feed(l_texture);
                ui.same_line();
                draw_cropped_feed(r_texture);
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
                draw_eyelid_state(l_raw_eye);
                ui.same_line();
                draw_gaze_state(l_raw_eye);
                ui.same_line();
                draw_gaze_state(r_raw_eye);
                ui.same_line();
                draw_eyelid_state(r_raw_eye);
                group.end();

                // Filtered Eye State

                ui.text(format!("Filtered Eye State"));
                let group = ui.begin_group();
                draw_eyelid_state(filtered_eyes.0);
                ui.same_line();
                draw_gaze_state(filtered_eyes.0);
                ui.same_line();
                draw_gaze_state(filtered_eyes.1);
                ui.same_line();
                draw_eyelid_state(filtered_eyes.1);
                group.end();
            });
        });
    })
}
