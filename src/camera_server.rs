use std::io::Cursor;

use async_broadcast::{InactiveReceiver, Receiver};
use futures::{FutureExt, Stream, StreamExt};
use hyper::http;
use hyper::{
    Body, HeaderMap, Request, Response,
    body::Bytes,
    service::{make_service_fn, service_fn},
};
use image::codecs::jpeg::JpegEncoder;
use tokio::task::JoinHandle;

use crate::camera::Frame;
use crate::structs::{EyesFrame, EyesFrameType};

const PART_BOUNDARY: &str = "123456789000000000000987654321";

// So much jank...
fn serve<const L: bool, const R: bool>(
    _req: Request<Body>,
    frame_stream: impl futures::Stream<Item = EyesFrame> + Send + 'static,
) -> Result<Response<Body>, http::Error> {
    let stream = frame_stream.filter_map(async |mut frame| {
        let body = Bytes::from(match (L, R) {
            (false, false) => frame.frame.as_jpeg_bytes(),
            (true, false) => {
                let Some(view) = frame.get_left_view() else {
                    return None;
                };
                let vec = Vec::with_capacity(8192);
                let mut cursor = Cursor::new(vec);

                JpegEncoder::new(&mut cursor)
                    .encode_image(&view.to_image())
                    .unwrap();
                cursor.into_inner()
            }
            (false, true) => {
                let Some(view) = frame.get_right_view() else {
                    return None;
                };
                let vec = Vec::with_capacity(8192);
                let mut cursor = Cursor::new(vec);

                JpegEncoder::new(&mut cursor)
                    .encode_image(&view.to_image())
                    .unwrap();
                cursor.into_inner()
            }
            _ => unreachable!("couldn't be both left and right"),
        });

        let mut headers = HeaderMap::new();
        headers.append(http::header::CONTENT_TYPE, "image/jpeg".parse().unwrap());
        // TODO: OpenIris also puts `X-Timestamp` headers, see if useful.

        let part = multipart_stream::Part { headers, body };
        Some(Ok::<_, std::convert::Infallible>(part))
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
    lr_rx: InactiveReceiver<EyesFrame>,
    f_rx: InactiveReceiver<Frame>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let addr = ([0, 0, 0, 0], 8881).into();
        let make_svc = make_service_fn(move |_conn| {
            // Ugh...
            let lr_rx = lr_rx.clone();
            let f_rx = f_rx.clone();
            futures::future::ok::<_, std::convert::Infallible>(service_fn(move |req| {
                let lr_rx = lr_rx.clone();
                let f_rx = f_rx.clone();
                async move {
                    match req.uri().path() {
                        // TODO: fix this mess...
                        "/L" => serve::<true, false>(req, lr_rx.activate()),
                        "/R" => serve::<false, true>(req, lr_rx.activate()),
                        "/F" => serve::<false, false>(
                            req,
                            f_rx.activate().map(|f| EyesFrame {
                                frame_type: EyesFrameType::Left,
                                frame: f,
                            }),
                        ),
                        _ => hyper::Response::builder()
                            .status(404)
                            .body(hyper::Body::empty()),
                    }
                }
            }))
        });
        let server = hyper::Server::bind(&addr).serve(make_svc);
        println!("Serving on http://{}", server.local_addr());
        server.await.unwrap();
    })
}
