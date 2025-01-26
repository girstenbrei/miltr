# Miltr Server

[<img alt="github" src="https://img.shields.io/badge/github-girstenbrei/miltr/server-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/girstenbrei/miltr/tree/main/server)
[<img alt="crates.io" src="https://img.shields.io/crates/v/miltr_server.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/miltr-server)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-miltr_--_server-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/miltr-server)

This crate is an implementation of the milter protocol used by postfix
and sendmail.

A minimum viable use is:

```rust
use async_trait::async_trait;
use miltr_common::{actions::{Action, Continue}, commands::Recipient};
use miltr_server::Milter;

struct PrintRcptMilter;

#[async_trait]
impl Milter for PrintRcptMilter {
    type Error = &'static str;

    /// Just print the recipient
    async fn rcpt(&mut self, recipient: Recipient) -> Result<Action, Self::Error> {
        println!("Received recipient: {:?}", recipient);

        Ok(Continue.into())
    }

    /// Abort has to be implemented. It is called at least once per mail
    /// handling an can occur at any time during the milter conversation.
    /// As this milter does not have any state, nothing has to be cleared.
    async fn abort(&mut self) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }
}
```

For examples on how to use it, see the `./examples` directory.

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

## Development

### Todos
**Parse incoming Macros**: \
In the `protocol::commands::optneg::Optneg::parse(…)`, implement parsing
incoming macros. This is currently stubbed out.

### Design Decision
This tries to give small 'justifications' about implementation details.

#### `BytesMut` and Ownership
It was relatively easy to 'parse' this protocol using `BytesMut::(split_to|split_off)`.
This allows all parsed commands to just own their data without any borrowing complexity
as well as having parsing logic inside the parse-step (instead of in the access
functions/getters on structs).

This incurs some additional allocations but ATM worth the tradeoff in simplicity.
This might be optimized in the future.

#### Length & Math & Overflows
Currently, this library is not strict in handling parameter length. \
This means, an implementor can pass data to the milter codec which is to long.

The codec will reject this data in encoding as well as reject data from the network
which is to long in decoding.

Rejecting in this case will error out of the milter codec. This behavior is
maybe not ideal, but ATM the best I could come up with.

Additionally, this crate suffers from overflow panics. If you pass a parameter
(e.g. a Header value) with length `usize::MAX` on a 32bit system (-~> 4Gi of size), the codec
will try to get an item length: `name.len() + value.len()`. This will overflow
and therefore panic in debug mode, wrap in release mode, breaking the connection.

This is a currently accepted tradeoff as emails will probably be much smaller
than a usize on a modern system.

This can be implemented in a better way.


### Integration tests

To run integration tests, this repository needs a few pieces of software:

- postfix
- swaks

The integration tests assume they can call out to these components freely. To
avoid having to install them on your system, this repo contains a `docker-compose.yml`
installing a container with all necessary tools.

The tests can then be run:

```bash
docker compose run test_milter
```

## External Documentation regarding the Milter protocol

- Postfix Milter Readme: <https://www.postfix.org/MILTER_README.html>
- Sendmail Libmilter Docs Overview: <https://fossies.org/linux/sendmail/libmilter/docs/overview.html>
- Purepythonmilter's remarks on Milter: <https://github.com/gertvdijk/purepythonmilter/blob/develop/docs/milter-protocol.md>


# Credits

## [purepythonmilter](https://github.com/gertvdijk/purepythonmilter/tree/develop)
Special credits go to [purepythonmilter](https://github.com/gertvdijk/purepythonmilter/tree/develop),
a python package containing a complete milter implementation. Without this resource to have a look
at "how they did it" this implementation would not have happened.

## Anh Vu
Another big thank you goes to Anh Vu (<vunpa1711@gmail.com>), working student at the time at Retarus who wrote a big
part of the integration tests and brought valuable feedback for implementation improvements. Thank you!
