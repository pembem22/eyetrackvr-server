use core::ffi::c_void;
use std::{
    ffi::{CString, c_char},
    num::NonZeroU32,
    ptr,
    time::{Instant, SystemTime},
};

// use imgui::FontSource;
// use imgui::MouseCursor;
// use imgui_wgpu::Renderer;

// use wgpu::{GlBackendOptions, GlFenceBehavior, Gles3MinorVersion};

use glow::{HasContext, NativeTexture};
use pollster::block_on;
// use wgpu::{
//     FeaturesWGPU,
//     hal::Adapter,
//     wgc::{api::Gles, hal_api::HalApi},
// };

use crate::{
    app::App,
    openxr_layer::layer::{self, LAYER},
    // ui::{AppRenderer, AppRendererContext},
};

// struct ImguiState {
//     context: imgui::Context,
//     // platform: WinitPlatform,
//     renderer: imgui_wgpu::Renderer,
//     clear_color: wgpu::Color,
//     last_frame: Instant,
//     last_cursor: Option<imgui::MouseCursor>,
// }

// impl ImguiState {
//     fn setup_imgui(&mut self) -> Self {
//         let mut context = imgui::Context::create();
//         let mut platform = imgui_winit_support::WinitPlatform::new(&mut context);
//         platform.attach_window(
//             context.io_mut(),
//             &self.window,
//             imgui_winit_support::HiDpiMode::Default,
//         );
//         context.set_ini_filename(None);

//         let font_size = (13.0 * self.hidpi_factor) as f32;
//         context.io_mut().font_global_scale = (1.0 / self.hidpi_factor) as f32;

//         context.fonts().add_font(&[FontSource::DefaultFontData {
//             config: Some(imgui::FontConfig {
//                 oversample_h: 1,
//                 pixel_snap_h: true,
//                 size_pixels: font_size,
//                 ..Default::default()
//             }),
//         }]);

//         //
//         // Set up dear imgui wgpu renderer
//         //
//         let clear_color = wgpu::Color {
//             r: 0.1,
//             g: 0.2,
//             b: 0.3,
//             a: 1.0,
//         };

//         let renderer_config = RendererConfig {
//             texture_format: self.surface_desc.format,
//             ..Default::default()
//         };

//         let renderer =
//             imgui_wgpu::Renderer::new(&mut context, &self.device, &self.queue, renderer_config);
//         let last_frame = Instant::now();
//         let last_cursor = None;

//         Self {
//             context,
//             // platform,
//             renderer,
//             clear_color,
//             last_frame,
//             last_cursor,
//         }
//     }
// }

#[link(name = "EGL")]
unsafe extern "C" {
    unsafe fn eglMakeCurrent(
        display: *mut c_void,
        draw: *mut c_void,
        read: *mut c_void,
        context: *mut c_void,
    ) -> u8;
    unsafe fn eglGetProcAddress(name: *const c_char) -> *const c_void;
}

struct RenderContext {
    // device: wgpu::Device,
    // queue: wgpu::Queue,
    // texture: wgpu::hal::gles::Texture,
    // surface: wgpu::hal::gles::Surface, // or custom FBO target if rendering into OpenXR swapchain
    // adapter: wgpu::hal::ExposedAdapter<Gles>,
    // imgui: Option<ImguiState>,
    gl: glow::Context,
    framebuffer: glow::Framebuffer,
}

impl RenderContext {
    unsafe fn init_on_current_context() -> RenderContext {
        // use wgpu::{GlBackendOptions, GlFenceBehavior, Gles3MinorVersion};

        // let mut get_proc = |name: &str| {
        //     let cname = std::ffi::CString::new(name).unwrap();
        //     unsafe { eglGetProcAddress(cname.as_ptr()) as *const _ }
        // };

        // let options = GlBackendOptions {
        //     gles_minor_version: Gles3MinorVersion::Automatic,
        //     fence_behavior: GlFenceBehavior::Normal,
        // };

        // let adapter =
        //     unsafe { wgpu::hal::gles::Adapter::new_external(&mut get_proc, options).unwrap() };

        // let (device, queue) = unsafe {
        //     adapter.
        // }?;

        // let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        //     backends: wgpu::Backends::GL,
        //     ..Default::default()
        // });

        // let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        //     power_preference: wgpu::PowerPreference::HighPerformance,
        //     compatible_surface: None,
        //     force_fallback_adapter: false,
        // }))
        // .unwrap();

        // let (device, queue) =
        //     block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

        // RenderContext {
        //     device,
        //     queue,
        //     // surface: /* optional or None if you're rendering directly to OpenXR FBO */,
        //     // adapter,
        // }

        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|cstr| {
                eglGetProcAddress(cstr.as_ptr()) as *const _
            })
        };
        let framebuffer = unsafe { gl.create_framebuffer().unwrap() };

        RenderContext { gl, framebuffer }
    }

    // fn setup_imgui(&mut self) {
    //     let mut context = imgui::Context::create();
    //     context.set_ini_filename(None);

    //     let hidpi_factor = 96.0;

    //     let font_size = (13.0 * hidpi_factor) as f32;
    //     context.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

    //     context.fonts().add_font(&[FontSource::DefaultFontData {
    //         config: Some(imgui::FontConfig {
    //             oversample_h: 1,
    //             pixel_snap_h: true,
    //             size_pixels: font_size,
    //             ..Default::default()
    //         }),
    //     }]);

    //     //
    //     // Set up dear imgui wgpu renderer
    //     //
    //     let clear_color = wgpu::Color {
    //         r: 0.1,
    //         g: 0.2,
    //         b: 0.3,
    //         a: 1.0,
    //     };

    //     let renderer_config = RendererConfig {
    //         texture_format: self.surface_desc.format,
    //         ..Default::default()
    //     };

    //     let renderer = Renderer::new(&mut context, &self.device, &self.queue, renderer_config);
    //     let last_frame = Instant::now();
    //     let last_cursor = None;

    //     self.imgui = Some(ImguiState {
    //         context,
    //         platform,
    //         renderer,
    //         clear_color,
    //         last_frame,
    //         last_cursor,
    //     })
    // }

    // fn
}

pub fn start_ui(app: &App) -> tokio::task::JoinHandle<()> {
    // FIXME: Apparently this is undefined behavior, figure this out.
    #![warn(static_mut_refs)]
    tokio::task::spawn_blocking(|| {
        println!("Hello from the render thread");
        let render_signal = unsafe { &mut LAYER.render_signal };
        let egl_pointers = unsafe { LAYER.egl_pointers.as_ref().unwrap() };

        let start_time = SystemTime::now();

        let mut maybe_render_ctx = None;

        loop {
            // Wait for the render thread to be woken up.
            let mut ready = render_signal.mutex.lock().unwrap();
            while !*ready {
                ready = render_signal.condvar.wait(ready).unwrap();
            }

            // Make sure the context is on the current thread.
            unsafe {
                println!("{egl_pointers:?}");
                eglMakeCurrent(
                    egl_pointers.display,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    egl_pointers.context,
                );
            }

            let render_context =
                maybe_render_ctx.get_or_insert(unsafe { RenderContext::init_on_current_context() });
            let gl = &mut render_context.gl;

            unsafe {
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(render_context.framebuffer));
                gl.framebuffer_texture_2d(
                    glow::FRAMEBUFFER,
                    glow::COLOR_ATTACHMENT0,
                    glow::TEXTURE_2D,
                    Some(glow::NativeTexture(
                        std::num::NonZero::new(LAYER.egl_image).unwrap(),
                    )),
                    0,
                );
                assert_eq!(
                    gl.check_framebuffer_status(glow::FRAMEBUFFER),
                    glow::FRAMEBUFFER_COMPLETE
                );

                let d = start_time.elapsed().unwrap().as_secs_f32();
                gl.clear_color(
                    d.sin().mul_add(0.5, 0.5),
                    (d * 0.75).cos().mul_add(0.5, 0.5),
                    (d * 0.25).sin().mul_add(0.5, 0.5),
                    1.0,
                );
                gl.clear(glow::COLOR_BUFFER_BIT);

                gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            }

            // render_context.device.texture_from_raw(name, desc, drop_callback)

            // wgpu::hal::gles::Texture

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

    // AppWindow::new(renderer_context)
    // let adapter = unsafe {
    //     wgpu::hal::gles::Adapter::new_external(
    //         gl_loader,
    //         GlBackendOptions {
    //             gles_minor_version: Gles3MinorVersion::Automatic,
    //             fence_behavior: GlFenceBehavior::Normal,
    //         },
    //     )
    //     .unwrap()
    // };

    // use wgpu::hal;
    // wgpu::hal::gles::Adapter::new_external(fun, options)
    // // let instance = <hal::api::Gles as hal::Api>::Instance::init(/* feature toggles */)?;

    // // glow::Context::from_loader_function_cstr(loader_function)
    // let raw_display_ptr = ptr::null();
    // let raw_config_ptr = ptr::null();
    // let raw_display = glutin::display::RawDisplay::Egl(raw_display_ptr);
    // let display = unsafe {
    //     glutin::api::egl::display::Display::new(raw_display).unwrap()
    // };

    // display.get_proc_address(addr)

    // let config = glutin::config::Config::Egl(glutin::api::egl::config::Config);

    // println!("eglMakeCurrent {:?}", eglMakeCurrent as *const ())
}
