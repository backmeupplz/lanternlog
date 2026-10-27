FROM rust:1-trixie AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
RUN cargo build --release --locked

FROM debian:trixie-slim
# Inter draws the share images; certificates let IndexNow pings leave over HTTPS.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates fonts-inter && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/lanternlog /usr/local/bin/lanternlog
ENV LANTERNLOG_DATA=/data PORT=8080
VOLUME /data
EXPOSE 8080
CMD ["lanternlog"]
