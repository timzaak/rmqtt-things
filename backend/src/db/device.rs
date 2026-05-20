#[cfg(test)]
use crate::db::models::Device;
use crate::db::models::RegistrationSource;
use sqlx::{PgPool, Row};

pub struct DeviceRepo {
    pool: PgPool,
}

impl DeviceRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Returns (device_exists, auto_provisioning) in a single DB round-trip.
    /// auto_provisioning is None when the product is not found.
    pub async fn admission_check(
        &self,
        product_id: &str,
        device_id: &str,
    ) -> Result<(bool, Option<bool>), sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT
              EXISTS(SELECT 1 FROM devices WHERE product_id = $1 AND device_id = $2) as device_exists,
              p.auto_provisioning
            FROM (SELECT $1::text as pid) const
            LEFT JOIN product p ON p.model_no = const.pid
            "#,
        )
        .bind(product_id)
        .bind(device_id)
        .fetch_one(&self.pool)
        .await?;
        Ok((row.get("device_exists"), row.get("auto_provisioning")))
    }

    pub async fn upsert(
        &self,
        product_id: &str,
        device_id: &str,
        source: RegistrationSource,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO devices (product_id, device_id, registration_source)
            VALUES ($1, $2, $3)
            ON CONFLICT (product_id, device_id) DO NOTHING
            "#,
        )
        .bind(product_id)
        .bind(device_id)
        .bind(source)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[cfg(test)]
    pub async fn find_by_product_and_device(
        &self,
        product_id: &str,
        device_id: &str,
    ) -> Result<Option<Device>, sqlx::Error> {
        sqlx::query_as::<_, Device>(
            "SELECT id, product_id, device_id, registration_source, created_at, updated_at FROM devices WHERE product_id = $1 AND device_id = $2",
        )
        .bind(product_id)
        .bind(device_id)
        .fetch_optional(&self.pool)
        .await
    }
}
