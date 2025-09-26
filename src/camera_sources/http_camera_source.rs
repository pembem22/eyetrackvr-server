use hyper::http;
use std::{
    io::Cursor,
    time::{Duration, SystemTime},
};
use tokio_stream::StreamExt;

use crate::{camera::Frame, camera_dispatcher::CameraDispatcher, camera_sources::CameraSource};

const HTTP_CONNECTION_TIMEOUT: Duration = Duration::from_secs(1);
#[derive(Clone, Debug)]
pub struct HttpCameraSource {
    url: String,
}

impl HttpCameraSource {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

impl CameraSource for HttpCameraSource {
    fn run(&self, dispatcher: Box<dyn CameraDispatcher>) -> tokio::task::JoinHandle<()> {
        let url = self.url.clone();
        // let dispatcher = dispatcher.clone();

        let future = async move {
            let mut reconnect = false;

            'connect_loop: loop {
                if reconnect {
                    println!("Reconnecting in a sec to {url}");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                reconnect = true;

                let client = hyper::Client::builder()
                    .pool_idle_timeout(HTTP_CONNECTION_TIMEOUT)
                    .build_http::<hyper::Body>();
                let result = client.get(http::Uri::try_from(url.clone()).unwrap()).await;
                if let Err(err) = result {
                    println!("{err:?}");
                    continue 'connect_loop;
                }

                let res = result.unwrap();

                if !res.status().is_success() {
                    println!("HTTP request failed with status {}", res.status());
                    continue 'connect_loop;
                }
                let content_type: mime::Mime = res
                    .headers()
                    .get(http::header::CONTENT_TYPE)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .parse()
                    .unwrap();
                assert_eq!(content_type.type_(), "multipart");
                let boundary = content_type.get_param(mime::BOUNDARY).unwrap();
                let stream = res.into_body();
                let mut stream = multipart_stream::parse(stream, boundary.as_str());
                while let Some(p) = stream.next().await {
                    let p = match p {
                        Ok(p) => p,
                        Err(err) => {
                            println!("Camera stream error:\n{err:?}");
                            continue 'connect_loop;
                        }
                    };
                    let buf = p.body;

                    let mut decoder = image::ImageReader::new(Cursor::new(buf.clone()));
                    decoder.set_format(image::ImageFormat::Jpeg);

                    let image = decoder.decode();

                    if image.is_err() {
                        println!("Failed to decode image");
                        continue;
                    }

                    let frame = Frame {
                        timestamp: SystemTime::now(),
                        raw_jpeg_data: Some(buf.to_vec()),
                        decoded: image.unwrap().into_rgb8(),
                    };

                    dispatcher.dispatch(frame).await;
                }
            }
        };
        tokio::spawn(future)
    }
}
