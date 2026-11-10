//! Read-only check: can this build's migrations open an existing database?
//!
//! Migrations only move forward, so a daemon built before migration N cannot start on a
//! database that already applied N ("previously applied but is missing"). Installers and
//! rollbacks ask this before swapping binaries, so they never put such a daemon in charge.

use crate::PersistenceError;
use sqlx::{
    Row,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{collections::BTreeSet, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaCheck {
    /// Newest migration this build carries.
    pub known_max: i64,
    /// Newest migration already applied to the database (`None`: no database yet).
    pub applied_max: Option<i64>,
    /// Applied migrations this build does not know about — non-empty means it cannot open it.
    pub unknown_applied: Vec<i64>,
}

impl SchemaCheck {
    pub fn can_open(&self) -> bool {
        self.unknown_applied.is_empty()
    }
}

/// Compare applied versions against a known set (pure, for tests and callers).
pub fn compare(known: &BTreeSet<i64>, applied: &[i64]) -> SchemaCheck {
    let mut unknown: Vec<i64> = applied
        .iter()
        .copied()
        .filter(|v| !known.contains(v))
        .collect();
    unknown.sort_unstable();
    SchemaCheck {
        known_max: known.iter().next_back().copied().unwrap_or(0),
        applied_max: applied.iter().copied().max(),
        unknown_applied: unknown,
    }
}

/// Inspect `path` without creating, migrating or locking it for writes.
pub async fn schema_check(path: &Path) -> Result<SchemaCheck, PersistenceError> {
    let known: BTreeSet<i64> = sqlx::migrate!().iter().map(|m| m.version).collect();
    if !path.exists() {
        return Ok(compare(&known, &[]));
    }
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .create_if_missing(false)
        .busy_timeout(std::time::Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    let has_table =
        sqlx::query("SELECT 1 FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'")
            .fetch_optional(&pool)
            .await?
            .is_some();
    let applied: Vec<i64> = if has_table {
        sqlx::query("SELECT version FROM _sqlx_migrations WHERE success = 1")
            .fetch_all(&pool)
            .await?
            .iter()
            .map(|row| row.get::<i64, _>(0))
            .collect()
    } else {
        Vec::new()
    };
    pool.close().await;
    Ok(compare(&known, &applied))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_database_is_refused_by_older_build() {
        let known: BTreeSet<i64> = (1..=26).collect();
        let check = compare(&known, &(1..=27).collect::<Vec<_>>());
        assert!(!check.can_open());
        assert_eq!(check.unknown_applied, vec![27]);
        assert_eq!((check.known_max, check.applied_max), (26, Some(27)));
    }

    #[test]
    fn same_or_older_database_is_fine() {
        let known: BTreeSet<i64> = (1..=27).collect();
        assert!(compare(&known, &(1..=27).collect::<Vec<_>>()).can_open());
        assert!(compare(&known, &(1..=20).collect::<Vec<_>>()).can_open());
        assert!(compare(&known, &[]).can_open());
    }

    #[tokio::test]
    async fn reads_a_real_database_without_touching_it() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("agentflow.db");
        assert_eq!(
            schema_check(&path).await?.applied_max,
            None,
            "missing database is fine"
        );
        let db = crate::Store::open(&path).await?;
        drop(db);
        let check = schema_check(&path).await?;
        assert!(check.can_open());
        assert_eq!(check.applied_max, Some(check.known_max));
        Ok(())
    }
}
