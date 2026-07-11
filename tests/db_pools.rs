#[cfg(all(feature = "with-sqlx-0_8", not(feature = "with-sqlx-0_9")))]
extern crate sqlx_0_8 as sqlx;
#[cfg(all(feature = "with-sqlx-0_9", not(feature = "with-sqlx-0_8")))]
extern crate sqlx_0_9 as sqlx;

mod common;

use common::lazy_pool;
use registry_sqlx::{postgres::PgPoolOptions, PgPool};
use sqlx_pool_registry::sqlx as registry_sqlx;
use sqlx_pool_registry::{DbPools, PoolProvider};

#[tokio::test]
async fn test_dbpools_topology_accessors() {
    let primary_only = DbPools::new(lazy_pool(1));

    assert_eq!(primary_only.primary().options().get_max_connections(), 1);
    assert!(primary_only.replica().is_none());
    assert!(std::ptr::eq(primary_only.primary(), primary_only.read()));
    assert!(std::ptr::eq(primary_only.primary(), primary_only.write()));

    let with_replica = DbPools::with_replica(lazy_pool(2), lazy_pool(3));

    assert_eq!(with_replica.primary().options().get_max_connections(), 2);
    assert_eq!(
        with_replica
            .replica()
            .unwrap()
            .options()
            .get_max_connections(),
        3
    );
    assert!(std::ptr::eq(
        with_replica.replica().unwrap(),
        with_replica.read()
    ));
    assert!(std::ptr::eq(with_replica.primary(), with_replica.write()));
}

/// Helper to create a test database and return its pool and name
async fn create_test_db(admin_pool: &PgPool, suffix: &str) -> (PgPool, String) {
    let db_name = format!("test_dbpools_{}", suffix);

    // Clean up if exists
    registry_sqlx::query(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1",
    )
    .bind(&db_name)
    .execute(admin_pool)
    .await
    .ok();
    execute_dynamic_ddl(admin_pool, format!("DROP DATABASE IF EXISTS {}", db_name))
        .await
        .unwrap();

    // Create fresh database
    execute_dynamic_ddl(admin_pool, format!("CREATE DATABASE {}", db_name))
        .await
        .unwrap();

    // Connect to it
    let url = build_test_url(&db_name);
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .unwrap();

    // Create a marker table to identify which database we're connected to
    registry_sqlx::query("CREATE TABLE db_marker (name TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    registry_sqlx::query("INSERT INTO db_marker VALUES ($1)")
        .bind(&db_name)
        .execute(&pool)
        .await
        .unwrap();

    (pool, db_name)
}

async fn drop_test_db(admin_pool: &PgPool, db_name: &str) {
    registry_sqlx::query(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1",
    )
    .bind(db_name)
    .execute(admin_pool)
    .await
    .ok();
    execute_dynamic_ddl(admin_pool, format!("DROP DATABASE IF EXISTS {}", db_name))
        .await
        .ok();
}

#[cfg(all(feature = "with-sqlx-0_8", not(feature = "with-sqlx-0_9")))]
async fn execute_dynamic_ddl(
    pool: &PgPool,
    statement: String,
) -> Result<registry_sqlx::postgres::PgQueryResult, registry_sqlx::Error> {
    registry_sqlx::raw_sql(&statement).execute(pool).await
}

#[cfg(all(feature = "with-sqlx-0_9", not(feature = "with-sqlx-0_8")))]
async fn execute_dynamic_ddl(
    pool: &PgPool,
    statement: String,
) -> Result<registry_sqlx::postgres::PgQueryResult, registry_sqlx::Error> {
    registry_sqlx::raw_sql(registry_sqlx::AssertSqlSafe(statement))
        .execute(pool)
        .await
}

fn build_test_url(database: &str) -> String {
    if let Ok(base_url) = std::env::var("DATABASE_URL") {
        if let Ok(mut url) = url::Url::parse(&base_url) {
            url.set_path(&format!("/{}", database));
            return url.to_string();
        }
    }
    format!("postgres://postgres:password@localhost:5432/{}", database)
}

#[registry_sqlx::test]
#[ignore = "requires PostgreSQL via DATABASE_URL"]
async fn test_dbpools_without_replica(pool: PgPool) {
    let db_pools = DbPools::new(pool.clone());

    // Without replica, read() should return primary
    assert!(!db_pools.has_replica());

    // Both read and write should work
    let read_result: (i32,) = registry_sqlx::query_as("SELECT 1")
        .fetch_one(db_pools.read())
        .await
        .unwrap();
    assert_eq!(read_result.0, 1);

    let write_result: (i32,) = registry_sqlx::query_as("SELECT 2")
        .fetch_one(db_pools.write())
        .await
        .unwrap();
    assert_eq!(write_result.0, 2);

    // Deref should also work
    let deref_result: (i32,) = registry_sqlx::query_as("SELECT 3")
        .fetch_one(&*db_pools)
        .await
        .unwrap();
    assert_eq!(deref_result.0, 3);
}

#[registry_sqlx::test]
#[ignore = "requires PostgreSQL via DATABASE_URL"]
async fn test_dbpools_with_replica_routes_correctly(_pool: PgPool) {
    // Create admin connection to postgres database
    let admin_url = build_test_url("postgres");
    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&admin_url)
        .await
        .unwrap();

    // Create two separate databases to simulate primary and replica
    let (primary_pool, primary_name) = create_test_db(&admin_pool, "primary").await;
    let (replica_pool, replica_name) = create_test_db(&admin_pool, "replica").await;

    let db_pools = DbPools::with_replica(primary_pool.clone(), replica_pool.clone());
    assert!(db_pools.has_replica());

    // read() should return replica
    let read_marker: (String,) = registry_sqlx::query_as("SELECT name FROM db_marker")
        .fetch_one(db_pools.read())
        .await
        .unwrap();
    assert_eq!(
        read_marker.0, replica_name,
        "read() should route to replica"
    );

    // write() should return primary
    let write_marker: (String,) = registry_sqlx::query_as("SELECT name FROM db_marker")
        .fetch_one(db_pools.write())
        .await
        .unwrap();
    assert_eq!(
        write_marker.0, primary_name,
        "write() should route to primary"
    );

    // Deref should return primary
    let deref_marker: (String,) = registry_sqlx::query_as("SELECT name FROM db_marker")
        .fetch_one(&*db_pools)
        .await
        .unwrap();
    assert_eq!(
        deref_marker.0, primary_name,
        "deref should route to primary"
    );

    // Cleanup
    primary_pool.close().await;
    replica_pool.close().await;
    drop_test_db(&admin_pool, &primary_name).await;
    drop_test_db(&admin_pool, &replica_name).await;
}

#[tokio::test]
async fn test_dbpools_close() {
    let primary = lazy_pool(1);
    let replica = lazy_pool(2);
    let db_pools = DbPools::with_replica(primary.clone(), replica.clone());

    db_pools.close().await;

    assert!(primary.is_closed());
    assert!(replica.is_closed());
}

#[registry_sqlx::test]
#[ignore = "requires PostgreSQL via DATABASE_URL"]
async fn test_pgpool_implements_pool_provider(pool: PgPool) {
    // PgPool should implement PoolProvider
    assert_eq!(pool.read() as *const _, pool.write() as *const _);

    // Should be able to use it the same way
    let result: (i32,) = registry_sqlx::query_as("SELECT 1")
        .fetch_one(pool.read())
        .await
        .unwrap();
    assert_eq!(result.0, 1);
}
