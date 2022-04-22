FROM rust:bullseye as build
WORKDIR /tmp/
RUN git clone https://github.com/Turbo87/united-flarmnet.git && cd united-flarmnet
WORKDIR /tmp/united-flarmnet
RUN cargo install --path . 

FROM debian:bullseye-slim 
COPY --from=build /usr/local/cargo/bin/united-flarmnet /usr/local/bin/united-flarmnet
WORKDIR /data
ENTRYPOINT /usr/local/bin/united-flarmnet
