use crate::android_serial_watcher::start_serial_watcher;
use crate::camera_dispatcher::{CameraDispatcher, MonoCameraDispatcher, MonoEyeCameraDispatcher};
use crate::structs::Eye;
use crate::{app::App, camera_server::start_camera_server};
use futures::future::try_join_all;
use log::{LevelFilter, info};
use tokio::task::JoinHandle;

pub fn init_logger() {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(LevelFilter::Trace) // limit log level
            .with_tag("RUST_ETFT") // logs will show under mytag tag
            .with_filter(
                android_logger::FilterBuilder::new()
                    .parse("info,eyetrackvr_server=trace")
                    .build(),
            ),
    );

    info!("Initalized android_logger");
}

pub fn main() {
    info!("Hello from Android main!");

    std::thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            info!("Hello from Tokio runtime!");

            let app = App::new();

            try_join_all(start_android_tasks(&app)).await.unwrap()
        });
    });

    info!("Started Tokio runtime thread");
}

fn start_android_tasks(app: &App) -> Vec<JoinHandle<()>> {
    let mut tasks = Vec::new();

    // HTTP server to mirror the face camera
    tasks.push(start_camera_server(
        app.eyes_cam_rx.clone(),
        app.f_cam_rx.clone(),
    ));

    tasks.push(start_serial_watcher(std::collections::HashMap::from([
        (
            "30:30:F9:33:DD:7C".to_string(),
            Box::new(MonoEyeCameraDispatcher::new(Eye::L, app.eye_cam_tx.clone()))
                as Box<dyn CameraDispatcher>,
        ),
        (
            "30:30:F9:17:F3:C4".to_string(),
            Box::new(MonoEyeCameraDispatcher::new(Eye::R, app.eye_cam_tx.clone())),
        ),
        (
            "DC:DA:0C:18:32:34".to_string(),
            Box::new(MonoCameraDispatcher::new(app.f_cam_tx.clone())),
        ),
    ])));

    tasks.push(start_ui(crate::ui::AppRendererContext {
        eyes_cam_rx: app.eyes_cam_rx.activate_cloned(),
        f_rx: app.f_cam_rx.activate_cloned(),
        raw_eyes_rx: app.raw_eyes_rx.activate_cloned(),
        combined_eyes_rx: app.combined_eyes_rx.activate_cloned(),
    }));

    // Inference, process the data, output

    #[cfg(feature = "inference")]
    {
        use crate::data_processing::process_gaze;
        use crate::inference::eye_inference;
        use crate::openxr_output::start_openxr_output;

        const THREADS_PER_EYE: usize = 1;

        tasks.push(eye_inference(
            app.eyes_cam_rx.activate_cloned(),
            app.raw_eyes_tx.clone(),
            THREADS_PER_EYE,
        ));

        // Filter

        tasks.push(process_gaze(
            app.raw_eyes_rx.activate_cloned(),
            app.combined_eyes_tx.clone(),
        ));

        // OpenXR output

        start_openxr_output(&app.combined_eyes_rx);
    }

    tasks
}
