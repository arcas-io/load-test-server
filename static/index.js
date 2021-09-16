"use strict";

let ws = new WebSocket("ws://localhost:3000/ws");
const connections = [];

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
  el.setAttribute("autoplay", "");
  parent.appendChild(el);

  return el;
}

let connection;

ws.onmessage = (message) => {
  console.log("new message ", message);

  let data = JSON.parse(message.data);

  if (data.type && data.type == "offer") {
    connection = createConnection();
    console.log("getNextConnection", connection);

    if (connection) {
      const pc = connection.peerConnection;
      pc.setRemoteDescription(new RTCSessionDescription(data));
      pc.createAnswer().then((desc) => {
        console.log("desc", desc);
        ws.send(JSON.stringify({ type: "answer", sdp: desc.sdp }));
        pc.setLocalDescription(desc);
      });
    }
    return;
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
};

async function onicecandidate(evt) {
  await waitForOpenSocket(ws);

  console.log("onicecandidate", evt);

  if (evt.candidate) {
    ws.send(
      JSON.stringify({
        type: "candidate",
        sdp: "a=" + evt.candidate.toJSON().candidate,
      })
    );
  }
}

function oniceconnectionstatechange(peerConnection) {
  console.log("new ice state: ", peerConnection.iceConnectionState);
}

function ontrack(evt, video) {
  console.log("added track", video, evt);
  video.srcObject = evt.streams[0];
}

async function waitForOpenSocket(socket) {
  return new Promise((resolve) => {
    if (socket.readyState !== socket.OPEN) {
      socket.addEventListener("open", (_) => resolve());
    } else {
      resolve();
    }
  });
}
