<!-- omit in toc -->
# Arcas Load Test Server
Using bindings into LibWebRTC from Chromium, this Rust-based load test server
interacts with a [JavaScript SDK](https://github.com/arcas-io/load-test-sdk) and 
creates peer connections on a given SFU.

<br>

<!-- omit in toc -->
# Table of Contents

- [Configuration](#configuration)
- [Running](#running)
- [Dependent Services](#dependent-services)
- [Running the Server](#running-the-server)
- [Building the Docker Image](#building-the-docker-image)
- [Running Docker](#running-docker)
- [Protocol Buffers](#protocol-buffers)
- [API](#api)
  - [Create a New Session](#create-a-new-session)
  - [Starting a Session](#starting-a-session)
  - [Stopping a Session](#stopping-a-session)
  - [Retrieve Session Stats](#retrieve-session-stats)
  - [Create Peer Connection](#create-peer-connection)
  - [Create Offer](#create-offer)
  - [Create Anwser](#create-anwser)
  - [Set Local Description](#set-local-description)
  - [Set Remote Description](#set-remote-description)
  - [Add a Track](#add-a-track)
  - [Add a Transceiver](#add-a-transceiver)
  - [Get Transceivers](#get-transceivers)
  - [Peer Connection Observer Stream](#peer-connection-observer-stream)

<br>

---

## Configuration
First, copy the example config:

```shell
cp .env.example .env
```

Update `.env` with the appropriate values.


## Running

## Dependent Services
To run containers of dependent services:
```shell
docker compose up
```

To pull up the Grafana dashboard, navigate your browser to `http://localhost:9091`.  The default username and password is `admin`.

To view the video dashboard, navigate your browser to `http://localhost:9091/d/R48Exadnz/video-dashboard?orgId=1`.


## Running the Server

```shell
RUST_LOG=INFO cargo run
```

## Building the Docker Image
```shell
docker build . -t "arcas/load-test-server"
```

## Running Docker
```shell
docker run -p 50051:50051 "arcas/load-test-server"
```

## Protocol Buffers
The protocol buffers are located in the `/proto` directory.

## API

The examples below use [grpccurl](https://github.com/fullstorydev/grpcurl) and assumes they're run from the repo base.

### Create a New Session

Create a new session on the server.

**Request Protocol Buffers**
```protobuf
enum LogLevel {
  NONE = 0;
  INFO = 1;
  WARN = 2;
  ERROR = 3;
  VERBOSE = 4;
}

message CreateSessionRequest {
  string session_id = 1;
  string name = 2;
  uint64 polling_state_s = 3;
  LogLevel log_level = 4;
}
```

**Response Protocol Buffers**
```protobuf
message CreateSessionResponse { string session_id = 1; }
```


```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"name": "First Session"}' [::]:50051 webrtc.WebRtc/CreateSession
```

### Starting a Session
Once a session is created, it can be started.

**Request Protocol Buffers**
```protobuf
message StartSessionRequest { string session_id = 1; }
```

**Response Protocol Buffers**
```protobuf
message StopSessionRequest { string session_id = 1; }
```

After creating a session:

```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"sessionId": "9s-KsEPQkO_IgfINBV4x6"}' [::]:50051 webrtc.WebRtc/StartSession
```

### Stopping a Session
Stop a session and clean up resources.

**Request Protocol Buffers**
```protobuf
message StopSessionRequest { string session_id = 1; }\
```

After creating and starting a session:

```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"sessionId": "9s-KsEPQkO_IgfINBV4x6"}' [::]:50051 webrtc.WebRtc/StopSession
```

### Retrieve Session Stats
Stats are collected per session.

**Request Protocol Buffers**
```protobuf
message GetStatsRequest { string session_id = 1; }
```

**Response Protocol Buffers**
```protobuf
message GetStatsRequest { string session_id = 1; }

message PeerConnectionState {
  int32 num_sending = 1;
  int32 num_not_sending = 2;
  int32 num_receiving = 3;
  int32 num_not_receiving = 4;
}

message SessionStats {
  string id = 1;
  string name = 2;
  uint64 num_peer_connections = 3;
  string state = 4;
  PeerConnectionState peer_connection_state = 5;
  google.protobuf.Timestamp start_time = 6;
  google.protobuf.Timestamp stop_time = 7;
  uint64 elapsed_time = 8;
}
message GetStatsResponse {
  SessionStats session = 1;
}
```

To retrieve stats for an active session:

```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"sessionId": "9s-KsEPQkO_IgfINBV4x6"}' [::]:50051 webrtc.WebRtc/GetStats
```


### Create Peer Connection
Create a new peer connection for an active session.

**Request Protocol Buffers**
```protobuf
message CreatePeerConnectionRequest {
  string session_id = 1;
  string peer_connection_id = 2;
  string name = 3;
}
```

To create a new peer connection on an active session:

```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"sessionId": "9s-KsEPQkO_IgfINBV4x6", "peerConnectionId": "py7cllxbm--cyw93x7k4p", "name": "First Peer Connection"}' [::]:50051 webrtc.WebRtc/CreatePeerConnection
```


### Create Offer


**Request Protocol Buffers**
```protobuf
message CreateSDPRequest { 
  string session_id = 1; 
  string peer_connection_id = 2; 
}
```

**Response Protocol Buffers**
```protobuf
enum SDPType {
  OFFER = 0;
  PRANSWER = 1;
  ANSWER = 2;
  ROLLBACK = 3;
}

message CreateSDPResponse { 
  string session_id = 1; 
  string peer_connection_id = 2; 
  string sdp = 3; 
  SDPType sdp_type = 4; 
}
```

To create an offer for a peer connection of an active session:

```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"sessionId": "9s-KsEPQkO_IgfINBV4x6", "peerConnectionId": "py7cllxbm--cyw93x7k4p"}' [::]:50051 webrtc.WebRtc/CreateOffer
```

### Create Anwser


**Request Protocol Buffers**
```protobuf
message CreateSDPRequest { 
  string session_id = 1; 
  string peer_connection_id = 2; 
}
```

**Response Protocol Buffers**
```protobuf
enum SDPType {
  OFFER = 0;
  PRANSWER = 1;
  ANSWER = 2;
  ROLLBACK = 3;
}

message CreateSDPResponse { 
  string session_id = 1; 
  string peer_connection_id = 2; 
  string sdp = 3; 
  SDPType sdp_type = 4; 
}
```

To create an answer for a peer connection of an active session:

```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"sessionId": "9s-KsEPQkO_IgfINBV4x6", "peerConnectionId": "py7cllxbm--cyw93x7k4p"}' [::]:50051 webrtc.WebRtc/CreateAnswer
```

### Set Local Description


**Request Protocol Buffers**
```protobuf
enum SDPType {
  OFFER = 0;
  PRANSWER = 1;
  ANSWER = 2;
  ROLLBACK = 3;
}

message SetSDPRequest { 
  string session_id = 1; 
  string peer_connection_id = 2; 
  string sdp = 3; 
  SDPType sdp_type = 4; 
}
```

**Response Protocol Buffers**
```protobuf
message SetSDPResponse { 
  string session_id = 1; 
  string peer_connection_id = 2; 
  bool success = 3; 
}
```

To set the local description for a peer connection of an active session:

```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"sessionId": "9s-KsEPQkO_IgfINBV4x6", "peerConnectionId": "py7cllxbm--cyw93x7k4p", "sdp": "", "sdp_type": "OFFER"}' [::]:50051 webrtc.WebRtc/SetLocalDescription
```

### Set Remote Description


**Request Protocol Buffers**
```protobuf
enum SDPType {
  OFFER = 0;
  PRANSWER = 1;
  ANSWER = 2;
  ROLLBACK = 3;
}

message SetSDPRequest { 
  string session_id = 1; 
  string peer_connection_id = 2; 
  string sdp = 3; 
  SDPType sdp_type = 4; 
}
```

**Response Protocol Buffers**
```protobuf
message CreateSDPResponse { 
  string session_id = 1; 
  string peer_connection_id = 2; 
  string sdp = 3; 
  SDPType sdp_type = 4; 
}
```

To set the remote description for a peer connection of an active session:

```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"sessionId": "9s-KsEPQkO_IgfINBV4x6", "peerConnectionId": "py7cllxbm--cyw93x7k4p", "sdp": "", "sdp_type": "ANSWER"}' [::]:50051 webrtc.WebRtc/SetRemotelDescription
```

### Add a Track


**Request Protocol Buffers**
```protobuf
message AddTrackRequest {
  string session_id = 1;
  string peer_connection_id = 2;
  string track_id = 3;
  string track_label = 4;
}
```

To add a track to a peer connection of an active session:

```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"sessionId": "9s-KsEPQkO_IgfINBV4x6", "peerConnectionId": "py7cllxbm--cyw93x7k4p", "track_id": "", "track_label": ""}' [::]:50051 webrtc.WebRtc/AddTrack
```

### Add a Transceiver


**Request Protocol Buffers**
```protobuf
message AddTransceiverRequest {
  string session_id = 1;
  string peer_connection_id = 2;
  string track_id = 3;
  string track_label = 4;
}
```

To add a transceiver to a peer connection of an active session:

```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"sessionId": "9s-KsEPQkO_IgfINBV4x6", "peerConnectionId": "py7cllxbm--cyw93x7k4p", "track_id": "", "track_label": ""}' [::]:50051 webrtc.WebRtc/AddTransceiver
```

### Get Transceivers


**Request Protocol Buffers**
```protobuf
message GetTransceiversRequest {
  string session_id = 1;
  string peer_connection_id = 2;
}
```

**Response Protocol Buffers**
```protobuf
message GetTransceiversResponse {
  repeated Transceiver transceivers = 1;
}
```

To retrieve transceivers for a peer connection of an active session:

```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"sessionId": "9s-KsEPQkO_IgfINBV4x6", "peerConnectionId": "py7cllxbm--cyw93x7k4p"}' [::]:50051 webrtc.WebRtc/GetTransceivers
```

### Peer Connection Observer Stream


**Request Protocol Buffers**
```protobuf
message ObserverRequest { 
  string session_id = 1; 
  string peer_connection_id = 2; 
}
```

**Response Protocol Buffers**
```protobuf
message IceCandidate {
    string sdp = 1;
    string mid = 2;
    uint32 mline_index = 3;
}

message VideoTransceiver {
    string mid = 1;
    string direction = 2;
};

enum MediaType {
    AUDIO = 0;
    VIDEO = 1;
    DATA = 2;
    UNSUPPORTED = 3;
}

enum TransceiverDirection {
    SENDRECV = 0;
    SENDONLY = 1;
    RECVONLY = 2;
    INACTIVE = 3;
}

message Transceiver {
    string id = 1;
    string mid = 2;
    TransceiverDirection direction = 3;
    MediaType media_type = 4;
}

message PeerConnectionObserverMessage {
    oneof event {
        IceCandidate ice_candidate = 1;
        VideoTransceiver video_transceiver = 2;
    }
}
```

To retrieve a stream of ice candidate and video transceiver events for a peer connection of an active session:

```shell
grpcurl -plaintext -import-path ./proto -proto webrtc.proto -d '{"sessionId": "9s-KsEPQkO_IgfINBV4x6", "peerConnectionId": "py7cllxbm--cyw93x7k4p"}' [::]:50051 webrtc.WebRtc/ObserverRequest
```
