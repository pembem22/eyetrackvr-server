use core::ffi::c_void;
use std::{ffi::c_char, ptr, time::Instant};

use glow::HasContext;
use log::debug;
use pollster::block_on;

use crate::{
    openxr_layer::{
        input::UiEvent,
        layer::{EGLPointers, LAYER},
    },
    ui::{AppRenderer, AppRendererContext, UI_WINDOW_H, UI_WINDOW_W},
};

struct ImguiState {
    context: imgui::Context,
    renderer: imgui_wgpu::Renderer,
    clear_color: wgpu::Color,
    last_frame: Instant,
    last_cursor: Option<imgui::MouseCursor>,
}

impl ImguiState {
    fn setup_imgui(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let mut context = imgui::Context::create();
        context.set_ini_filename(None);

        let hidpi_factor = 1.0;
        let font_size = (13.0 * hidpi_factor) as f32;
        context.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;
        context.io_mut().display_framebuffer_scale = [hidpi_factor, hidpi_factor];
        context.io_mut().display_size = [UI_WINDOW_W as f32, UI_WINDOW_H as f32];

        context
            .fonts()
            .add_font(&[imgui::FontSource::DefaultFontData {
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
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };

        let renderer_config = imgui_wgpu::RendererConfig {
            texture_format: wgpu::TextureFormat::Rgba8UnormSrgb,
            ..Default::default()
        };

        let renderer = imgui_wgpu::Renderer::new(&mut context, &device, &queue, renderer_config);
        let last_frame = Instant::now();
        let last_cursor = None;

        Self {
            context,
            renderer,
            clear_color,
            last_frame,
            last_cursor,
        }
    }
}

#[link(name = "EGL")]
unsafe extern "C" {
    unsafe fn eglMakeCurrent(
        display: *mut c_void,
        draw: *mut c_void,
        read: *mut c_void,
        context: *mut c_void,
    ) -> u8;
    unsafe fn eglGetCurrentContext() -> *const c_void;
    unsafe fn eglGetProcAddress(name: *const c_char) -> *const c_void;
}

struct RenderContext {
    device: wgpu::Device,
    queue: wgpu::Queue,

    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,

    gl: glow::Context,
    openxr_fb: glow::Framebuffer,
    imgui_fb: glow::Framebuffer,

    imgui: ImguiState,

    renderer: AppRenderer,
    renderer_ctx: AppRendererContext,
}

impl RenderContext {
    #![warn(static_mut_refs)]
    unsafe fn init_on_current_context(
        renderer_ctx: AppRendererContext,
        egl_pointers: &EGLPointers,
    ) -> RenderContext {
        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|cstr| {
                eglGetProcAddress(cstr.as_ptr()) as *const _
            })
        };
        let openxr_fb = unsafe { gl.create_framebuffer().unwrap() };
        let imgui_fb = unsafe { gl.create_framebuffer().unwrap() };

        unsafe {
            wgpu::hal::gles::EGL_CONTEXT
                .insert(khronos_egl::Context::from_ptr(egl_pointers.context))
        };
        unsafe {
            wgpu::hal::gles::EGL_DISPLAY
                .insert(khronos_egl::Display::from_ptr(egl_pointers.display))
        };

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::GL,
            flags: wgpu::InstanceFlags::advanced_debugging(),
            ..Default::default()
        });

        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (mut device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

        let texture = device.create_texture(&wgpu::wgt::TextureDescriptor {
            label: Some("imgui texture"),
            size: wgpu::Extent3d {
                width: UI_WINDOW_W,
                height: UI_WINDOW_H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
        });
        let texture_view = texture.create_view(&Default::default());

        let mut imgui: ImguiState = ImguiState::setup_imgui(&device, &queue);

        let renderer = AppRenderer::new(&mut device, &mut imgui.renderer);

        RenderContext {
            device,
            queue,
            texture,
            texture_view,
            gl,
            openxr_fb,
            imgui_fb,
            imgui,
            renderer,
            renderer_ctx,
        }
    }

    fn render(&mut self) {
        let imgui = &mut self.imgui;
        let io = imgui.context.io_mut();

        // Handle events from OpenXR inputs.
        unsafe {
            if let Some(inputs) = &LAYER.inputs {
                for event in &inputs.events {
                    debug!("{:?}", event);
                    match event {
                        UiEvent::PointerMove { x, y } => {
                            io.add_mouse_pos_event([
                                x * UI_WINDOW_W as f32,
                                y * UI_WINDOW_H as f32,
                            ]);
                        }

                        UiEvent::PointerButton { down } => {
                            io.add_mouse_button_event(imgui::MouseButton::Left, *down);
                        }
                    }
                }
            }
        }

        // Render the frame.

        let now = Instant::now();
        io.update_delta_time(now - imgui.last_frame);
        imgui.last_frame = now;

        let ui = imgui.context.frame();

        let renderer = &mut self.renderer;
        renderer.update(&mut self.renderer_ctx, &self.queue, &mut imgui.renderer);
        unsafe {
            let openxr_layers = &mut LAYER.modules;
            renderer.render(ui, openxr_layers);
        }

        let mut encoder: wgpu::CommandEncoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        if imgui.last_cursor != ui.mouse_cursor() {
            imgui.last_cursor = ui.mouse_cursor();
        }

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.texture_view,
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
                &self.queue,
                &self.device,
                &mut rpass,
            )
            .expect("Rendering failed");

        drop(rpass);

        self.queue.submit(Some(encoder.finish()));
    }
}

pub fn start_ui(renderer_ctx: AppRendererContext) -> tokio::task::JoinHandle<()> {
    // FIXME: Apparently this is undefined behavior, figure this out.
    #![warn(static_mut_refs)]
    tokio::task::spawn_blocking(|| {
        println!("Hello from the render thread");
        let render_signal = unsafe { &mut LAYER.render_signal };
        let egl_pointers = unsafe { LAYER.egl_pointers.as_ref().unwrap() };

        let mut maybe_render_ctx = None;
        let mut renderer_ctx = Some(renderer_ctx);

        loop {
            // Wait for the render thread to be woken up.
            let mut ready = render_signal.mutex.lock().unwrap();
            while !*ready {
                ready = render_signal.condvar.wait(ready).unwrap();
            }

            // Make sure the context is on the current thread.
            unsafe {
                eglMakeCurrent(
                    egl_pointers.display,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    egl_pointers.context,
                );
            }

            let render_ctx = maybe_render_ctx.get_or_insert_with(|| unsafe {
                RenderContext::init_on_current_context(
                    renderer_ctx.take().expect("renderer_ctx was already taken"),
                    egl_pointers,
                )
            });

            render_ctx.render();

            unsafe {
                let gl = &mut render_ctx.gl;
                gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(render_ctx.imgui_fb));
                let texture_id = render_ctx
                    .texture
                    .as_hal::<wgpu::hal::api::Gles, _, _>(|x| {
                        if let wgpu::hal::gles::TextureInner::Texture { raw, .. } = x.unwrap().inner
                        {
                            raw
                        } else {
                            panic!("not a texture!")
                        }
                    });
                gl.framebuffer_texture_2d(
                    glow::READ_FRAMEBUFFER,
                    glow::COLOR_ATTACHMENT0,
                    glow::TEXTURE_2D,
                    Some(texture_id),
                    0,
                );
                assert_eq!(
                    gl.check_framebuffer_status(glow::READ_FRAMEBUFFER),
                    glow::FRAMEBUFFER_COMPLETE,
                    "GL read framebuffer has error status"
                );

                gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(render_ctx.openxr_fb));
                gl.framebuffer_texture_2d(
                    glow::DRAW_FRAMEBUFFER,
                    glow::COLOR_ATTACHMENT0,
                    glow::TEXTURE_2D,
                    Some(glow::NativeTexture(
                        std::num::NonZero::new(LAYER.egl_image).unwrap(),
                    )),
                    0,
                );
                assert_eq!(
                    gl.check_framebuffer_status(glow::DRAW_FRAMEBUFFER),
                    glow::FRAMEBUFFER_COMPLETE,
                    "GL draw framebuffer has error status"
                );

                gl.disable(glow::SCISSOR_TEST);
                gl.blit_framebuffer(
                    0,
                    0,
                    UI_WINDOW_W as i32,
                    UI_WINDOW_H as i32,
                    0,
                    UI_WINDOW_H as i32,
                    UI_WINDOW_W as i32,
                    0,
                    glow::COLOR_BUFFER_BIT,
                    glow::NEAREST,
                );
                assert_eq!(gl.get_error(), glow::NO_ERROR, "GL framebuffer blit failed");

                gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            }

            // Unbind the context.
            unsafe {
                eglMakeCurrent(
                    egl_pointers.display,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                );
            }

            // Done rendering, signal the calling thread back.
            *ready = false;
            render_signal.condvar.notify_one();
        }
    })
}
