#![cfg(feature = "with-named-pools")]

mod common;

use common::lazy_pool;
use sqlx_pool_registry::sqlx::PgPool;
use sqlx_pool_registry::{DbPools, PoolProvider, PoolRegistry};

#[tokio::test]
async fn test_pool_registry_named_lookup_and_iteration() {
    let mut registry = PoolRegistry::new();

    assert!(registry.is_empty());
    assert!(registry
        .insert("auth", DbPools::with_replica(lazy_pool(4), lazy_pool(5)))
        .is_none());
    assert!(registry
        .insert("analytics", DbPools::new(lazy_pool(6)))
        .is_none());

    assert_eq!(registry.len(), 2);
    assert!(!registry.is_empty());
    assert!(registry.contains_key("auth"));
    assert!(registry.contains_key("analytics"));
    assert!(!registry.contains_key("Auth"));

    let auth = registry.try_get("auth").unwrap();
    assert_eq!(auth.primary().options().get_max_connections(), 4);
    assert_eq!(auth.replica().unwrap().options().get_max_connections(), 5);
    assert!(std::ptr::eq(auth.replica().unwrap(), auth.read()));

    let analytics = registry.get("analytics").unwrap();
    assert!(analytics.replica().is_none());
    assert!(std::ptr::eq(analytics.primary(), analytics.read()));

    let mut names: Vec<_> = registry.iter().map(|(name, _)| name).collect();
    names.sort_unstable();
    assert_eq!(names, ["analytics", "auth"]);
}

#[test]
fn test_pool_registry_reports_unknown_pool() {
    let registry = PoolRegistry::<DbPools>::default();

    assert!(registry.get("missing").is_none());

    let error = registry.try_get("missing").unwrap_err();
    assert_eq!(error.name(), "missing");
    assert_eq!(error.to_string(), "unknown pool `missing`");

    let _: &dyn std::error::Error = &error;
}

#[tokio::test]
async fn test_pool_registry_replaces_duplicate_name() {
    let mut registry = PoolRegistry::new();

    assert!(registry
        .insert("auth", DbPools::new(lazy_pool(7)))
        .is_none());
    let replaced = registry.insert("auth", DbPools::new(lazy_pool(8))).unwrap();

    assert_eq!(replaced.primary().options().get_max_connections(), 7);
    assert_eq!(
        registry
            .get("auth")
            .unwrap()
            .primary()
            .options()
            .get_max_connections(),
        8
    );
    assert_eq!(registry.len(), 1);
}

#[tokio::test]
async fn test_pool_registry_is_generic_and_supports_iterators() {
    let mut registry: PoolRegistry<PgPool> = [("direct", lazy_pool(9))].into_iter().collect();
    registry.extend([("reporting".to_owned(), lazy_pool(10))]);

    let direct = registry.try_get("direct").unwrap();
    assert!(std::ptr::eq(direct, direct.read()));
    assert!(std::ptr::eq(direct, direct.write()));
    assert_eq!(
        registry
            .get("reporting")
            .unwrap()
            .options()
            .get_max_connections(),
        10
    );
}
