"use strict";

const { Device } = require("mediasoup-client");
const io = require("socket.io-client");

let device;
const connections = [];

// connect to the SFU
const socket = io("https://127.0.0.1:3000", {
  secure: true,
  reconnection: true,
  rejectUnauthorized: false,
  path: "/ws",
  transports: ["websocket"],
});

// promisify the socket requests
const socketRequest = (type, data = {}) => {
  return new Promise((resolve) => socket.emit(type, data, resolve));
};

// load the mediasoup device by providing it with the RTP capabilities of the
// server (mediasoup router)
async function loadDevice(routerRtpCapabilities) {
  try {
    device = new Device();
  } catch (error) {
    if (error.name === "UnsupportedError") {
      console.error("browser not supported");
    }
    console.error(error);
  }

  await device.load({ routerRtpCapabilities });
}

// add a video element to the DOM and create a peer connection
function createConnection() {
  const id = connections.length + 1;
  const video = newVideo(id, document.getElementById("videos"));
  const peerConnection = new RTCPeerConnection();
  peerConnection.onicecandidate = async (evt) => await onicecandidate(evt);
  peerConnection.oniceconnectionstatechange = () =>
    oniceconnectionstatechange(peerConnection);
  peerConnection.ontrack = (evt) => ontrack(evt, video);

  const connection = {
    id,
    video,
    peerConnection,
    isConnected: false,
    candidate: null,
  };

  connections.push(connection);

  return connection;
}

function newVideo(id, parent) {
  const el = document.createElement("video");
  el.setAttribute("id", `video_${id}`);
  el.setAttribute("controls", "");
  el.setAttribute("autoplay", "");
  el.setAttribute("playsinline", "");
  parent.appendChild(el);

  return el;
}

// signaling
socket.on("connect", async function () {
  console.log("connected to websocket");

  const data = await socketRequest("getRouterRtpCapabilities");
  await loadDevice(data);

  // TODO: onlys subscribe when a remote peer connection
  await subscribe();
  await subscribe();
});

socket.on("disconnect", () => {
  console.log("disconnected from websocket");
});

socket.on("connect_error", (error) => {
  console.error(error);
});

socket.on("newProducer", () => {
  console.log("newProducer");
});

async function subscribe() {
  console.log("subscribing");

  let stream;
  const data = await socketRequest("createConsumerTransport", {
    forceTcp: false,
  });

  if (data.error) {
    console.error(data.error);
    return;
  }

  const transport = device.createRecvTransport(data);

  transport.on("connect", ({ dtlsParameters }, callback, errback) => {
    socketRequest("connectConsumerTransport", {
      transportId: transport.id,
      dtlsParameters,
    })
      .then(callback)
      .catch(errback);
  });

  transport.on("connectionstatechange", async (state) => {
    console.log("connectionstatechange: ", state);

    switch (state) {
      case "connecting":
        break;

      case "connected":
        let connection = createConnection();
        // if (connection) {
        //   const pc = connection.peerConnection;
        //   pc.setRemoteDescription(new RTCSessionDescription(data));
        //   pc.createAnswer().then((desc) => {
        //     console.log("desc", desc);
        //     // ws.send(JSON.stringify({ type: "answer", sdp: desc.sdp }));
        //     pc.setLocalDescription(desc);
        //   });
        // }
        if (stream) connection.video.srcObject = await stream;
        await socketRequest("resume");
        break;

      case "failed":
        transport.close();
        break;

      default:
        break;
    }
  });

  stream = await consume(transport);
}

async function consume(transport) {
  console.log("consume");
  const { rtpCapabilities } = device;
  const data = await socketRequest("consume", { rtpCapabilities });
  const { producerId, id, kind, rtpParameters } = data;

  let codecOptions = {};
  const consumer = await transport.consume({
    id,
    producerId,
    kind,
    rtpParameters,
    codecOptions,
  });
  const stream = new MediaStream();
  stream.addTrack(consumer.track);
  return stream;
}

// TODO: is this ever called?
// if (data.type == "candidate") {
//   console.log("sdp", data.sdp);
//   let candidate = new RTCIceCandidate({
//     candidate: data.sdp.candidate,
//     sdpMid: "something", // don't make it up, you get this in onicecandidate
//     sdpMLineIndex: 12345,
//   });
//   last_connection.peerConnection.addIceCandidate(candidate);
// }

async function onicecandidate(evt) {
  console.log("onicecandidate", evt);

  await waitForOpenSocket(ws);

  if (evt.candidate) {
    ws.send(
      JSON.stringify({
        type: "candidate",
        sdp: "a=" + evt.candidate.toJSON().candidate,
      })
    );
  }
}

function ontrack(evt, video) {
  console.log("added track", video, evt);
  video.srcObject = evt.streams[0];
}

async function waitForOpenSocket(socket) {
  console.log("waitForOpenSocket", socket);

  return new Promise((resolve) => {
    if (socket.readyState !== socket.OPEN) {
      socket.addEventListener("open", (_) => resolve());
    } else {
      resolve();
    }
  });
}
