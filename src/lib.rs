//! # sqlx_pool_registry
//!
//! A lightweight library for routing database operations to different SQLx PostgreSQL connection pools
//! based on whether they're read or write operations.
//!
//! This enables load distribution by routing read-heavy operations to read replicas while ensuring
//! write operations always go to the primary database.
//!
//! ## Features
//!
//! - **Zero-cost abstraction**: Trait-based design with no runtime overhead
//! - **Explicit routing**: Read/write intent stays visible at each database call site
//! - **Backward compatible**: `PgPool` implements `PoolProvider` for seamless integration
//! - **Flexible**: Use single pool or separate primary/replica pools
//! - **SQLx compatibility**: `with-sqlx-0_8` is enabled by default; select
//!   `with-sqlx-0_9` with default features disabled to use SQLx 0.9. Exactly
//!   one SQLx compatibility feature must be enabled. The selected crate is
//!   re-exported as [`sqlx`].
//! - **Named pools (optional)**: Group independent providers by name with the
//!   `with-named-pools` feature
//! - **Legacy `Deref` compatibility (optional)**: The `with-deref` feature
//!   restores `DbPools: Deref<Target = PgPool>` for migrations. It always
//!   selects the primary pool; use [`PoolProvider::read`] or
//!   [`PoolProvider::write`] for database queries instead.
//! - **Test helpers**: [`TestDbPools`] for testing with `#[sqlx::test]`
//! - **Well-tested**: Comprehensive test suite with replica routing verification
//!
//! ## Quick Start
//!
//! ### Single Pool (Development)
//!
//! ```rust,no_run
//! use sqlx_pool_registry::sqlx::{self, PgPool};
//! use sqlx_pool_registry::PoolProvider;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let pool = PgPool::connect("postgresql://localhost/mydb").await?;
//!
//! // PgPool implements PoolProvider automatically
//! let result: (i32,) = sqlx::query_as("SELECT 1")
//!     .fetch_one(pool.read())
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Read/Write Separation (Production)
//!
//! ```rust,no_run
//! use sqlx_pool_registry::sqlx::{self, postgres::PgPoolOptions};
//! use sqlx_pool_registry::{DbPools, PoolProvider};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let primary = PgPoolOptions::new()
//!     .max_connections(5)
//!     .connect("postgresql://primary-host/mydb")
//!     .await?;
//!
//! let replica = PgPoolOptions::new()
//!     .max_connections(10)
//!     .connect("postgresql://replica-host/mydb")
//!     .await?;
//!
//! let pools = DbPools::with_replica(primary, replica);
//!
//! // Reads go to replica
//! let users: Vec<(i32, String)> = sqlx::query_as("SELECT id, name FROM users")
//!     .fetch_all(pools.read())
//!     .await?;
//!
//! // Writes go to primary
//! sqlx::query("INSERT INTO users (name) VALUES ($1)")
//!     .bind("Alice")
//!     .execute(pools.write())
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Named Pools (Optional)
//!
//! Enable the `with-named-pools` feature to use `PoolRegistry`. A lookup
//! returns one independent provider and never changes registry-wide state.
//! The full example is available on the `PoolRegistry` API documentation when
//! the feature is enabled.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐
//! │   DbPools   │
//! └──────┬──────┘
//!        │
//!   ┌────┴────┐
//!   ↓         ↓
//! ┌─────┐  ┌─────────┐
//! │Primary│  │ Replica │ (optional)
//! └─────┘  └─────────┘
//! ```
//!
//! ## Generic Programming
//!
//! Make your types generic over `PoolProvider` to support both single and multi-pool configurations:
//!
//! ```rust
//! use sqlx_pool_registry::{sqlx, PoolProvider};
//!
//! struct Repository<P: PoolProvider> {
//!     pools: P,
//! }
//!
//! impl<P: PoolProvider> Repository<P> {
//!     async fn get_user(&self, id: i64) -> Result<String, sqlx::Error> {
//!         // Read from replica
//!         sqlx::query_scalar("SELECT name FROM users WHERE id = $1")
//!             .bind(id)
//!             .fetch_one(self.pools.read())
//!             .await
//!     }
//!
//!     async fn create_user(&self, name: &str) -> Result<i64, sqlx::Error> {
//!         // Write to primary
//!         sqlx::query_scalar("INSERT INTO users (name) VALUES ($1) RETURNING id")
//!             .bind(name)
//!             .fetch_one(self.pools.write())
//!             .await
//!     }
//! }
//! ```
//!
//! ## Testing
//!
//! Use [`TestDbPools`] with `#[sqlx::test]` to make ordinary write-through-read
//! routing mistakes fail during tests:
//!
//! ```rust,no_run
//! use sqlx_pool_registry::sqlx::{self, PgPool};
//! use sqlx_pool_registry::{PoolProvider, TestDbPools};
//!
//! #[sqlx::test]
//! async fn test_repository(pool: PgPool) {
//!     let pools = TestDbPools::new(pool).await.unwrap();
//!
//!     // Ordinary writes through .read() fail unless read-only mode is overridden
//!     let result = sqlx::query("INSERT INTO users VALUES (1)")
//!         .execute(pools.read())
//!         .await;
//!     assert!(result.is_err());
//! }
//! ```
//!
//! This helps catch routing bugs without needing a real replica database.
//! `TestDbPools` is a testing aid, not a security boundary: clients can override
//! PostgreSQL's read-only default for an individual transaction or session.

#[cfg(all(feature = "with-sqlx-0_8", feature = "with-sqlx-0_9"))]
compile_error!("features `with-sqlx-0_8` and `with-sqlx-0_9` are mutually exclusive");

#[cfg(not(any(feature = "with-sqlx-0_8", feature = "with-sqlx-0_9")))]
compile_error!("enable exactly one SQLx feature: `with-sqlx-0_8` or `with-sqlx-0_9`");

#[cfg(all(feature = "with-sqlx-0_8", not(feature = "with-sqlx-0_9")))]
pub extern crate sqlx_0_8 as sqlx;

#[cfg(all(feature = "with-sqlx-0_9", not(feature = "with-sqlx-0_8")))]
pub extern crate sqlx_0_9 as sqlx;

mod db_pools;
mod provider;
#[cfg(feature = "with-named-pools")]
mod registry;
mod test_db_pools;

pub use db_pools::DbPools;
pub use provider::PoolProvider;
#[cfg(feature = "with-named-pools")]
pub use registry::{PoolRegistry, UnknownPool};
pub use test_db_pools::TestDbPools;
