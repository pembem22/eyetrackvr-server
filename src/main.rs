use clap::Parser;
use tokio::join;

mod app;
mod camera;
mod camera_texture;
mod inference;
mod ui;

use crate::{app::App, camera::*};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Left camera URL
    #[arg(short = 'l')]
    l_camera_url: String,

    /// Right camera URL
    #[arg(short = 'r')]
    r_camera_url: String,

    /// Enable inference
    #[arg(short = 'I')]
    inference: bool,

    /// OSC output address
    #[arg(short = 'o')]
    osc_out_address: String,

    /// Path to the ONNX model
    #[arg(short = 'm', default_value = "./model.onnx")]
    model_path: String,
}

#[tokio::main]
async fn main() -> tokio_serial::Result<()> {
    let args = Args::parse();

    let mut app = App::new();

    let mut tasks = Vec::new();

    let (l_camera, r_camera) = app.start_cameras(args.l_camera_url, args.r_camera_url)?;
    let ui = app.start_ui();
    let server = app.start_server();

    tasks.push(l_camera);
    tasks.push(r_camera);
    tasks.push(ui);
    tasks.push(server);

    if args.inference {
        let inference = app.start_inference(args.osc_out_address, args.model_path);
        tasks.push(inference);
    }

    // TODO: this can be done better
    for task in tasks {
        join!(task).0.unwrap();
    }

    Ok(())
}
