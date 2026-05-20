use crate::db::models::{Device, RegistrationSource};
use sqlx::PgPool;

pub struct DeviceRepo {
    pool: PgPool,
}

impl DeviceRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn upsert_auto(&self, product_id: &str, device_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO devices (product_id, device_id, registration_source)
            VALUES ($1, $2, $3)
            ON CONFLICT (product_id, device_id) DO NOTHING
            "#,
        )
        .bind(product_id)
        .bind(device_id)
        .bind(RegistrationSource::Auto)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn upsert_manual(
        &self,
        product_id: &str,
        device_id: &str,
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
        .bind(RegistrationSource::Manual)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn exists(&self, product_id: &str, device_id: &str) -> Result<bool, sqlx::Error> {
        let row: (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM devices WHERE product_id = $1 AND device_id = $2)",
        )
        .bind(product_id)
        .bind(device_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

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
