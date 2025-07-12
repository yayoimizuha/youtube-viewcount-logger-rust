FROM rust:1-slim as builder
LABEL authors="tomokazu"
RUN apt update && apt upgrade -y && apt install libssl-dev build-essential pkg-config -y
RUN pwd
COPY . .
RUN cargo install --path .
#ENTRYPOINT ["echo", "Yo!"]

FROM debian:stable-slim
RUN apt update && apt install -y libssl-dev && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/ /usr/local/bin/
CMD ["youtube-viewcount-logger-rust"]
