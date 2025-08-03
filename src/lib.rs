mod app;
mod camera;
mod camera_server;
mod frame_server;
mod structs;

#[cfg(feature = "gui")]
mod camera_texture;
#[cfg(feature = "gui")]
mod ui;
#[cfg(all(feature = "gui", target_os = "windows"))]
mod window_desktop;

mod window_android;

#[cfg(feature = "inference")]
mod data_processing;
#[cfg(feature = "inference")]
mod inference;
#[cfg(feature = "inference")]
mod osc_sender;

#[cfg(feature = "desktop")]
pub mod desktop;

#[cfg(feature = "android")]
mod android;
#[cfg(feature = "openxr-api-layer")]
mod openxr_layer;

#[cfg(all(feature = "openxr-api-layer", feature = "inference"))]
mod openxr_output;
