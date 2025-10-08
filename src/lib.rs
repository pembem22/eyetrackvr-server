mod app;
mod camera;

mod camera_dispatcher;
mod camera_manager;
mod camera_server;
mod camera_sources;
mod frame_server;
mod logging;
mod structs;

#[cfg(feature = "gui")]
mod camera_texture;
#[cfg(feature = "gui")]
mod ui;
#[cfg(all(
    feature = "gui",
    any(feature = "desktop", feature = "android-standalone")
))]
mod window_desktop;

#[cfg(all(target_os = "android", feature = "gui", feature = "openxr-api-layer"))]
mod window_android;

#[cfg(feature = "inference")]
mod data_processing;
#[cfg(feature = "inference")]
mod inference;
#[cfg(feature = "inference")]
mod osc_sender;

#[cfg(feature = "desktop")]
pub mod desktop;

#[cfg(target_os = "android")]
mod android_serial_watcher;

#[cfg(all(target_os = "android", feature = "openxr-api-layer"))]
mod android_openxr_layer;
#[cfg(feature = "android-standalone")]
mod android_standalone;

#[cfg(feature = "openxr-api-layer")]
mod openxr_layer;
#[cfg(feature = "openxr-api-layer")]
mod openxr_output;
