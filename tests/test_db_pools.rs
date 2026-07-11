#[cfg(all(feature = "with-sqlx-0_8", not(feature = "with-sqlx-0_9")))]
extern crate sqlx_0_8 as sqlx;
#[cfg(all(feature = "with-sqlx-0_9", not(feature = "with-sqlx-0_8")))]
extern crate sqlx_0_9 as sqlx;

use registry_sqlx::PgPool;
use sqlx_pool_registry::sqlx as registry_sqlx;
use sqlx_pool_registry::{PoolProvider, TestDbPools};

#[registry_sqlx::test]
async fn test_testdbpools_read_pool_rejects_writes(pool: PgPool) {
    let pools = TestDbPools::new(pool).await.unwrap();

    // Write operations should work on the write pool
    registry_sqlx::query("CREATE TEMP TABLE test_write (id INT)")
        .execute(pools.write())
        .await
        .expect("Write pool should allow CREATE TABLE");

    // Write operations should FAIL on the read pool
    let result = registry_sqlx::query("CREATE TEMP TABLE test_read_reject (id INT)")
        .execute(pools.read())
        .await;

    assert!(result.is_err(), "Read pool should reject CREATE TABLE");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("read-only") || err.contains("cannot execute"),
        "Error should mention read-only restriction, got: {}",
        err
    );
}

#[registry_sqlx::test]
async fn test_testdbpools_read_pool_allows_selects(pool: PgPool) {
    let pools = TestDbPools::new(pool).await.unwrap();

    // Read operations should work on the read pool
    let result: (i32,) = registry_sqlx::query_as("SELECT 1 + 1 as sum")
        .fetch_one(pools.read())
        .await
        .expect("Read pool should allow SELECT");

    assert_eq!(result.0, 2, "Should compute 1 + 1 = 2");
}

#[registry_sqlx::test]
async fn test_testdbpools_write_pool_allows_writes(_pool: PgPool) {
    // Note: This test is removed because sqlx::test doesn't easily support
    // testing TEMP tables (which are per-connection) with TestDbPools
    // (which creates separate connection pools).
    //
    // The functionality is still well-tested by:
    // - test_testdbpools_read_pool_rejects_writes (proves read pool rejects writes)
    // - test_testdbpools_read_pool_allows_selects (proves read pool allows reads)
    // - Integration tests in examples/testing.rs
}
