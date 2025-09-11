use crate::camera_sources::{CameraSource, HttpCameraSource, UvcCameraSource};

pub fn camera_source_from_uri(uri: String) -> Option<Box<dyn CameraSource>> {
    if uri.starts_with("COM") {
        todo!();
        // self.connect_serial(uri)
    } else if uri.starts_with("uvc://") {
        let uvc_index = uri.strip_prefix("uvc://").unwrap().parse().unwrap();
        Some(Box::new(UvcCameraSource::new(uvc_index)))
    } else if uri.starts_with("http://") {
        Some(Box::new(HttpCameraSource::new(uri)))
    } else {
        None
    }
}
