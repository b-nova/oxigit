use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use lru::LruCache;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tokio::sync::Mutex;

use crate::db;
use crate::error::{OxigitError, Result};

/// Manages per-tenant SQLite database pools with LRU eviction.
///
/// Each tenant (organization) gets its own SQLite database at
/// `{data_dir}/tenants/{org_slug}/tenant.db` and its own git repo
/// directory at `{data_dir}/tenants/{org_slug}/repos/`.
///
/// The control plane database (users, auth, billing, orgs) is shared
/// across all tenants and accessed via `control_pool()`.
pub struct TenantPoolManager {
    control_pool: SqlitePool,
    data_dir: PathBuf,
    cache: Mutex<LruCache<String, SqlitePool>>,
    max_connections_per_tenant: u32,
}

impl TenantPoolManager {
    /// Create a new tenant pool manager.
    ///
    /// `max_cached` controls the LRU cache size — how many tenant pools
    /// are kept open simultaneously. Evicted pools are closed, freeing
    /// their file handles. A value of 50 with 3 connections each = 150
    /// file handles at peak.
    pub fn new(control_pool: SqlitePool, data_dir: PathBuf, max_cached: usize) -> Self {
        Self {
            control_pool,
            data_dir,
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(max_cached).expect("max_cached must be > 0"),
            )),
            max_connections_per_tenant: 3,
        }
    }

    /// Returns a reference to the control plane pool.
    pub fn control_pool(&self) -> &SqlitePool {
        &self.control_pool
    }

    /// Returns the data directory.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Get the tenant database pool for the given org slug.
    ///
    /// On cache hit, returns the existing pool. On cache miss, opens a
    /// new pool, runs tenant migrations, and inserts it into the LRU
    /// cache (potentially evicting the least-recently-used pool).
    pub async fn get_tenant_pool(&self, org_slug: &str) -> Result<SqlitePool> {
        // Fast path: check cache
        {
            let mut cache = self.cache.lock().await;
            if let Some(pool) = cache.get(org_slug) {
                return Ok(pool.clone());
            }
        }

        // Slow path: open new pool
        let tenant_dir = self.tenant_dir(org_slug);
        let db_path = tenant_dir.join("tenant.db");

        if !db_path.exists() {
            return Err(OxigitError::InvalidInput(format!(
                "Tenant '{}' has not been provisioned",
                org_slug
            )));
        }

        let pool = self.open_tenant_pool(&db_path).await?;

        // Run migrations to ensure schema is up-to-date
        db::run_tenant_migrations(&pool).await?;

        // Insert into cache, closing evicted pool if any
        let mut cache = self.cache.lock().await;
        if let Some((_, evicted)) = cache.push(org_slug.to_string(), pool.clone()) {
            evicted.close().await;
        }

        Ok(pool)
    }

    /// Provision a new tenant: create directory structure, database,
    /// and run initial migrations.
    pub async fn provision_tenant(&self, org_slug: &str) -> Result<()> {
        let tenant_dir = self.tenant_dir(org_slug);
        let repos_dir = tenant_dir.join("repos");

        // Create directory structure
        tokio::fs::create_dir_all(&repos_dir).await.map_err(|e| {
            OxigitError::InvalidInput(format!(
                "Failed to create tenant directory: {}",
                e
            ))
        })?;

        // Create and migrate tenant database
        let db_path = tenant_dir.join("tenant.db");
        let pool = self.open_tenant_pool(&db_path).await?;
        db::run_tenant_migrations(&pool).await?;

        // Insert into cache
        let mut cache = self.cache.lock().await;
        if let Some((_, evicted)) = cache.push(org_slug.to_string(), pool) {
            evicted.close().await;
        }

        Ok(())
    }

    /// Returns the filesystem path to a tenant's directory.
    pub fn tenant_dir(&self, org_slug: &str) -> PathBuf {
        self.data_dir.join("tenants").join(org_slug)
    }

    /// Returns the filesystem path to a tenant's repos directory.
    pub fn tenant_repos_dir(&self, org_slug: &str) -> PathBuf {
        self.tenant_dir(org_slug).join("repos")
    }

    async fn open_tenant_pool(&self, db_path: &Path) -> Result<SqlitePool> {
        let url = format!("sqlite:{}?mode=rwc", db_path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(self.max_connections_per_tenant)
            .connect(&url)
            .await?;
        Ok(pool)
    }
}
