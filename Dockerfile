FROM rust:1-trixie AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
RUN cargo build --release --locked

FROM debian:trixie-slim
# Inter draws the share images, with Noto for the letters of translated posts it lacks (Arabic, Devanagari, Thai,
# Chinese, Japanese, Korean...); certificates let IndexNow pings leave over HTTPS.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates fonts-inter fonts-noto-core fonts-noto-cjk \
  && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/lanternlog /usr/local/bin/lanternlog
ENV LANTERNLOG_DATA=/data PORT=8080
VOLUME /data
EXPOSE 8080
CMD ["lanternlog"]
