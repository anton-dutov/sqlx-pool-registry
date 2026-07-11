use crate::{db_pools::DbPools, provider::PoolProvider};
use std::{collections::HashMap, error::Error, fmt};

/// Error returned when a named pool is not registered.
///
/// Available with the `with-named-pools` feature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownPool {
    name: String,
}

impl UnknownPool {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
        }
    }

    /// Return the name that was not found.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for UnknownPool {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown pool `{}`", self.name)
    }
}

impl Error for UnknownPool {}

/// A registry of database pool providers keyed by name.
///
/// `PoolRegistry` is available with the `with-named-pools` feature. It stores
/// one homogeneous provider type and defaults to [`DbPools`]. Selecting a name
/// returns the provider for that name; the registry itself does not keep a
/// stateful "current" pool and does not implement [`PoolProvider`].
///
/// # Examples
///
/// ```rust
/// use sqlx_pool_registry::sqlx::{self, postgres::PgPoolOptions};
/// use sqlx_pool_registry::{DbPools, PoolProvider, PoolRegistry};
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let auth_primary = PgPoolOptions::new()
///     .connect_lazy("postgresql://localhost/auth")?;
/// let auth_replica = PgPoolOptions::new()
///     .connect_lazy("postgresql://localhost/auth_replica")?;
/// let analytics = PgPoolOptions::new()
///     .connect_lazy("postgresql://localhost/analytics")?;
///
/// let mut pools = PoolRegistry::new();
/// pools.insert("auth", DbPools::with_replica(auth_primary, auth_replica));
/// pools.insert("analytics", DbPools::new(analytics));
///
/// let auth = pools.try_get("auth")?;
/// let _primary = auth.primary();
/// let _replica = auth.replica(); // Option<&PgPool>, with no fallback
/// let _read = auth.read(); // Replica, or primary when no replica is configured
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct PoolRegistry<P: PoolProvider = DbPools> {
    pools: HashMap<String, P>,
}

impl<P: PoolProvider> PoolRegistry<P> {
    /// Create an empty pool registry.
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
        }
    }

    /// Insert a provider under `name`.
    ///
    /// If the name was already registered, the previous provider is replaced
    /// and returned.
    pub fn insert(&mut self, name: impl Into<String>, provider: P) -> Option<P> {
        self.pools.insert(name.into(), provider)
    }

    /// Get a provider by name.
    ///
    /// Names are matched exactly and case-sensitively.
    pub fn get(&self, name: &str) -> Option<&P> {
        self.pools.get(name)
    }

    /// Get a provider by name, returning [`UnknownPool`] when it is absent.
    pub fn try_get(&self, name: &str) -> Result<&P, UnknownPool> {
        self.get(name).ok_or_else(|| UnknownPool::new(name))
    }

    /// Return whether a provider is registered under `name`.
    pub fn contains_key(&self, name: &str) -> bool {
        self.pools.contains_key(name)
    }

    /// Return the number of registered providers.
    pub fn len(&self) -> usize {
        self.pools.len()
    }

    /// Return whether the registry contains no providers.
    pub fn is_empty(&self) -> bool {
        self.pools.is_empty()
    }

    /// Iterate over registered names and providers in unspecified order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &P)> {
        self.pools
            .iter()
            .map(|(name, provider)| (name.as_str(), provider))
    }
}

impl<P: PoolProvider> Default for PoolRegistry<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, P> FromIterator<(K, P)> for PoolRegistry<P>
where
    K: Into<String>,
    P: PoolProvider,
{
    fn from_iter<T: IntoIterator<Item = (K, P)>>(iter: T) -> Self {
        let mut registry = Self::new();
        registry.extend(iter);
        registry
    }
}

impl<K, P> Extend<(K, P)> for PoolRegistry<P>
where
    K: Into<String>,
    P: PoolProvider,
{
    fn extend<T: IntoIterator<Item = (K, P)>>(&mut self, iter: T) {
        self.pools.extend(
            iter.into_iter()
                .map(|(name, provider)| (name.into(), provider)),
        );
    }
}
