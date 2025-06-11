FROM rust:1-slim
LABEL authors="tomokazu"
RUN apt update && apt upgrade -y && apt install libssl-dev -y
RUN pwd
COPY . .
RUN cargo install --path .
#ENTRYPOINT ["echo", "Yo!"]

FROM debian:slim
RUN apt update && apt install -y libssl-dev && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/youtube-viewcount-logger-rust /usr/local/bin/youtube-viewcount-logger-rust
CMD ["youtube-viewcount-logger-rust"]
