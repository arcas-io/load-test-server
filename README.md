# WebRTC Server


## Running
```shell
RUST_LOG=TRACE cargo run
```

## Validating
```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"name": "First Session"}' [::]:50051 webrtc.WebRtc/CreateSession
```
