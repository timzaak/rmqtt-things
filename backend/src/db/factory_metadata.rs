//! Factory metadata repository (support-multiple-device feature, design §5.1).
//!
//! Backs the production-line write path and the admin/device read path for
//! factory metadata. Four tables back this module; see migration
//! `20260718000001_add_factory_metadata.sql`.
//!
//! Behavioural invariants enforced here:
//! - `upsert_component` captures the before-snapshot inside the same tx and
//!   writes a `factory_metadata_change_log` row only on overwrite (R5).
//! - `replace_associations` is a full-replace upsert and writes NO change log
//!   (R5 scopes the log to metadata overwrites only).
//! - `get_device_view` left-joins associations with component metadata so rows
//!   whose metadata has not arrived yet surface with `None` fields (R3,
//!   out-of-order normal). Returns `None` only when the device has neither
//!   associations nor device-level metadata (→ handler 404).

use crate::db::models::{FactoryComponentMetadata, FactoryMetadataChangeLog};
use serde_json::{Value as JsonValue, json};
use sqlx::{PgPool, Postgres, QueryBuilder, Row};

/// Outcome of `FactoryMetadataRepo::upsert_component`.
///
/// `Overwritten.before` carries the pre-overwrite snapshot (always `Some` in
/// practice when the variant is `Overwritten`, kept `Option` for forward
/// compatibility with future schema-driven edge cases). Currently unused by
/// callers — the change-log row written inside the tx is the side-effect the
/// handler relies on — but retained so future audit hooks can read the
/// in-memory snapshot without a DB round-trip.
#[derive(Debug, Clone)]
pub enum UpsertOutcome {
    Created,
    #[allow(dead_code)]
    Overwritten {
        before: Option<JsonValue>,
    },
}

/// Input for `FactoryMetadataRepo::replace_associations`. `component_type` is
/// an optional hint carried at association time; the metadata table's value
/// takes precedence in the merged view.
#[derive(Debug, Clone)]
pub struct ComponentAssociationInput {
    pub component_sn: String,
    pub component_type: Option<String>,
}

/// Merged-view row for a single device. Mirrors the left-join of
/// `factory_component_association` (always present for a device's component)
/// with `factory_component_metadata` (may be absent when metadata has not
/// arrived yet — the `Option` fields are then `None`).
///
/// Carries both `assoc_type` (from the association row) and `meta_type` (from
/// the metadata row) so handlers can apply the "metadata table takes
/// precedence" rule without a second round-trip.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FactoryDeviceViewRow {
    pub component_sn: String,
    pub assoc_type: Option<String>,
    pub meta_type: Option<String>,
    pub metadata: Option<JsonValue>,
    pub file_attachments: Option<JsonValue>,
    pub updated_at: Option<time::OffsetDateTime>,
}

#[derive(Clone)]
pub struct FactoryMetadataRepo {
    pool: PgPool,
}

impl FactoryMetadataRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Upsert a component's factory metadata.
    ///
    /// Within a single transaction: read the existing row (before-snapshot),
    /// `INSERT ... ON CONFLICT DO UPDATE`, and — only when a before-snapshot
    /// existed (i.e. an overwrite happened) — append a
    /// `factory_metadata_change_log` row carrying the before/after JSONB
    /// snapshots (design §5.1, R5).
    pub async fn upsert_component(
        &self,
        component_sn: &str,
        component_type: &str,
        metadata: &JsonValue,
        file_attachments: &JsonValue,
    ) -> Result<UpsertOutcome, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        // Read the before-snapshot (None on first report). Plain SELECT is
        // sufficient: the subsequent ON CONFLICT upsert serialises concurrent
        // writers on the component_sn PK; the worst case under concurrency is a
        // log row whose `before` reflects a slightly earlier committed state,
        // which still satisfies the "overwrite is logged" invariant.
        let before: Option<FactoryComponentMetadata> =
            sqlx::query_as::<_, FactoryComponentMetadata>(
                r#"
            SELECT component_sn, component_type, metadata, file_attachments, updated_at, created_at
            FROM factory_component_metadata
            WHERE component_sn = $1
            "#,
            )
            .bind(component_sn)
            .fetch_optional(&mut *tx)
            .await?;

        // RETURNING gives the authoritative after-snapshot (updated_at is set
        // by the DB).
        let after: FactoryComponentMetadata = sqlx::query_as::<_, FactoryComponentMetadata>(
            r#"
            INSERT INTO factory_component_metadata (component_sn, component_type, metadata, file_attachments, updated_at)
            VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP)
            ON CONFLICT (component_sn) DO UPDATE SET
                component_type   = EXCLUDED.component_type,
                metadata         = EXCLUDED.metadata,
                file_attachments = EXCLUDED.file_attachments,
                updated_at       = CURRENT_TIMESTAMP
            RETURNING component_sn, component_type, metadata, file_attachments, updated_at, created_at
            "#,
        )
        .bind(component_sn)
        .bind(component_type)
        .bind(metadata)
        .bind(file_attachments)
        .fetch_one(&mut *tx)
        .await?;

        let outcome = match &before {
            None => UpsertOutcome::Created,
            Some(before_row) => {
                // Write the change log row with before/after JSONB snapshots.
                // `after` snapshot includes component_type/metadata/file_attachments/
                // updated_at per design §4.3.2.
                let before_json = json!({
                    "component_type":   before_row.component_type,
                    "metadata":         &before_row.metadata,
                    "file_attachments": &before_row.file_attachments,
                    "updated_at":       before_row.updated_at,
                });
                let after_json = json!({
                    "component_type":   &after.component_type,
                    "metadata":         &after.metadata,
                    "file_attachments": &after.file_attachments,
                    "updated_at":       after.updated_at,
                });
                sqlx::query(
                    r#"
                    INSERT INTO factory_metadata_change_log (component_sn, before, after, actor)
                    VALUES ($1, $2, $3, 'factory')
                    "#,
                )
                .bind(component_sn)
                .bind(&before_json)
                .bind(&after_json)
                .execute(&mut *tx)
                .await?;
                UpsertOutcome::Overwritten {
                    before: Some(before_json),
                }
            }
        };

        tx.commit().await?;
        Ok(outcome)
    }

    /// Full-replace upsert of a device's component associations.
    ///
    /// Deletes associations not present in `components`, then upserts each item
    /// in `components`. Writes NO change log (R5 scopes the log to metadata
    /// overwrites; association change history is out of P0 scope).
    ///
    /// When `components` is empty this deletes all existing associations for the
    /// device (full-replace-to-empty semantics).
    pub async fn replace_associations(
        &self,
        device_sn: &str,
        components: &[ComponentAssociationInput],
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        if components.is_empty() {
            sqlx::query(
                r#"
                DELETE FROM factory_component_association
                WHERE device_sn = $1
                "#,
            )
            .bind(device_sn)
            .execute(&mut *tx)
            .await?;
        } else {
            // Delete associations whose component_sn is not in the incoming
            // list, then upsert each item. Built with QueryBuilder so the
            // variadic NOT IN binding is type-safe.
            let sns: Vec<&str> = components.iter().map(|c| c.component_sn.as_str()).collect();
            let mut delete_builder: QueryBuilder<'_, Postgres> =
                QueryBuilder::new("DELETE FROM factory_component_association WHERE device_sn = ");
            delete_builder.push_bind(device_sn);
            delete_builder.push(" AND component_sn NOT IN (");
            let mut separated = delete_builder.separated(", ");
            for sn in &sns {
                separated.push_bind(*sn);
            }
            separated.push_unseparated(")");
            delete_builder.build().execute(&mut *tx).await?;

            // Upsert each incoming association.
            for item in components {
                sqlx::query(
                    r#"
                    INSERT INTO factory_component_association (device_sn, component_sn, component_type, updated_at)
                    VALUES ($1, $2, $3, CURRENT_TIMESTAMP)
                    ON CONFLICT (device_sn, component_sn) DO UPDATE SET
                        component_type = EXCLUDED.component_type,
                        updated_at     = CURRENT_TIMESTAMP
                    "#,
                )
                .bind(device_sn)
                .bind(&item.component_sn)
                .bind(&item.component_type)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;
        Ok(())
    }

    /// Query a device's merged factory view.
    ///
    /// Left-joins `factory_component_association` with
    /// `factory_component_metadata`: components whose metadata has not arrived
    /// yet surface with `None` metadata/file_attachments/updated_at (R3). If
    /// the device has no associations, falls back to checking
    /// `factory_device_metadata`; returns `None` (→ handler 404) only when
    /// neither has any row for the device.
    pub async fn get_device_view(
        &self,
        device_sn: &str,
    ) -> Result<Option<Vec<FactoryDeviceViewRow>>, sqlx::Error> {
        let rows: Vec<FactoryDeviceViewRow> = sqlx::query_as::<_, FactoryDeviceViewRow>(
            r#"
            SELECT
              a.component_sn                                AS component_sn,
              a.component_type                             AS assoc_type,
              m.component_type                             AS meta_type,
              m.metadata                                   AS metadata,
              m.file_attachments                           AS file_attachments,
              m.updated_at                                 AS updated_at
            FROM factory_component_association a
            LEFT JOIN factory_component_metadata m
              ON m.component_sn = a.component_sn
            WHERE a.device_sn = $1
            "#,
        )
        .bind(device_sn)
        .fetch_all(&self.pool)
        .await?;

        if !rows.is_empty() {
            return Ok(Some(rows));
        }

        // No associations — check whether the device has device-level metadata.
        // If yes, return an empty components list (Some, empty); if no, None.
        let device_meta_present: bool = sqlx::query(
            r#"
            SELECT EXISTS(
              SELECT 1 FROM factory_device_metadata WHERE device_sn = $1
            ) AS present
            "#,
        )
        .bind(device_sn)
        .fetch_one(&self.pool)
        .await?
        .get::<bool, _>("present");

        if device_meta_present {
            Ok(Some(Vec::new()))
        } else {
            Ok(None)
        }
    }

    /// Time-descending paginated query of a component's change log.
    ///
    /// `page` is 1-based. Returns `(rows, total)`; `total` is the row count
    /// matching `component_sn` (used by handlers to populate a
    /// `PaginatedResponse`).
    pub async fn query_change_log(
        &self,
        component_sn: &str,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<FactoryMetadataChangeLog>, u64), sqlx::Error> {
        let page = page.max(1) as i64;
        let page_size = page_size.max(1) as i64;
        let offset = (page - 1) * page_size;

        let rows: Vec<FactoryMetadataChangeLog> = sqlx::query_as::<_, FactoryMetadataChangeLog>(
            r#"
            SELECT id, component_sn, before, after, actor, created_at
            FROM factory_metadata_change_log
            WHERE component_sn = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(component_sn)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let total: i64 = sqlx::query(
            r#"
            SELECT COUNT(*) AS count
            FROM factory_metadata_change_log
            WHERE component_sn = $1
            "#,
        )
        .bind(component_sn)
        .fetch_one(&self.pool)
        .await?
        .get::<i64, _>("count");

        Ok((rows, total as u64))
    }
}
