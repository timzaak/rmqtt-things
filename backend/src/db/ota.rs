use crate::db::models::OtaVersion;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use time::OffsetDateTime;

#[derive(Clone)]
pub struct OtaRepo {
    pool: PgPool,
}

impl OtaRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // 通用分页构建器
    fn add_pagination<'a>(builder: &mut QueryBuilder<'a, Postgres>, page: i64, page_size: i64) {
        let offset = (page - 1) * page_size;
        builder.push(" LIMIT ");
        builder.push_bind(page_size);
        builder.push(" OFFSET ");
        builder.push_bind(offset);
    }

    // Upsert device version
    pub async fn upsert_device_version(
        &self,
        product_id: &str,
        device_id: &str,
        key: &str,
        version: i32,
    ) -> anyhow::Result<()> {
        let now = OffsetDateTime::now_utc();
        sqlx::query(
            r#"
            INSERT INTO device_versions (product_id, device_id, key, version, last_updated_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $5)
            ON CONFLICT (product_id, device_id, key)
            DO UPDATE SET
                version = $4,
                last_updated_at = $5,
                updated_at = $5
            WHERE
                device_versions.version != $4
    AND
    (
        device_versions.last_updated_at IS NULL
        OR
        device_versions.last_updated_at < $5 - INTERVAL '10 minutes'
    )
            "#,
        )
        .bind(product_id)
        .bind(device_id)
        .bind(key)
        .bind(version)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // Get OTA update
    pub async fn get_ota_update(
        &self,
        product_id: &str,
        device_id: &str,
        key: &str,
        version: i32,
    ) -> anyhow::Result<Option<OtaVersion>> {
        let result = sqlx::query_as::<_, OtaVersion>(
            r#"
            SELECT id, product_id, key, version, max_version, min_version, file_key, log, device_ids, released_at, status, created_at, updated_at, bin_length, bin_md5
            FROM ota_versions
            WHERE product_id = $1
              AND key = $2
              AND status = 0
              AND $3 >= min_version
              AND (max_version IS NULL OR max_version > $3)
              AND released_at <= NOW()
              AND (device_ids IS NULL OR $4 = ANY(device_ids))
            ORDER BY version DESC
            LIMIT 1
            "#,
        )
        .bind(product_id)
        .bind(key)
        .bind(version)
        .bind(device_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    // Create OTA version
    pub async fn create_ota_version(
        &self,
        req: &crate::api::admin_models::CreateOtaVersionRequest,
    ) -> anyhow::Result<i32> {
        let version = self.parse_version_to_int(&req.version)?;
        let min_version = self.parse_version_to_int(&req.min_version)?;
        let max_version = req
            .max_version
            .as_ref()
            .map(|v| self.parse_version_to_int(v))
            .transpose()?;

        let row = sqlx::query(
            r#"
            INSERT INTO ota_versions (product_id, key, version, max_version, min_version, file_key, log, device_ids, released_at, status, bin_length, bin_md5)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), 0, $9, $10)
            RETURNING id
            "#,
        )
        .bind(&req.product_id)
        .bind(&req.key)
        .bind(version)
        .bind(max_version)
        .bind(min_version)
        .bind(&req.file_key)
        .bind(&req.log)
        .bind(&req.device_ids)
        .bind(req.bin_length)
        .bind(&req.bin_md5)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get("id"))
    }

    // Update OTA version
    pub async fn update_ota_version(
        &self,
        id: i64,
        req: &crate::api::admin_models::UpdateOtaVersionRequest,
    ) -> anyhow::Result<u64> {
        let mut builder: QueryBuilder<Postgres> =
            QueryBuilder::new("UPDATE ota_versions SET updated_at = NOW()");
        let mut updated = false;
        if let Some(min_version_str) = &req.min_version {
            let min_version = self.parse_version_to_int(min_version_str)?;
            builder.push(", min_version = ");
            builder.push_bind(min_version);
            updated = true;
        }

        if req.max_version.is_some() {
            let max_version = req
                .max_version
                .as_ref()
                .map(|v| self.parse_version_to_int(v))
                .transpose()?;
            builder.push(", max_version = ");
            builder.push_bind(max_version);
            updated = true;
        }

        if let Some(file_key) = &req.file_key {
            builder.push(", file_key = ");
            builder.push_bind(file_key);
            updated = true;
        }
        if let Some(log) = &req.log {
            builder.push(", log = ");
            builder.push_bind(log);
            updated = true;
        }
        if req.device_ids.is_some() {
            builder.push(", device_ids = ");
            builder.push_bind(&req.device_ids);
            updated = true;
        }

        if req.bin_length.is_some() {
            builder.push(", bin_length = ");
            builder.push_bind(req.bin_length);
            updated = true;
        }

        if req.bin_md5.is_some() {
            builder.push(", bin_md5 = ");
            builder.push_bind(&req.bin_md5);
            updated = true;
        }

        if !updated {
            return Ok(0);
        }
        builder.push(" WHERE id = ");
        builder.push_bind(id);

        let result = builder.build().execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    // Delete OTA version (soft delete)
    pub async fn delete_ota_version(&self, id: i64) -> anyhow::Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE ota_versions
            SET status = 1, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    // Get OTA version by ID
    pub async fn get_ota_version_by_id(&self, id: i32) -> anyhow::Result<Option<OtaVersion>> {
        let ota_version = sqlx::query_as::<_, OtaVersion>(
            "SELECT id, product_id, key, version, max_version, min_version, file_key, log, device_ids, released_at, status, created_at, updated_at, bin_length, bin_md5 FROM ota_versions WHERE id = $1 and status=0",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(ota_version)
    }

    // Query OTA versions
    pub async fn query_ota_versions(
        &self,
        product_id: Option<&str>,
        page: i64,
        page_size: i64,
    ) -> anyhow::Result<(Vec<OtaVersion>, i64)> {
        let mut query_builder = QueryBuilder::new(
            "SELECT id, product_id, key, version, max_version, min_version, file_key, log, device_ids, released_at, status, created_at, updated_at, bin_length, bin_md5 FROM ota_versions WHERE status = 0",
        );
        if let Some(product_id) = product_id {
            query_builder.push(" AND product_id = ");
            query_builder.push_bind(product_id);
        }
        query_builder.push(" ORDER BY updated_at DESC");
        Self::add_pagination(&mut query_builder, page, page_size);

        let versions = query_builder
            .build_query_as::<OtaVersion>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder =
            QueryBuilder::new("SELECT COUNT(*) as count FROM ota_versions WHERE status = 0");
        if let Some(product_id) = product_id {
            count_builder.push(" AND product_id = ");
            count_builder.push_bind(product_id);
        }
        let count_row = count_builder.build().fetch_one(&self.pool).await?;
        let total: i64 = count_row.get("count");
        Ok((versions, total))
    }

    // Helper function to parse version string "major.minor.patch" to integer
    fn parse_version_to_int(&self, version_str: &str) -> anyhow::Result<i32> {
        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() != 3 {
            return Err(anyhow::anyhow!("Version format must be major.minor.patch"));
        }
        let major = parts[0].parse::<i32>()?;
        let minor = parts[1].parse::<i32>()?;
        let patch = parts[2].parse::<i32>()?;

        if major > 99 || minor > 99 || patch > 999 {
            return Err(anyhow::anyhow!("Version parts are too large"));
        }

        Ok(major * 100_000 + minor * 1_000 + patch)
    }
}
