use async_broadcast::Receiver;
use hyper::http;
use hyper::{
    body::Bytes,
    service::{make_service_fn, service_fn},
    Body, HeaderMap, Request, Response,
};
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;

use crate::Frame;

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

const PART_BOUNDARY: &str = "123456789000000000000987654321";

async fn serve(
    _req: Request<Body>,
    frame_stream: Receiver<Frame>,
) -> Result<Response<Body>, BoxedError> {
    let stream = frame_stream.map(|frame| {
        let body = Bytes::from(frame.raw_data);

        let mut headers = HeaderMap::new();
        headers.append(http::header::CONTENT_TYPE, "image/jpeg".parse().unwrap());
        // TODO: OpenIris also puts `X-Timestamp` headers, see if useful.

        let part = multipart_stream::Part { headers, body };
        Ok::<_, std::convert::Infallible>(part)
    });
    let stream = multipart_stream::serialize(stream, PART_BOUNDARY);

    Ok(hyper::Response::builder()
        .header(
            http::header::CONTENT_TYPE,
            "multipart/x-mixed-replace;boundary=".to_owned() + PART_BOUNDARY,
        )
        .body(hyper::Body::wrap_stream(stream))?)
}

pub fn start_camera_server(l_rx: Receiver<Frame>, r_rx: Receiver<Frame>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let addr = ([0, 0, 0, 0], 80).into();
        let make_svc = make_service_fn(move |_conn| {
            let r_rx = r_rx.clone();
            futures::future::ok::<_, std::convert::Infallible>(service_fn(move |req| {
                serve(req, r_rx.clone())
            }))
        });
        let server = hyper::Server::bind(&addr).serve(make_svc);
        println!("Serving on http://{}", server.local_addr());
        server.await.unwrap();
    })
}
