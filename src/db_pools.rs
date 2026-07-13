#[cfg(feature = "with-deref")]
use std::ops::Deref;

use crate::{provider::PoolProvider, sqlx::PgPool};

/// Database pool abstraction supporting read replicas.
///
/// Wraps primary and optional replica pools, providing methods for
/// explicit read/write routing.
///
/// Use [`PoolProvider::read`] and [`PoolProvider::write`] for every database
/// query so its routing intent stays explicit. The legacy `Deref<Target =
/// PgPool>` compatibility path is available only with the `with-deref` feature.
///
/// # Examples
///
/// ## Single Pool Configuration
///
/// ```rust,no_run
/// use sqlx_pool_registry::sqlx::{self, PgPool};
/// use sqlx_pool_registry::DbPools;
///
/// # async fn example() -> Result<(), sqlx::Error> {
/// let pool = PgPool::connect("postgresql://localhost/db").await?;
/// let pools = DbPools::new(pool);
///
/// // Both read() and write() return the same pool
/// assert!(!pools.has_replica());
/// # Ok(())
/// # }
/// ```
///
/// ## Primary/Replica Configuration
///
/// ```rust,no_run
/// use sqlx_pool_registry::sqlx::{self, postgres::PgPoolOptions};
/// use sqlx_pool_registry::DbPools;
///
/// # async fn example() -> Result<(), sqlx::Error> {
/// let primary = PgPoolOptions::new()
///     .max_connections(5)
///     .connect("postgresql://primary/db")
///     .await?;
///
/// let replica = PgPoolOptions::new()
///     .max_connections(10)
///     .connect("postgresql://replica/db")
///     .await?;
///
/// let pools = DbPools::with_replica(primary, replica);
/// assert!(pools.has_replica());
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct DbPools {
    primary: PgPool,
    replica: Option<PgPool>,
}

impl DbPools {
    /// Create a new DbPools with only a primary pool.
    ///
    /// This is useful for development or when you don't have a read replica configured.
    /// All read and write operations will route to the primary pool.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use sqlx_pool_registry::sqlx::{self, PgPool};
    /// use sqlx_pool_registry::DbPools;
    ///
    /// # async fn example() -> Result<(), sqlx::Error> {
    /// let pool = PgPool::connect("postgresql://localhost/db").await?;
    /// let pools = DbPools::new(pool);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(primary: PgPool) -> Self {
        Self {
            primary,
            replica: None,
        }
    }

    /// Create a new DbPools with primary and replica pools.
    ///
    /// Read operations will route to the replica pool for load distribution,
    /// while write operations always use the primary pool.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use sqlx_pool_registry::sqlx::{self, postgres::PgPoolOptions};
    /// use sqlx_pool_registry::DbPools;
    ///
    /// # async fn example() -> Result<(), sqlx::Error> {
    /// let primary = PgPoolOptions::new()
    ///     .max_connections(5)
    ///     .connect("postgresql://primary/db")
    ///     .await?;
    ///
    /// let replica = PgPoolOptions::new()
    ///     .max_connections(10)
    ///     .connect("postgresql://replica/db")
    ///     .await?;
    ///
    /// let pools = DbPools::with_replica(primary, replica);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_replica(primary: PgPool, replica: PgPool) -> Self {
        Self {
            primary,
            replica: Some(replica),
        }
    }

    /// Get the primary pool.
    ///
    /// Unlike [`write`](PoolProvider::write), this method describes the physical
    /// pool topology rather than the operation being routed. Both methods return
    /// the same pool for `DbPools`.
    pub fn primary(&self) -> &PgPool {
        &self.primary
    }

    /// Get the configured replica pool, if one exists.
    ///
    /// This method does not fall back to the primary pool. Use
    /// [`read`](PoolProvider::read) when a routable read pool is required.
    pub fn replica(&self) -> Option<&PgPool> {
        self.replica.as_ref()
    }

    /// Check if a replica pool is configured.
    ///
    /// Returns `true` if a replica pool was provided via [`with_replica`](Self::with_replica).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use sqlx_pool_registry::sqlx::{self, PgPool};
    /// use sqlx_pool_registry::DbPools;
    ///
    /// # async fn example() -> Result<(), sqlx::Error> {
    /// let pool = PgPool::connect("postgresql://localhost/db").await?;
    /// let pools = DbPools::new(pool);
    /// assert!(!pools.has_replica());
    /// # Ok(())
    /// # }
    /// ```
    pub fn has_replica(&self) -> bool {
        self.replica().is_some()
    }

    /// Close all database connections.
    ///
    /// Closes both primary and replica pools (if configured).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use sqlx_pool_registry::sqlx::{self, PgPool};
    /// use sqlx_pool_registry::DbPools;
    ///
    /// # async fn example() -> Result<(), sqlx::Error> {
    /// let pool = PgPool::connect("postgresql://localhost/db").await?;
    /// let pools = DbPools::new(pool);
    /// pools.close().await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn close(&self) {
        self.primary().close().await;
        if let Some(replica) = self.replica() {
            replica.close().await;
        }
    }
}

impl PoolProvider for DbPools {
    fn read(&self) -> &PgPool {
        self.replica().unwrap_or(self.primary())
    }

    fn write(&self) -> &PgPool {
        self.primary()
    }
}

/// Legacy compatibility: dereferences to the primary pool.
///
/// This implementation is available only with the `with-deref` feature and
/// will be removed in the next major version. `&*pools` always selects the
/// primary pool and bypasses read routing.
///
/// For database queries, always use [`PoolProvider::read`] or
/// [`PoolProvider::write`] instead.
///
/// ```rust,no_run
/// use sqlx_pool_registry::sqlx::{self, PgPool};
/// use sqlx_pool_registry::{DbPools, PoolProvider};
///
/// # async fn example() -> Result<(), sqlx::Error> {
/// let pools = DbPools::new(PgPool::connect("postgresql://localhost/db").await?);
///
/// // Legacy: `.fetch_one(&*pools)` always uses the primary pool.
/// let _: (i32,) = sqlx::query_as("SELECT 1").fetch_one(pools.read()).await?;
/// sqlx::query("INSERT INTO users (name) VALUES ('Alice')")
///     .execute(pools.write())
///     .await?;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "with-deref")]
#[cfg_attr(docsrs, doc(cfg(feature = "with-deref")))]
impl Deref for DbPools {
    type Target = PgPool;

    fn deref(&self) -> &Self::Target {
        self.primary()
    }
}
