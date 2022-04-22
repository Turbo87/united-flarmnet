united-flarmnet
==============================================================================

Merging [FlarmNet], [OGN] and [WeGlide live tracking] data into a single
FlarmNet file.

[FlarmNet]: https://www.flarmnet.org/
[OGN]: http://ddb.glidernet.org/
[WeGlide live tracking]: https://www.weglide.org/live/


Usage
------------------------------------------------------------------------------

```
cargo run
```

After the program has finished downloading and processing the data,
a `united.fln` file.


Docker
------------------------------------------------------------------------------
To build a docker image with a function united-flarmnet binary, use the
following commands:

```
docker build . -t united-flarmnet
docker run --mount type=bind,source=${PWD},target=/data -t united-flarmnet
```
This will create a `united.fln` in the current directory.


License
------------------------------------------------------------------------------

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)

- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.
