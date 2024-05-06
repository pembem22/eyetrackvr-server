use std::net::UdpSocket;
use std::sync::{Arc, Mutex};

use async_broadcast::{Receiver, Sender};
use tokio::task::JoinHandle;

use crate::inference::start_onnx;
use crate::Frame;
use crate::{camera_texture::CameraTexture, ui, Camera, Eye};

pub(crate) struct App {
    l_camera: Camera,
    r_camera: Camera,
}

impl App {
    pub fn new(l_sender: Sender<Frame>, r_sender: Sender<Frame>) -> App {
        let l_camera = Camera::new(Eye::L, l_sender);
        let r_camera = Camera::new(Eye::R, r_sender);

        App { l_camera, r_camera }
    }

    pub fn start_cameras(
        &mut self,
        l_tty_path: String,
        r_tty_path: String,
    ) -> tokio_serial::Result<(JoinHandle<()>, JoinHandle<()>)> {
        Ok((
            self.l_camera.start(l_tty_path)?,
            self.r_camera.start(r_tty_path)?,
        ))
    }

    pub fn start_ui(&mut self, l_rx: Receiver<Frame>, r_rx: Receiver<Frame>) -> JoinHandle<()> {
        tokio::task::spawn_blocking(|| {
            let mut ui = ui::UI::new();

            let l_texture = CameraTexture::new(&mut ui);
            let r_texture = CameraTexture::new(&mut ui);

            let l_rx = Arc::new(Mutex::new(l_rx));
            let r_rx = Arc::new(Mutex::new(r_rx));

            ui.run(move |imgui, queue, renderer| {
                let mut l_rx = l_rx.lock().unwrap();
                let mut r_rx = r_rx.lock().unwrap();

                l_texture.update_texture(&mut l_rx, queue, renderer);
                r_texture.update_texture(&mut r_rx, queue, renderer);

                imgui.window("Hello!").build(move || {
                    l_texture.build(imgui);
                    imgui.same_line();
                    r_texture.build(imgui);
                });
            });
        })
    }

    pub fn start_inference(
        &mut self,
        osc_out_address: String,
        model_path: String,
        threads_per_eye: usize,
        l_rx: Receiver<Frame>,
        r_rx: Receiver<Frame>,
    ) -> JoinHandle<()> {
        let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
        sock.connect(osc_out_address).unwrap();
        println!("OSC connected");

        start_onnx(l_rx, r_rx, sock, model_path, threads_per_eye).unwrap()
    }
}
