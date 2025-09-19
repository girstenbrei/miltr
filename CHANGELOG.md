# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/girstenbrei/miltr/compare/miltr-v0.1.2...miltr-v0.1.3) - 2025-09-19

### Added

- Setup postfix integation tests ([#18](https://github.com/girstenbrei/miltr/pull/18))

### Fixed

- Fix protocol violation: Respond to SMFIC_ABORT with no response, not SMFIS_CONTINUE ([#22](https://github.com/girstenbrei/miltr/pull/22))
- failing client_v_server test ([#20](https://github.com/girstenbrei/miltr/pull/20))

### Other

- Upgrade dependencies ([#21](https://github.com/girstenbrei/miltr/pull/21))
- Add tarpaulin run to ci ([#15](https://github.com/girstenbrei/miltr/pull/15))

## [0.1.2](https://github.com/girstenbrei/miltr/compare/miltr-v0.1.1...miltr-v0.1.2) - 2025-05-23

### Other

- release v0.1.2 ([#10](https://github.com/girstenbrei/miltr/pull/10))

## [0.1.1](https://github.com/girstenbrei/miltr/compare/miltr-v0.1.0...miltr-v0.1.1) - 2025-01-26

### Added

- Add basic github workflows (#5)
- Add tracing as a feature
- Document client server separately

### Fixed

- Validate examples are compatible with a test (#4)

### Other

- Bump dependencies
- [chore] Make clippy happy!
- [docs] Remove and replace old references
- Add basic release script
