FROM rust:1-trixie@sha256:e84068000361bf3d4a5ba2b7647f62893528b1eb9f847d7a23b493f5ce8ea254 AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
RUN cargo build --release --locked

FROM debian:trixie-slim@sha256:8aa5a7bf5a1955bddbe7d74fcdd0b91ab30e74362a8b73af672b0481b35b44a0
# Inter draws the share images, with Noto for the letters of translated posts it lacks (Arabic, Devanagari, Thai,
# Chinese, Japanese, Korean...); certificates let IndexNow pings leave over HTTPS.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates fonts-inter fonts-noto-core fonts-noto-cjk \
  && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/lanternlog /usr/local/bin/lanternlog
ENV LANTERNLOG_DATA=/data PORT=8080
VOLUME /data
EXPOSE 8080
CMD ["lanternlog"]
