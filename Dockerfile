FROM rust:latest
LABEL authors="tomokazu"

RUN apt update && apt upgrade -y && apt install libssl-dev -y
WORKDIR /work
RUN git clone https://github.com/yayoimizuha/youtube-viewcount-logger-rust.git /work --depth 1
RUN cargo build --release
ENTRYPOINT ["echo", "Yo!"]