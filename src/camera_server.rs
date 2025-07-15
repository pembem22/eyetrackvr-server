use async_broadcast::{InactiveReceiver, Receiver};
use hyper::http;
use hyper::{
    Body, HeaderMap, Request, Response,
    body::Bytes,
    service::{make_service_fn, service_fn},
};
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;

use crate::camera::Frame;

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

const PART_BOUNDARY: &str = "123456789000000000000987654321";

fn serve(
    _req: Request<Body>,
    frame_stream: Receiver<Frame>,
) -> Result<Response<Body>, http::Error> {
    let stream = frame_stream.map(|frame| {
        let body = Bytes::from(frame.raw_data);

        let mut headers = HeaderMap::new();
        headers.append(http::header::CONTENT_TYPE, "image/jpeg".parse().unwrap());
        // TODO: OpenIris also puts `X-Timestamp` headers, see if useful.

        let part = multipart_stream::Part { headers, body };
        Ok::<_, std::convert::Infallible>(part)
    });
    let stream = multipart_stream::serialize(stream, PART_BOUNDARY);

    hyper::Response::builder()
        .header(
            http::header::CONTENT_TYPE,
            "multipart/x-mixed-replace;boundary=".to_owned() + PART_BOUNDARY,
        )
        .body(hyper::Body::wrap_stream(stream))
}

pub fn start_camera_server(
    l_rx: InactiveReceiver<Frame>,
    r_rx: InactiveReceiver<Frame>,
    f_rx: InactiveReceiver<Frame>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let addr = ([0, 0, 0, 0], 8881).into();
        let make_svc = make_service_fn(move |_conn| {
            // Ugh...
            let r_rx = r_rx.clone();
            let l_rx = l_rx.clone();
            let f_rx = f_rx.clone();
            futures::future::ok::<_, std::convert::Infallible>(service_fn(move |req| {
                let l_rx = l_rx.clone();
                let r_rx = r_rx.clone();
                let f_rx = f_rx.clone();
                async move {
                    let camera_rx = match req.uri().path() {
                        "/L" => l_rx,
                        "/R" => r_rx,
                        "/F" => f_rx,
                        _ => {
                            return hyper::Response::builder()
                                .status(404)
                                .body(hyper::Body::empty());
                        }
                    };
                    serve(req, camera_rx.activate_cloned())
                }
            }))
        });
        let server = hyper::Server::bind(&addr).serve(make_svc);
        println!("Serving on http://{}", server.local_addr());
        server.await.unwrap();
    })
}
