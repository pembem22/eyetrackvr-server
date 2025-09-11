use crate::camera_dispatcher::{MonoCameraDispatcher, StereoCameraDispatcher};
use crate::camera_manager;
use crate::frame_server::start_frame_server;

#[cfg(feature = "inference")]
use crate::data_processing::{filter_eye, merge_eyes};
#[cfg(feature = "inference")]
use crate::inference::eye_inference;
#[cfg(feature = "inference")]
use crate::osc_sender::start_osc_sender;

#[cfg(feature = "gui")]
use crate::window_desktop::start_ui;

use clap::Parser;
use futures::future::try_join_all;
use tokio::task::JoinHandle;

use crate::{app::App, camera::*};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Left camera URL
    #[arg(short = 'l')]
    l_camera_url: Option<String>,

    /// Right camera URL
    #[arg(short = 'r')]
    r_camera_url: Option<String>,

    /// Face camera URL
    #[arg(short = 'f')]
    f_camera_url: Option<String>,

    /// Combined left and right eyes camera URL
    #[arg(long = "lr")]
    lr_camera_url: Option<String>,

    /// Enable inference
    #[arg(short = 'I')]
    inference: bool,

    /// OSC output address
    #[arg(short = 'o', default_value = "localhost:9000")]
    osc_out_address: String,

    /// Path to the ONNX model
    #[arg(short = 'm', default_value = "./model.onnx")]
    model_path: String,

    /// Number of threads to use for inference per eye
    #[arg(short = 't', default_value_t = 1)]
    threads_per_eye: usize,

    /// Headless mode, no GUI
    #[arg(short = 'H')]
    headless: bool,
}

pub async fn desktop_main() {
    nokhwa::query(nokhwa::utils::ApiBackend::Auto)
        .unwrap()
        .iter()
        .for_each(|cam| println!("{:#?}", cam));

    let args = Args::parse();

    let app = App::new();

    let tasks = start_desktop_tasks(&args, &app);

    let _ = try_join_all(tasks).await.unwrap();
}

fn start_desktop_tasks(args: &Args, app: &App) -> Vec<JoinHandle<()>> {
    let mut tasks = Vec::new();

    // Connect to the cameras
    if let Some(lr_camera_url) = &args.lr_camera_url {
        if args.l_camera_url.is_some() || args.r_camera_url.is_some() {
            println!(
                "Both combined (--lr) and at least one individual (-l or -r) cameras defined, this is not supported."
            );
            std::process::exit(1);
        }

        // I have no idea what I'm doing here.

        let camera_source = camera_manager::camera_source_from_uri(lr_camera_url.to_string());
        match camera_source {
            Some(camera_source) => tasks.push(camera_source.run(Box::new(
                StereoCameraDispatcher::new(app.l_cam_tx.clone(), app.r_cam_tx.clone()),
            ))),
            None => eprintln!("Invalid camera URI {lr_camera_url}"),
        }
    }

    // TODO: Deduplicate

    if let Some(l_camera_url) = &args.l_camera_url {
        let camera_source = camera_manager::camera_source_from_uri(l_camera_url.to_string());
        match camera_source {
            Some(camera_source) => tasks
                .push(camera_source.run(Box::new(MonoCameraDispatcher::new(app.l_cam_tx.clone())))),
            None => eprintln!("Invalid camera URI {l_camera_url}"),
        }
    }

    if let Some(r_camera_url) = &args.r_camera_url {
        let camera_source = camera_manager::camera_source_from_uri(r_camera_url.to_string());
        match camera_source {
            Some(camera_source) => tasks
                .push(camera_source.run(Box::new(MonoCameraDispatcher::new(app.r_cam_tx.clone())))),
            None => eprintln!("Invalid camera URI {r_camera_url}"),
        }
    }

    if let Some(f_camera_url) = &args.f_camera_url {
        let camera_source = camera_manager::camera_source_from_uri(f_camera_url.to_string());
        match camera_source {
            Some(camera_source) => tasks
                .push(camera_source.run(Box::new(MonoCameraDispatcher::new(app.f_cam_tx.clone())))),
            None => eprintln!("Invalid camera URI {f_camera_url}"),
        }
    }

    // Save dataset

    tasks.push(start_frame_server(
        app.l_cam_rx.activate_cloned(),
        app.r_cam_rx.activate_cloned(),
    ));

    // Inference, process the data, output OSC

    if args.inference {
        #[cfg(feature = "inference")]
        {
            tasks.push(eye_inference(
                app.l_cam_rx.activate_cloned(),
                &args.model_path,
                args.threads_per_eye,
                app.l_raw_eye_tx.clone(),
                Eye::L,
            ));
            tasks.push(eye_inference(
                app.r_cam_rx.activate_cloned(),
                &args.model_path,
                args.threads_per_eye,
                app.r_raw_eye_tx.clone(),
                Eye::R,
            ));

            // Filter

            tasks.push(filter_eye(
                app.l_raw_eye_rx.activate_cloned(),
                app.l_filtered_eye_tx.clone(),
            ));
            tasks.push(filter_eye(
                app.r_raw_eye_rx.activate_cloned(),
                app.r_filtered_eye_tx.clone(),
            ));

            // Merge

            tasks.push(merge_eyes(
                app.l_filtered_eye_rx.activate_cloned(),
                app.r_filtered_eye_rx.activate_cloned(),
                app.filtered_eyes_tx.clone(),
            ));

            // OSC sender

            tasks.push(start_osc_sender(
                app.filtered_eyes_rx.activate_cloned(),
                args.osc_out_address.clone(),
            ));
        }

        #[cfg(not(feature = "inference"))]
        println!("Compiled without inference support, ignoring")
    }

    // GUI

    if !args.headless {
        #[cfg(feature = "gui")]
        {
            tasks.push(start_ui(crate::ui::AppRendererContext {
                l_rx: app.l_cam_rx.activate_cloned(),
                r_rx: app.r_cam_rx.activate_cloned(),
                f_rx: app.f_cam_rx.activate_cloned(),
                l_raw_rx: app.l_raw_eye_rx.activate_cloned(),
                r_raw_rx: app.r_raw_eye_rx.activate_cloned(),
                filtered_eyes_rx: app.filtered_eyes_rx.activate_cloned(),
            }));
        }

        #[cfg(not(feature = "gui"))]
        println!("Compiled without GUI support, starting headless anyway")
    }

    // HTTP server to mirror cameras
    // let camera_server = start_camera_server(l_cam_rx.clone(), f_cam_rx.clone());
    // tasks.push(camera_server);

    tasks
}
