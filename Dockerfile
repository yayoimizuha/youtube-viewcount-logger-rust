FROM rust:latest
LABEL authors="tomokazu"

ENTRYPOINT ["top", "-b"]