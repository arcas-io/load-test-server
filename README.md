# WebRTC Server


## Running
```shell
RUST_LOG=INFO cargo run
```

## API

### Create a New Session
```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"name": "First Session"}' [::]:50051 webrtc.WebRtc/CreateSession
```
