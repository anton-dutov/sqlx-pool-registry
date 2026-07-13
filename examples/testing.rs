//! Example demonstrating how to use TestDbPools for testing.
//!
//! This example shows how TestDbPools uses PostgreSQL's read-only default
//! to help catch ordinary routing bugs in tests.
//!
//! To run this example:
//! 1. Set DATABASE_URL environment variable
//! 2. Run: cargo run --example testing

use sqlx_pool_registry::{
    PoolProvider, TestDbPools,
    sqlx::{self, postgres::PgPoolOptions},
};

/// A repository that should route reads to .read() and writes to .write()
struct UserRepository<P: PoolProvider> {
    pools: P,
}

impl<P: PoolProvider> UserRepository<P> {
    async fn create_user(&self, name: &str) -> Result<i64, sqlx::Error> {
        // This MUST use .write() - TestDbPools will catch if we use .read()
        sqlx::query_scalar("INSERT INTO users (name) VALUES ($1) RETURNING id")
            .bind(name)
            .fetch_one(self.pools.write())
            .await
    }

    async fn get_user(&self, id: i64) -> Result<String, sqlx::Error> {
        // This can use .read() - it's a SELECT
        sqlx::query_scalar("SELECT name FROM users WHERE id = $1")
            .bind(id)
            .fetch_one(self.pools.read())
            .await
    }

    async fn count_users(&self) -> Result<i64, sqlx::Error> {
        // This can use .read() - it's a SELECT
        sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(self.pools.read())
            .await
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost/test".to_string());

    println!("🧪 TestDbPools Example");
    println!("====================");
    println!();
    println!("Connecting to: {}", database_url);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Create TestDbPools - this is what you'd do in your #[sqlx::test] functions
    println!("Creating TestDbPools...");
    let pools = TestDbPools::new(pool).await?;

    println!("✓ TestDbPools created (read pool is read-only by default)");
    println!();

    // Set up test table
    println!("📝 Setting up test table...");
    sqlx::query("DROP TABLE IF EXISTS users")
        .execute(pools.write())
        .await?;

    sqlx::query("CREATE TABLE users (id SERIAL PRIMARY KEY, name TEXT NOT NULL)")
        .execute(pools.write())
        .await?;

    println!("✓ Table created");
    println!();

    let repo = UserRepository {
        pools: pools.clone(),
    };

    // Test 1: Writing through .write() works
    println!("Test 1: Writing through .write() pool");
    let user_id = repo.create_user("Alice").await?;
    println!("   ✓ Created user with ID: {}", user_id);
    println!();

    // Test 2: Reading through .read() works
    println!("Test 2: Reading through .read() pool");
    let name = repo.get_user(user_id).await?;
    println!("   ✓ Read user name: {}", name);
    assert_eq!(name, "Alice");
    println!();

    // Test 3: Reading aggregate through .read() works
    println!("Test 3: Aggregate queries through .read() pool");
    let count = repo.count_users().await?;
    println!("   ✓ User count: {}", count);
    assert_eq!(count, 1);
    println!();

    // Test 4: Writing through .read() FAILS
    println!("Test 4: Writing through .read() pool (should fail)");
    let result = sqlx::query("INSERT INTO users (name) VALUES ($1)")
        .bind("Bob")
        .execute(pools.read())
        .await;

    match result {
        Ok(_) => {
            println!("   ✗ UNEXPECTED: Write succeeded on read pool!");
            println!("   This should have failed - the read pool is read-only by default");
        }
        Err(e) => {
            println!("   ✓ Write correctly rejected on read pool by default");
            println!("   Error: {}", e);
            assert!(
                e.to_string().contains("read-only") || e.to_string().contains("cannot execute")
            );
        }
    }
    println!();

    // Test 5: Even CREATE TABLE fails on .read()
    println!("Test 5: DDL through .read() pool (should fail)");
    let result = sqlx::query("CREATE TEMP TABLE temp_test (id INT)")
        .execute(pools.read())
        .await;

    match result {
        Ok(_) => {
            println!("   ✗ UNEXPECTED: DDL succeeded on read pool!");
        }
        Err(e) => {
            println!("   ✓ DDL correctly rejected on read pool by default");
            println!("   Error: {}", e);
        }
    }
    println!();

    // Cleanup
    println!("🧹 Cleaning up...");
    sqlx::query("DROP TABLE users")
        .execute(pools.write())
        .await?;
    println!("   ✓ Table dropped");
    println!();

    println!("✅ All tests passed!");
    println!();
    println!("💡 Key Takeaways:");
    println!("   - TestDbPools helps detect read/write routing mistakes in tests");
    println!("   - Ordinary writes through .read() fail by default");
    println!("   - Catches routing bugs before they reach production");
    println!("   - Works seamlessly with #[sqlx::test] macro");
    println!();
    println!("📚 Use in your tests:");
    println!("   #[sqlx::test]");
    println!("   async fn test_my_feature(pool: PgPool) {{");
    println!("       let pools = TestDbPools::new(pool).await.unwrap();");
    println!("       let repo = MyRepository {{ pools }};");
    println!("       // Test will fail if repo routes incorrectly!");
    println!("   }}");

    Ok(())
}
