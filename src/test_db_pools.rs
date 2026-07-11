use crate::{
    provider::PoolProvider,
    sqlx::{self, PgPool},
};

/// Test pool provider with read-only replica enforcement.
///
/// This creates two separate connection pools from the same database:
/// - Primary pool for writes (normal permissions)
/// - Replica pool for reads (enforces `default_transaction_read_only = on`)
///
/// This ensures tests catch bugs where write operations are incorrectly
/// routed through `.read()`. PostgreSQL will reject writes with:
/// "cannot execute INSERT/UPDATE/DELETE in a read-only transaction"
///
/// # Usage with `#[sqlx::test]`
///
/// ```rust,no_run
/// use sqlx_pool_registry::sqlx::{self, PgPool};
/// use sqlx_pool_registry::{PoolProvider, TestDbPools};
///
/// #[sqlx::test]
/// async fn test_read_write_routing(pool: PgPool) {
///     let pools = TestDbPools::new(pool).await.unwrap();
///
///     // Write operations work on .write()
///     sqlx::query("CREATE TEMP TABLE users (id INT)")
///         .execute(pools.write())
///         .await
///         .expect("Write pool should allow writes");
///
///     // Write operations FAIL on .read()
///     let result = sqlx::query("INSERT INTO users VALUES (1)")
///         .execute(pools.read())
///         .await;
///     assert!(result.is_err(), "Read pool should reject writes");
///
///     // Read operations work on .read()
///     let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
///         .fetch_one(pools.read())
///         .await
///         .expect("Read pool should allow reads");
/// }
/// ```
///
/// # Why This Matters
///
/// Without this test helper, you might accidentally route write operations through
/// `.read()` and not catch the bug until production when you have an actual replica
/// with replication lag. This helper makes the bug obvious immediately in tests.
///
/// # Example
///
/// ```rust,no_run
/// use sqlx_pool_registry::sqlx::{self, PgPool};
/// use sqlx_pool_registry::{PoolProvider, TestDbPools};
///
/// struct Repository<P: PoolProvider> {
///     pools: P,
/// }
///
/// impl<P: PoolProvider> Repository<P> {
///     async fn get_user(&self, id: i64) -> Result<String, sqlx::Error> {
///         sqlx::query_scalar("SELECT name FROM users WHERE id = $1")
///             .bind(id)
///             .fetch_one(self.pools.read())
///             .await
///     }
///
///     async fn create_user(&self, name: &str) -> Result<i64, sqlx::Error> {
///         sqlx::query_scalar("INSERT INTO users (name) VALUES ($1) RETURNING id")
///             .bind(name)
///             .fetch_one(self.pools.write())
///             .await
///     }
/// }
///
/// #[sqlx::test]
/// async fn test_repository_routing(pool: PgPool) {
///     let pools = TestDbPools::new(pool).await.unwrap();
///     let repo = Repository { pools };
///
///     // Test will fail if create_user incorrectly uses .read()
///     sqlx::query("CREATE TEMP TABLE users (id SERIAL PRIMARY KEY, name TEXT)")
///         .execute(repo.pools.write())
///         .await
///         .unwrap();
///
///     let user_id = repo.create_user("Alice").await.unwrap();
///     let name = repo.get_user(user_id).await.unwrap();
///     assert_eq!(name, "Alice");
/// }
/// ```
#[derive(Clone, Debug)]
pub struct TestDbPools {
    primary: PgPool,
    replica: PgPool,
}

impl TestDbPools {
    /// Create test pools from a single database pool.
    ///
    /// This creates:
    /// - A primary pool (clone of input) for writes
    /// - A replica pool (new connection) configured as read-only
    ///
    /// The replica pool enforces `default_transaction_read_only = on`,
    /// so any write operations will fail with a PostgreSQL error.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use sqlx_pool_registry::sqlx::{self, PgPool};
    /// use sqlx_pool_registry::TestDbPools;
    ///
    /// # async fn example(pool: PgPool) -> Result<(), sqlx::Error> {
    /// let pools = TestDbPools::new(pool).await?;
    ///
    /// // Now you have pools that enforce read/write separation
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(pool: PgPool) -> Result<Self, sqlx::Error> {
        use crate::sqlx::postgres::PgPoolOptions;

        let primary = pool.clone();

        // Create a separate pool with read-only enforcement
        let replica = PgPoolOptions::new()
            .max_connections(pool.options().get_max_connections())
            .after_connect(|conn, _meta| {
                Box::pin(async move {
                    // Set all transactions to read-only by default
                    sqlx::query("SET default_transaction_read_only = on")
                        .execute(&mut *conn)
                        .await?;
                    Ok(())
                })
            })
            .connect_with(pool.connect_options().as_ref().clone())
            .await?;

        Ok(Self { primary, replica })
    }
}

impl PoolProvider for TestDbPools {
    fn read(&self) -> &PgPool {
        &self.replica
    }

    fn write(&self) -> &PgPool {
        &self.primary
    }
}
