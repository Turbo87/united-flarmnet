FROM rust:bullseye as build
WORKDIR /tmp/united-flarmnet
COPY . .
RUN cargo install --path . 

FROM debian:bullseye-slim 
COPY --from=build /usr/local/cargo/bin/united-flarmnet /usr/local/bin/united-flarmnet
WORKDIR /data
ENTRYPOINT /usr/local/bin/united-flarmnet
