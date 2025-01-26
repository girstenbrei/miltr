# Miltr Client

[<img alt="github" src="https://img.shields.io/badge/github-girstenbrei/miltr/client-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/girstenbrei/miltr/tree/main/client)
[<img alt="crates.io" src="https://img.shields.io/crates/v/miltr_client.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/miltr-client)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-miltr_--_client-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/miltr-client)

A client implementation of the milter protocol.

```rust no_run
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncReadCompatExt;

use miltr_client::Client;
use miltr_common::{
    commands::Connect,
    optneg::OptNeg,
};

#[tokio::main]
async fn main() {
    let mut stream = TcpStream::connect("localhost:8080")
        .await
        .expect("Failed connecting to milter server")
        .compat();

    println!("Opened TCP connection");
    let options = OptNeg::default();
    let client = Client::new(options);
    let mut connection = client
        .connect_via(&mut stream)
        .await
        .expect("Failed to setup connection");

    // Further processing
}
```

Currently, the milter client is relatively barebone. It should handle all
commands, actions and modification actions correctly. But, everything else is
basically up to the user. For example, nothing prevents you from claiming one
behavior in option negotiation, but actually doing something else.

The use case for this client library currently is to have an example client to
mess around and test behavior with.

## Safety
This crate uses `unsafe_code = "forbid"` in it's linting, but is also using
`cast-possible-truncation = "allow"`. So use at your own risk.

## Semver
This crate follows semver specification with the following exceptions:

1. Minimum supported rust version: \
   A bump to the MSRV is not considered a semver major semver change, only a minor one.
2. Features starting with `_`. These are considered 'internal' and 'private'. This
   is mainly used for fuzz testing. It makes it much easier to fuzz internals directly.
   No external user should need to enable those features.


# Credits

## [purepythonmilter](https://github.com/gertvdijk/purepythonmilter/tree/develop)
Special credits go to [purepythonmilter](https://github.com/gertvdijk/purepythonmilter/tree/develop),
a python package containing a complete milter implementation. Without this resource to have a look
at "how they did it" this implementation would not have happened.

## Anh Vu
Another big thank you goes to Anh Vu (<vunpa1711@gmail.com>), working student at Retarus, who wrote a big
part of the integration tests and brought valuable feedback for implementation improvements. Thank you!
