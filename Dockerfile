FROM rust:1.87 AS builder

WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./
COPY src src
RUN cargo build --release

FROM scratch

COPY --from=builder /usr/src/app/target/release/anonymous-bot /usr/local/bin/app

CMD ["app"]