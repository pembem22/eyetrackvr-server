use crate::{camera_texture::CameraTexture, ui, Frame};
use async_broadcast::Receiver;
use imgui::*;
use imgui_wgpu::{Renderer, RendererConfig};
use pollster::block_on;
use std::sync::{Arc, Mutex};
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

                    render(ui, &self.queue, &mut self.renderer);

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

pub fn start_ui(l_rx: Receiver<Frame>, r_rx: Receiver<Frame>, f_rx: Receiver<Frame>) -> JoinHandle<()> {
    tokio::task::spawn_blocking(|| {
        let mut ui = ui::UI::new();

        let mut l_texture = CameraTexture::new(&mut ui, Some("L texture"));
        let mut r_texture = CameraTexture::new(&mut ui, Some("R texture"));
        let mut f_texture = CameraTexture::new(&mut ui, Some("F texture"));

        let l_rx = Arc::new(Mutex::new(l_rx));
        let r_rx = Arc::new(Mutex::new(r_rx));
        let f_rx = Arc::new(Mutex::new(f_rx));

        ui.run(move |imgui, queue, renderer| {
            let mut l_rx = l_rx.lock().unwrap();
            let mut r_rx = r_rx.lock().unwrap();
            let mut f_rx = f_rx.lock().unwrap();

            l_texture.update_texture(&mut l_rx, queue, renderer);
            r_texture.update_texture(&mut r_rx, queue, renderer);
            f_texture.update_texture(&mut f_rx, queue, renderer);

            imgui.window("Camera Feeds").build(move || {
                let group = imgui.begin_group();
                l_texture.build(imgui);
                let l_fps = l_texture.get_fps();
                imgui.text(format!("Left Eye, fps: {l_fps:03.1}"));
                group.end();

                imgui.same_line();
                
                let group = imgui.begin_group();
                r_texture.build(imgui);
                let r_fps = r_texture.get_fps();
                imgui.text(format!("Right Eye, fps: {r_fps:03.1}"));
                group.end();
                
                imgui.same_line();
                
                let group = imgui.begin_group();
                f_texture.build(imgui);
                let f_fps = f_texture.get_fps();
                imgui.text(format!("Face, fps: {f_fps:03.1}"));
                group.end();
            });
        });
    })
}
