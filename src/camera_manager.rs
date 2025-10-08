use crate::camera_sources::{CameraSource, HttpCameraSource};

#[cfg(not(target_os = "android"))]
use crate::camera_sources::UvcCameraSource;

pub fn camera_source_from_uri(uri: String) -> Option<Box<dyn CameraSource>> {
    if uri.starts_with("COM") {
        todo!();
        // self.connect_serial(uri)
    }

    #[cfg(not(target_os = "android"))]
    if uri.starts_with("uvc://") {
        let uvc_index = uri.strip_prefix("uvc://").unwrap().parse().unwrap();
        Some(Box::new(UvcCameraSource::new(uvc_index)))
    }

    if uri.starts_with("http://") {
        return Some(Box::new(HttpCameraSource::new(uri)));
    }

    None
}
