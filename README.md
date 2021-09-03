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

### Starting a Session
After creating a session:

```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"sessionId": "a2x9RSxNvV6huHvvFLp62"}' [::]:50051 webrtc.WebRtc/StartSession
```

### Stopping a Session
After creating and starting a session:

```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"sessionId": "a2x9RSxNvV6huHvvFLp62"}' [::]:50051 webrtc.WebRtc/StopSession
```