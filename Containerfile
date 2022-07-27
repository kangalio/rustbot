FROM rust:latest as builder

ENV SQLX_OFFLINE=true
ENV DATABASE_URL=sqlite:database/database.sqlite

RUN USER=root cargo new --bin rustbot

WORKDIR /rustbot

COPY Cargo.toml .
COPY Cargo.lock .

RUN cargo build --release
RUN rm src/*.rs

COPY . .

RUN rm ./target/release/deps/rustbot*
RUN cargo build --release


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

RUN mkdir database
RUN chown -R $APP_USER:$APP_USER .

USER $APP_USER

CMD ["./rustbot"]
