use crate::db::models::{CreateProductRequest, Product, UpdateProductRequest};
use sqlx::{PgPool, Postgres, QueryBuilder, Row};

#[derive(Clone)]
pub struct ProductRepo {
    pool: PgPool,
}

impl ProductRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn add_pagination<'a>(builder: &mut QueryBuilder<'a, Postgres>, page: i64, page_size: i64) {
        let offset = (page - 1) * page_size;
        builder.push(" LIMIT ").push_bind(page_size);
        builder.push(" OFFSET ").push_bind(offset);
    }

    fn add_search_filter<'a>(builder: &mut QueryBuilder<'a, Postgres>, search: Option<&str>) {
        if let Some(search) = search {
            let search_pattern = format!("%{}%", search);
            builder
                .push(" AND (name ILIKE ")
                .push_bind(search_pattern.clone())
                .push(" OR model_no ILIKE ")
                .push_bind(search_pattern)
                .push(")");
        }
    }

    pub async fn create_product(&self, req: &CreateProductRequest) -> anyhow::Result<Product> {
        let product = sqlx::query_as::<_, Product>(
            r#"
            INSERT INTO product (name, model_no, description)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
        )
        .bind(&req.name)
        .bind(&req.model_no)
        .bind(&req.description)
        .fetch_one(&self.pool)
        .await?;

        Ok(product)
    }

    pub async fn get_product(&self, id: i64) -> anyhow::Result<Option<Product>> {
        let product = sqlx::query_as::<_, Product>("SELECT * FROM product where id=$1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(product)
    }

    pub async fn get_product_by_model_no(&self, model_no: &str) -> anyhow::Result<Option<Product>> {
        let product = sqlx::query_as::<_, Product>("SELECT * FROM product WHERE model_no = $1")
            .bind(model_no)
            .fetch_optional(&self.pool)
            .await?;
        Ok(product)
    }

    pub async fn update_product(
        &self,
        id: i32,
        req: &UpdateProductRequest,
    ) -> anyhow::Result<Option<Product>> {
        let product = sqlx::query_as::<_, Product>(
            "UPDATE product SET name=$1, description=$2, auto_provisioning=$3 WHERE id=$4 RETURNING *",
        )
        .bind(&req.name)
        .bind(&req.description)
        .bind(req.auto_provisioning)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(product)
    }

    pub async fn list_products(
        &self,
        search: Option<&str>,
        page: i64,
        page_size: i64,
    ) -> anyhow::Result<(Vec<Product>, i64)> {
        let mut query_builder = QueryBuilder::new("SELECT * FROM product WHERE 1=1");
        Self::add_search_filter(&mut query_builder, search);
        query_builder.push(" ORDER BY updated_at DESC");
        Self::add_pagination(&mut query_builder, page, page_size);

        let products = query_builder
            .build_query_as::<Product>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder =
            QueryBuilder::new("SELECT COUNT(*) as count FROM product WHERE 1=1");
        Self::add_search_filter(&mut count_builder, search);

        let count_row = count_builder.build().fetch_one(&self.pool).await?;
        let total: i64 = count_row.get("count");

        Ok((products, total))
    }
}
