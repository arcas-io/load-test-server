FROM rust:latest as builder

RUN USER=root cargo new --bin load-test-server
WORKDIR /load-test-server

COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release
RUN rm src/*.rs
COPY ./src ./src

RUN rm ./target/release/deps/load-test-server*
RUN cargo build --release

FROM debian:buster-slim
COPY --from=builder /load-test-server/target/release/load-test-server .
ENV HOST=0.0.0.0
ENV PORT=50061
CMD ["./load-test-server"]
EXPOSE 50061
