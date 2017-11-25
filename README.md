# Schemer [![Build Status](https://travis-ci.org/aschampion/schemer.svg?branch=master)](https://travis-ci.org/aschampion/schemer)

Schemer is a database schema migration library for Rust that supports directed acyclic graph (DAG) dependencies between migrations. It currently has adapters for the following databases:

- PostgreSQL: schemer-postgres

Other Rust schema migration libraries to consider if you do not require DAG migration dependencies:

- [schemamama](https://crates.io/crates/schemamama) (recommended -- this is the basis for Schemer's API)
- [dbmigrate](https://crates.io/crates/dbmigrate)
- [migrant](https://crates.io/crates/migrant)

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
