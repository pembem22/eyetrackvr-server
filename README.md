This readme is yet another TODO to do

# Building
## For Windows as a Standalone App
```ps
cargo run -r --no-default-features --features gui,desktop,inference --bin eyetrackvr-server -- --help
```
## For Android as an OpenXR API Layer
```ps
$env:ORT_LIB_LOCATION="path\to\onnxruntime\build\Windows\Release"; cargo dinghy -p auto-android-aarch64-api32 build -r --lib --no-default-features --features openxr-api-layer,android,gui,inference
cargo make apk
```