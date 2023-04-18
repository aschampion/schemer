# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).


<!-- next-header -->
## [Unreleased]


## [0.2.2] - 2023-04-18
### Changed
- Updated MSRV from 1.56 to 1.59.

## [0.2.1] - 2022-11-08
### Changed
- Updated minimum rusqlite from 0.24 to 0.25.

## [0.2.0] - 2022-08-19
### Changed
- `RusqliteAdapter` is now generic over an error type `E` so that migrations can return error types other than `rusqlite::Error`.
- Migrated error handling from `failure` to `thiserror`.
- Updated crate to Rust 2018 Edition.


<!-- next-url -->
[Unreleased]: https://github.com/aschampion/schemer/compare/schemer-rusqlite-v0.2.2...HEAD
[0.2.2]: https://github.com/aschampion/schemer/compare/schemer-rusqlite-v0.2.1...schemer-rusqlite-v0.2.2"
[0.2.1]: https://github.com/aschampion/schemer/compare/schemer-rusqlite-v0.2.0...schemer-rusqlite-v0.2.1
[0.2.0]: https://github.com/aschampion/schemer/compare/schemer-rusqlite=v0.1.0...schemer-rusqlite-v0.2.0
