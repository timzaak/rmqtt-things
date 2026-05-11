use crate::db::models::CertIssue;
use sqlx::PgPool;

pub struct CertIssueRepo {
    pool: PgPool,
}

impl CertIssueRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, cert: &CertIssue) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO cert_issue (product_id, device_id, pub_cert, start_at, end_at, status)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(&cert.product_id)
        .bind(&cert.device_id)
        .bind(&cert.pub_cert)
        .bind(cert.start_at)
        .bind(cert.end_at)
        .bind(cert.status)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_device_id(
        &self,
        product_id: &str,
        device_id: &str,
    ) -> Result<Option<CertIssue>, sqlx::Error> {
        sqlx::query_as::<_, CertIssue>(
            "SELECT * FROM cert_issue WHERE product_id = $1 AND device_id = $2",
        )
        .bind(product_id)
        .bind(device_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn update_status(
        &self,
        product_id: &str,
        device_id: &str,
        status: i16,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE cert_issue SET status = $1 WHERE product_id = $2 AND device_id = $3")
            .bind(status)
            .bind(product_id)
            .bind(device_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Option<CertIssue>, sqlx::Error> {
        sqlx::query_as::<_, CertIssue>("SELECT * FROM cert_issue WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn list(
        &self,
        product_id: Option<String>,
        device_id: Option<String>,
        page: i64,
        page_size: i64,
    ) -> Result<Vec<CertIssue>, sqlx::Error> {
        let offset = (page - 1) * page_size;
        let certs = sqlx::query_as::<_, CertIssue>(
            r#"
            SELECT *
            FROM cert_issue
            WHERE ($1 IS NULL OR product_id = $1)
              AND ($2 IS NULL OR device_id = $2)
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(&product_id)
        .bind(&device_id)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(certs)
    }
}
