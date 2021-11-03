# WebRTC Server

## Configuration
First, copy the example config:
```shell
cp .env.example .env
```
Update `.env` with the appropriate values.


## Running
```shell
RUST_LOG=INFO cargo run
```

## Building the Docker Image
```shell
docker build . -t "littlebearlabs/server"
```

## Running Docker
```shell
docker run -p 50051:50051 "littlebearlabs/server"
```

## Running the Dependencies in a Docker Network
To run the docker network, which includes Grafana, Prometheus, and the StatsD Exporter:
```shell
docker compuse up
```

To pull up the Grafana dashboard, navigate your browser to `http://localhost:9091`.  The default username and password is `admin`.

To view the video dashboard, navigate your browser to `http://localhost:9091/d/R48Exadnz/video-dashboard?orgId=1`.

## API

### Create a New Session
```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"name": "First Session"}' [::]:50051 webrtc.WebRtc/CreateSession
```

### Starting a Session
After creating a session:

```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"sessionId": "9s-KsEPQkO_IgfINBV4x6"}' [::]:50051 webrtc.WebRtc/StartSession
```

### Stopping a Session
After creating and starting a session:

```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"sessionId": "9s-KsEPQkO_IgfINBV4x6"}' [::]:50051 webrtc.WebRtc/StopSession
```
