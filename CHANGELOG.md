# Changelog

All notable changes to `sqlx-pool-registry` are documented in this file.

This project was forked from
[`sqlx-pool-router` 0.2.0](https://crates.io/crates/sqlx-pool-router/0.2.0)
at commit [`bd5dd8a`](https://github.com/doublewordai/sqlx-pool-router/commit/bd5dd8a049697dfab92fe88a6febb82af96c4800).
The upstream changelog is intentionally not duplicated here; entries below
cover changes made in this crate after the fork.

## [0.3.0]

### Changed

- Raised the minimum supported Rust version (MSRV) to 1.94.
- Made `DbPools: Deref<Target = PgPool>` an opt-in legacy API behind the
  `with-deref` feature. It always returns the primary pool, bypasses read
  routing, and will be removed in the next major version. Use `read()` or
  `write()` for database queries.

## [0.2.1]

### Added

- Added optional named-pool support through the `with-named-pools` feature.
- Added the generic `PoolRegistry<P>` API with exact-name lookup, replacement,
  iteration, collection conversion, and the `UnknownPool` error type.
- Added mutually exclusive SQLx 0.8 and SQLx 0.9 compatibility features. SQLx
  0.8 remains enabled by default, while SQLx 0.9 is opt-in.
- Re-exported the selected SQLx version as `sqlx_pool_registry::sqlx` so public
  API types and consumer queries use the same SQLx version.
- Added `DbPools::primary()` and `DbPools::replica()` topology accessors. The
  replica accessor returns only an explicitly configured replica and does not
  fall back to the primary pool.

### Changed

- Renamed the fork from `sqlx-pool-router` to `sqlx-pool-registry` and updated
  package, repository, documentation, and example references.
- Split the implementation into focused modules for pool routing, providers,
  named registries, and test pools without changing the core routing model.
- Clarified that `read()` and `write()` express routing intent rather than
  providing compile-time query restrictions.
- Clarified that `TestDbPools` uses PostgreSQL's read-only default to help find
  ordinary routing mistakes and is not a security boundary.
- Configured docs.rs to document named pools with the default SQLx 0.8 backend
  without enabling the mutually exclusive SQLx features together.

### Testing

- Added offline coverage for pool topology, fallback routing, closing, named
  registry behavior, generic providers, and both supported SQLx versions.
- Separated PostgreSQL-dependent tests from the default offline test suite.
- Expanded CI and release validation to build, lint, test, and check examples
  for SQLx 0.8 and SQLx 0.9 independently.

[0.3.0]: https://github.com/anton-dutov/sqlx-pool-registry/releases/tag/sqlx-pool-registry-v0.3.0
[0.2.1]: https://github.com/anton-dutov/sqlx-pool-registry/releases/tag/sqlx-pool-registry-v0.2.1
