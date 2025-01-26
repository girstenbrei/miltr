# Miltr

[<img alt="github" src="https://img.shields.io/badge/github-girstenbrei/miltr-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/girstenbrei/miltr)
[<img alt="crates.io" src="https://img.shields.io/crates/v/miltr.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/miltr)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-miltr-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/miltr)

This package is atm a purely virtual package, providing a namespace for:

- [miltr-server](https://docs.rs/miltr-server/latest/miltr_server/)
- [miltr-client](https://docs.rs/miltr-client/latest/miltr_client/)

Add one of those to your dependencies to get the client or server functionality.


## Safety
These crates uses `unsafe_code = "forbid"` in it's linting, but is also using
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
Another big thank you goes to Anh Vu (<vunpa1711@gmail.com>), working student at Retarus who wrote a big
part of the integration tests and brought valuable feedback for implementation improvements. Thank you!
