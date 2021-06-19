FROM rust:1.52 as builder

RUN USER=root cargo new --bin rustbot

WORKDIR /rustbot

COPY Cargo.toml .

RUN cargo build --release \
 && rm src/*.rs

COPY . .

RUN rm ./target/release/deps/rustbot* \
 && cargo build --release


FROM debian:buster-slim

ARG APP=/usr/src/app

ENV TZ=Etc/UTC
ENV APP_USER=appuser

WORKDIR ${APP}

RUN apt-get update \
 && apt-get install -y ca-certificates tzdata \
 && rm -rf /var/lib/apt/lists/* \
 && groupadd $APP_USER \
 && useradd -g $APP_USER $APP_USER

COPY --from=builder /rustbot/target/release/rustbot .

RUN chown -R $APP_USER:$APP_USER .

USER $APP_USER

CMD ["./rustbot"]
