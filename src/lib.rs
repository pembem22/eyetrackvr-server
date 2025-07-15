mod app;
mod camera;
mod camera_server;
mod frame_server;
mod structs;

#[cfg(feature = "gui")]
mod camera_texture;
#[cfg(feature = "gui")]
mod ui;
#[cfg(feature = "gui")]
mod window;

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