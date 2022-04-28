FROM rust:latest as builder

RUN USER=root cargo new --bin load-test-server
WORKDIR /load-test-server

RUN apt-get update
RUN apt-get install -y libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libgstreamer-plugins-bad1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav gstreamer1.0-tools gstreamer1.0-x gstreamer1.0-alsa gstreamer1.0-gl gstreamer1.0-gtk3 gstreamer1.0-qt5 gstreamer1.0-pulseaudio

COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release
RUN rm src/*.rs
COPY ./src ./src

RUN rm -f ./target/release/deps/load-test-server*
RUN cargo build --release

FROM debian:buster-slim
COPY --from=builder /load-test-server/target/release/load-test-server .
ENV HOST=0.0.0.0
ENV PORT=50061
CMD ["./load-test-server"]
EXPOSE 50061
