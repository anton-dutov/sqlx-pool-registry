use crate::sqlx::PgPool;

/// Trait for providing database pools with read/write routing.
///
/// Implementations can provide separate read and write pools for load distribution,
/// or use a single pool for both operations.
///
/// # Thread Safety
///
/// Implementations must be `Clone`, `Send`, and `Sync` to work with async Rust
/// and be shared across tasks.
///
/// # When to Use Each Method
///
/// ## `.read()` - For Read Operations
///
/// Use for queries that:
/// - Don't modify data (SELECT without FOR UPDATE)
/// - Can tolerate slight staleness (eventual consistency)
/// - Benefit from load distribution
///
/// Examples: user listings, analytics, dashboards, search
///
/// ## `.write()` - For Write Operations
///
/// Use for operations that:
/// - Modify data (INSERT, UPDATE, DELETE)
/// - Require transactions
/// - Need locking reads (SELECT FOR UPDATE)
/// - Require read-after-write consistency
///
/// Examples: creating records, updates, deletes, transactions
///
/// # Example Implementation
///
/// ```
/// use sqlx_pool_registry::sqlx::{self, PgPool};
/// use sqlx_pool_registry::PoolProvider;
///
/// #[derive(Clone)]
/// struct MyPools {
///     primary: PgPool,
///     replica: Option<PgPool>,
/// }
///
/// impl PoolProvider for MyPools {
///     fn read(&self) -> &PgPool {
///         self.replica.as_ref().unwrap_or(&self.primary)
///     }
///
///     fn write(&self) -> &PgPool {
///         &self.primary
///     }
/// }
/// ```
pub trait PoolProvider: Clone + Send + Sync + 'static {
    /// Get a pool for read operations.
    ///
    /// May return a read replica for load distribution, or fall back to
    /// the primary pool if no replica is configured.
    fn read(&self) -> &PgPool;

    /// Get a pool for write operations.
    ///
    /// Should return the primary pool for operations that require writes,
    /// locking reads, or read-after-write consistency.
    fn write(&self) -> &PgPool;
}

/// Implement PoolProvider for PgPool for backward compatibility.
///
/// This allows existing code using `PgPool` directly to work with generic
/// code that accepts `impl PoolProvider` without any changes.
///
/// # Example
///
/// ```rust,no_run
/// use sqlx_pool_registry::sqlx::{self, PgPool};
/// use sqlx_pool_registry::PoolProvider;
///
/// async fn query_user<P: PoolProvider>(pools: &P, id: i64) -> Result<String, sqlx::Error> {
///     sqlx::query_scalar("SELECT name FROM users WHERE id = $1")
///         .bind(id)
///         .fetch_one(pools.read())
///         .await
/// }
///
/// # async fn example() -> Result<(), sqlx::Error> {
/// let pool = PgPool::connect("postgresql://localhost/db").await?;
///
/// // Works with PgPool directly
/// let name = query_user(&pool, 1).await?;
/// # Ok(())
/// # }
/// ```
impl PoolProvider for PgPool {
    fn read(&self) -> &PgPool {
        self
    }

    fn write(&self) -> &PgPool {
        self
    }
}
