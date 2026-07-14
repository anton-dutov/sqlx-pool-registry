# sqlx-pool-registry

Forked from [doublewordai/sqlx-pool-router](https://github.com/doublewordai/sqlx-pool-router).

[![Crates.io](https://img.shields.io/crates/v/sqlx-pool-registry.svg)](https://crates.io/crates/sqlx-pool-registry) [![Documentation](https://docs.rs/sqlx-pool-registry/badge.svg)](https://docs.rs/sqlx-pool-registry) [![License](https://img.shields.io/crates/l/sqlx-pool-registry.svg)](https://github.com/anton-dutov/sqlx-pool-registry#license)

A lightweight Rust library for routing database operations to different SQLx PostgreSQL connection pools based on whether they're read or write operations.

This enables load distribution by routing read-heavy operations to read replicas while ensuring write operations always go to the primary database.

## Features

- **Zero-cost abstraction**: Trait-based design with no runtime overhead
- **Explicit routing**: Read/write intent stays visible at each database call site
- **Backward compatible**: `PgPool` implements `PoolProvider` for seamless integration
- **Flexible**: Use single pool or separate primary/replica pools
- **SQLx compatibility**: Select SQLx 0.8 (default) or SQLx 0.9 at compile time
- **Named pools (optional)**: Group independent providers by name with the `with-named-pools` feature
- **Legacy `Deref` compatibility (optional)**: Restore `DbPools: Deref<Target = PgPool>` temporarily with the `with-deref` feature
- **Well-tested**: Comprehensive test suite with replica routing verification

## Installation

SQLx 0.8 is enabled by default:

``` toml
[dependencies]
sqlx-pool-registry = "0.2.1"
sqlx = { version = "0.8", features = ["postgres", "runtime-tokio"] }
```

For SQLx 0.9, disable the default compatibility feature and select 0.9 explicitly:

``` toml
[dependencies]
sqlx-pool-registry = { version = "0.2.1", default-features = false, features = ["with-sqlx-0_9"] }
sqlx = { version = "0.9", features = ["postgres", "runtime-tokio"] }
```

Exactly one of `with-sqlx-0_8` and `with-sqlx-0_9` must be enabled. The selected SQLx crate is also available as `sqlx_pool_registry::sqlx`. Add `with-named-pools` to either configuration to enable `PoolRegistry`. The effective minimum Rust version is 1.94 with SQLx 0.8 and 1.94 with SQLx 0.9.

### Legacy `Deref` migration

`DbPools` does not implement `Deref<Target = PgPool>` by default. To keep an existing `&*pools` call compiling while migrating, add `with-deref` to the crate features. This is a temporary compatibility path and will be removed in the next major version.

``` toml
[dependencies]
sqlx-pool-registry = { version = "0.2.1", features = ["with-deref"] }
```

`&*pools` always selects the primary pool and bypasses read routing. Do not use it for database queries: replace `fetch_one(&*pools)` with `fetch_one(pools.read())` for eligible reads or `fetch_one(pools.write())` for writes, locking reads, and read-after-write operations.

## Quick Start

### Single Pool (Development)

``` rust
use sqlx::PgPool;
use sqlx_pool_registry::PoolProvider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPool::connect("postgresql://localhost/mydb").await?;

    // PgPool implements PoolProvider automatically
    let result: (i32,) = sqlx::query_as("SELECT 1")
        .fetch_one(pool.read())
        .await?;

    Ok(())
}
```

### Read/Write Separation (Production)

``` rust
use sqlx::postgres::PgPoolOptions;
use sqlx_pool_registry::{DbPools, PoolProvider};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let primary = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgresql://primary-host/mydb")
        .await?;

    let replica = PgPoolOptions::new()
        .max_connections(10)  // More connections for read-heavy workload
        .connect("postgresql://replica-host/mydb")
        .await?;

    let pools = DbPools::with_replica(primary, replica);

    // Reads go to replica
    let users: Vec<(i32, String)> = sqlx::query_as("SELECT id, name FROM users")
        .fetch_all(pools.read())
        .await?;

    // Writes go to primary
    sqlx::query("INSERT INTO users (name) VALUES ($1)")
        .bind("Alice")
        .execute(pools.write())
        .await?;

    Ok(())
}
```

### Named Pools (Optional)

With the `with-named-pools` feature, one registry can hold independent pool providers for databases such as `auth` and `analytics`. Looking up a name returns that provider; it does not select mutable registry-wide state.

``` rust
use sqlx::postgres::PgPoolOptions;
use sqlx_pool_registry::{DbPools, PoolProvider, PoolRegistry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let auth_primary = PgPoolOptions::new()
        .connect("postgresql://primary-host/auth")
        .await?;
    let auth_replica = PgPoolOptions::new()
        .connect("postgresql://replica-host/auth")
        .await?;
    let analytics_primary = PgPoolOptions::new()
        .connect("postgresql://primary-host/analytics")
        .await?;

    let mut pools = PoolRegistry::new();
    pools.insert(
        "auth",
        DbPools::with_replica(auth_primary, auth_replica),
    );
    pools.insert("analytics", DbPools::new(analytics_primary));

    let auth = pools.try_get("auth")?;
    let analytics = pools.try_get("analytics")?;

    let auth_primary = auth.primary();
    let auth_replica = auth.replica(); // Option<&PgPool>; no fallback
    let auth_reads = auth.read(); // Replica
    let analytics_reads = analytics.read(); // Primary fallback
    let analytics_writes = analytics.write(); // Primary

    Ok(())
}
```

Use `try_get()` in functions returning `Result`; `get()` returns `Option` for optional lookups. `DbPools::replica()` exposes only an explicitly configured replica and may return `None`. In contrast, `read()` falls back to the primary, while `write()` always returns the primary.

## Testing with `TestDbPools`

The crate includes a `TestDbPools` helper for use with `#[sqlx::test]` that makes ordinary write-through-read routing mistakes fail during tests:

``` rust
use sqlx::PgPool;
use sqlx_pool_registry::{PoolProvider, TestDbPools};

#[sqlx::test]
async fn test_repository(pool: PgPool) {
    // TestDbPools creates a replica that is read-only by default
    let pools = TestDbPools::new(pool).await.unwrap();

    // Ordinary writes through .read() fail unless read-only mode is overridden
    let result = sqlx::query("INSERT INTO users (name) VALUES ('Alice')")
        .execute(pools.read())
        .await;
    assert!(result.is_err());

    // Writes through .write() work fine
    sqlx::query("CREATE TEMP TABLE users (id INT, name TEXT)")
        .execute(pools.write())
        .await
        .unwrap();
}
```

**Why use `TestDbPools`?**

- Helps catch routing bugs immediately in tests
- No need for an actual replica database in test environment
- Sets `default_transaction_read_only = on` on each read-pool connection
- PostgreSQL rejects writes to non-temporary tables by default

`TestDbPools` is a testing aid, not a security boundary. PostgreSQL clients can override the default for an individual transaction or session, and read-only transactions do not prohibit every possible write. See PostgreSQL's [`SET TRANSACTION`](https://www.postgresql.org/docs/current/sql-set-transaction.html) documentation for the exact restrictions.

## Generic Programming

Make your types generic over `PoolProvider` to support both single and multi-pool configurations:

``` rust
use sqlx_pool_registry::PoolProvider;

struct Repository<P: PoolProvider> {
    pools: P,
}

impl<P: PoolProvider> Repository<P> {
    async fn get_user(&self, id: i64) -> Result<String, sqlx::Error> {
        // Read from replica
        sqlx::query_scalar("SELECT name FROM users WHERE id = $1")
            .bind(id)
            .fetch_one(self.pools.read())
            .await
    }

    async fn create_user(&self, name: &str) -> Result<i64, sqlx::Error> {
        // Write to primary
        sqlx::query_scalar("INSERT INTO users (name) VALUES ($1) RETURNING id")
            .bind(name)
            .fetch_one(self.pools.write())
            .await
    }
}

// Works with both PgPool and DbPools!
let repo_single = Repository { pools: single_pool };
let repo_multi = Repository { pools: db_pools };
```

## When to Use Each Method

### `.read()` - For Read Operations

Use for queries that:

- Don't modify data (SELECT without FOR UPDATE)
- Can tolerate slight staleness (eventual consistency)
- Benefit from load distribution

Examples: user listings, analytics, dashboards, search

### `.write()` - For Write Operations

Use for operations that:

- Modify data (INSERT, UPDATE, DELETE)
- Require transactions
- Need locking reads (SELECT FOR UPDATE)
- Require read-after-write consistency

Examples: creating records, updates, deletes, transactions

## Architecture

``` text
       ┌─────────────┐
       │ Named Pools │ (optional)
       └──────┬──────┘
              │
              ▼
       ┌─────────┐
       │ DbPools │
       └────┬────┘
            │
       ┌────┴────┐
       │         │
       ▼         ▼
┌─────────┐ ┌─────────┐
│ Primary │ │ Replica │ (optional)
└─────────┘ └─────────┘
```

## Real-World Use Cases

This library is used in production by:

- [outlet-postgres](https://github.com/doublewordai/outlet-postgres) - HTTP request/response logging middleware
- [fusillade](https://github.com/doublewordai/fusillade) - LLM request batching daemon
- [dwctl](https://github.com/doublewordai/control-layer) - Observability and analytics platform

## License

This project is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Running Tests

The default test suite is offline: it does not require PostgreSQL or `DATABASE_URL`. Database-dependent tests are explicitly ignored, so both SQLx configurations can be checked with:

``` bash
unset DATABASE_URL
cargo test --features with-named-pools
cargo test --no-default-features --features with-sqlx-0_9,with-named-pools
```

To run only the database-dependent tests, start PostgreSQL and set `DATABASE_URL` explicitly:

``` bash
# Start PostgreSQL (using Docker)
docker run -d \
  -p 5432:5432 \
  -e POSTGRES_PASSWORD=password \
  -e POSTGRES_DB=test \
  --name sqlx-pool-registry-test-db \
  postgres:16

# The PostgreSQL role must have permission to create databases.
export DATABASE_URL=postgresql://postgres:password@localhost:5432/test

# Run only tests that require PostgreSQL.
cargo test --features with-named-pools -- --ignored
cargo test --no-default-features --features with-sqlx-0_9,with-named-pools -- --ignored

# Clean up
docker stop sqlx-pool-registry-test-db
docker rm sqlx-pool-registry-test-db
```

For the release-equivalent full suite, replace `-- --ignored` with `-- --include-ignored`. The database tests use `#[sqlx::test]`, which creates isolated test databases; this is why the role in `DATABASE_URL` needs `CREATE DATABASE`. An unset URL is fine for the default suite, while an unset or unreachable URL makes an explicit database-test run fail.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

### Commit Convention

This project uses [Conventional Commits](https://www.conventionalcommits.org/). Please format your commits as:

- `feat:` New features
- `fix:` Bug fixes
- `docs:` Documentation changes
- `test:` Test additions or modifications
- `refactor:` Code refactoring
- `perf:` Performance improvements
- `chore:` Build process or tooling changes

Example: `feat: add support for connection timeout configuration`
