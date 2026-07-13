use sqlx_pool_registry::sqlx::{PgPool, postgres::PgPoolOptions};

pub fn lazy_pool(max_connections: u32) -> PgPool {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .connect_lazy("postgresql://postgres:password@localhost/test")
        .unwrap()
}
