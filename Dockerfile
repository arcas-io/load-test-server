# larger build image
FROM debian:buster as builder
USER root

RUN apt-get update -y && apt-get install -y libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  build-essential \
  libssl-dev \
  gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly \
  gstreamer1.0-libav libgstrtspserver-1.0-dev libges-1.0-dev libsrtp2-dev \
  libclang-dev \
  git
USER root
RUN mkdir -p /deps
WORKDIR /deps
RUN git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git
ENV PATH=/deps/depot_tools:$PATH

USER root
RUN mkdir -p /webrtc
WORKDIR /webrtc
RUN fetch --nohooks webrtc && \
  gclient sync && \
  cd /webrtc/src && \
  git checkout 27edde3182ccc9c6afcd65b7e6d8b6558cb49d64 && \
  gclient sync && \
  rm -rf ./.git ./third_party/.git
WORKDIR /webrtc/src
RUN apt-get install lsb-release sudo gcc-arm-linux-gnueabihf g++-8-arm-linux-gnueabihf -y && /webrtc/src/build/install-build-deps.sh
RUN gn gen /webrtc/out --args='rtc_build_examples=false rtc_build_tools=false rtc_use_x11=false rtc_enable_protobuf=false rtc_include_tests=false is_debug=false target_os="linux"'
RUN ninja -C /webrtc/out

# smaller run image
FROM base as webrtc
COPY --from=builder /webrtc/src /webrtc/src
COPY --from=builder /webrtc/out /webrtc/out
WORKDIR /webrtc
ENV WEBRTC_SRC_DIR=/webrtc/src WEBRTC_OBJ_DIR=/webrtc/out/obj