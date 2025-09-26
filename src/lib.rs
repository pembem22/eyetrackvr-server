mod app;
mod camera;

mod camera_dispatcher;
mod camera_server;
mod camera_sources;
mod frame_server;
mod structs;

#[cfg(feature = "desktop")]
mod camera_manager;
#[cfg(feature = "gui")]
mod camera_texture;
#[cfg(feature = "gui")]
mod ui;
#[cfg(all(feature = "gui", feature = "desktop"))]
mod window_desktop;

#[cfg(all(feature = "gui", feature = "android"))]
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
#[cfg(feature = "android")]
mod android_serial_watcher;
#[cfg(feature = "openxr-api-layer")]
mod openxr_layer;

#[cfg(all(feature = "openxr-api-layer", feature = "inference"))]
mod openxr_output;
