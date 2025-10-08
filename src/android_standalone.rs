use std::sync::Arc;

use crate::android_serial_watcher::start_serial_watcher;
use crate::camera_dispatcher::{
    CameraDispatcher, MonoCameraDispatcher, MonoEyeCameraDispatcher, StereoEyesCameraDispatcher,
};
use crate::camera_manager;
use crate::camera_server::start_camera_server;
use crate::frame_server::start_frame_server;

#[cfg(feature = "inference")]
use crate::data_processing::process_gaze;
#[cfg(feature = "inference")]
use crate::inference::eye_inference;
#[cfg(feature = "inference")]
use crate::osc_sender::start_osc_sender;

use crate::structs::Eye;
#[cfg(feature = "gui")]
use crate::window_desktop::start_ui;

use futures::future::try_join_all;
use log::info;
use tokio::task::JoinHandle;
use winit::platform::android::activity::AndroidApp;

use crate::app::App;

#[allow(dead_code)]
#[unsafe(no_mangle)]
fn android_main(android_app: AndroidApp) {
    crate::logging::setup_logging();
    info!("Hello from Android main!");

    let app = Arc::new(App::new());
    let app_clone = app.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            info!("Hello from Tokio runtime!");

            try_join_all(start_android_standalone_tasks(&app_clone))
                .await
                .unwrap()
        });
    });

    info!("Started Tokio runtime thread");
    info!("Starting GUI on main thread");

    start_ui(
        android_app,
        crate::ui::AppRendererContext {
            eyes_cam_rx: app.eyes_cam_rx.activate_cloned(),
            f_rx: app.f_cam_rx.activate_cloned(),
            raw_eyes_rx: app.raw_eyes_rx.activate_cloned(),
            combined_eyes_rx: app.combined_eyes_rx.activate_cloned(),
        },
    )
}

fn start_android_standalone_tasks(app: &App) -> Vec<JoinHandle<()>> {
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

    // Inference, process the data, output OSC

    #[cfg(feature = "inference")]
    {
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

        // OSC sender

        tasks.push(start_osc_sender(
            app.combined_eyes_rx.activate_cloned(),
            "localhost:9000".to_string(),
        ));
    }

    // HTTP server to mirror cameras
    // let camera_server = start_camera_server(l_cam_rx.clone(), f_cam_rx.clone());
    // tasks.push(camera_server);

    tasks
}
