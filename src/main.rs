#[cfg(feature = "desktop")]
#[tokio::main]
async fn main() {
    eyetrackvr_server::desktop::desktop_main().await
}

#[cfg(not(feature = "desktop"))]
fn main() {}
