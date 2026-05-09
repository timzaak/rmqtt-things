use anyhow::Context;
use async_trait::async_trait;
use dashmap::DashMap;
use jsonschema::Validator;
use redis::AsyncCommands;
use serde_json::Value;
use std::sync::Arc;

#[async_trait]
pub trait SchemaCacheManager: Send + Sync {
    async fn get(&self, product_id: &str) -> anyhow::Result<Option<Value>>;
    async fn set(&self, product_id: String, schema: Value) -> anyhow::Result<()>;
    async fn _remove(&self, product_id: &str) -> anyhow::Result<()>;
}

#[derive(Clone, Default)]
pub struct InMemorySchemaCache {
    property_schemas: Arc<DashMap<String, Value>>,
}

impl InMemorySchemaCache {
    pub fn new() -> Self {
        Self {
            property_schemas: Arc::new(DashMap::new()),
        }
    }
}

#[async_trait]
impl SchemaCacheManager for InMemorySchemaCache {
    async fn get(&self, product_id: &str) -> anyhow::Result<Option<Value>> {
        Ok(self
            .property_schemas
            .get(product_id)
            .map(|entry| entry.value().clone()))
    }

    async fn set(&self, product_id: String, schema: Value) -> anyhow::Result<()> {
        self.property_schemas.insert(product_id, schema);
        Ok(())
    }

    async fn _remove(&self, product_id: &str) -> anyhow::Result<()> {
        self.property_schemas.remove(product_id);
        Ok(())
    }
}

#[derive(Clone)]
pub struct RedisSchemaCache {
    client: redis::Client,
}

impl RedisSchemaCache {
    pub fn new(redis_url: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(redis_url).context("Failed to open redis client")?;
        Ok(Self { client })
    }

    fn get_key(product_id: &str) -> String {
        format!("schema:{}", product_id)
    }
}

#[async_trait]
impl SchemaCacheManager for RedisSchemaCache {
    async fn get(&self, product_id: &str) -> anyhow::Result<Option<Value>> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .context("Failed to get redis connection")?;
        let key = Self::get_key(product_id);
        let value: Option<String> = conn
            .get(key)
            .await
            .context("Failed to get value from redis")?;
        match value {
            Some(v) => {
                let schema =
                    serde_json::from_str(&v).context("Failed to deserialize schema from redis")?;
                Ok(Some(schema))
            }
            None => Ok(None),
        }
    }

    async fn set(&self, product_id: String, schema: Value) -> anyhow::Result<()> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .context("Failed to get redis connection")?;
        let key = Self::get_key(&product_id);
        let value =
            serde_json::to_string(&schema).context("Failed to serialize schema to string")?;
        conn.set::<_, _, ()>(key, value)
            .await
            .context("Failed to set value in redis")?;
        Ok(())
    }

    async fn _remove(&self, product_id: &str) -> anyhow::Result<()> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .context("Failed to get redis connection")?;
        let key = Self::get_key(product_id);
        conn.del::<_, ()>(key)
            .await
            .context("Failed to delete value from redis")?;
        Ok(())
    }
}

#[derive(Clone)]
pub enum SchemaCache {
    InMemory(Arc<InMemorySchemaCache>),
    Redis(Arc<RedisSchemaCache>),
}

#[async_trait]
impl SchemaCacheManager for SchemaCache {
    async fn get(&self, product_id: &str) -> anyhow::Result<Option<Value>> {
        match self {
            SchemaCache::InMemory(cache) => cache.get(product_id).await,
            SchemaCache::Redis(cache) => cache.get(product_id).await,
        }
    }

    async fn set(&self, product_id: String, schema: Value) -> anyhow::Result<()> {
        match self {
            SchemaCache::InMemory(cache) => cache.set(product_id, schema).await,
            SchemaCache::Redis(cache) => cache.set(product_id, schema).await,
        }
    }

    async fn _remove(&self, product_id: &str) -> anyhow::Result<()> {
        match self {
            SchemaCache::InMemory(cache) => cache._remove(product_id).await,
            SchemaCache::Redis(cache) => cache._remove(product_id).await,
        }
    }
}

pub fn compile_schema(schema: &Value) -> anyhow::Result<Validator> {
    Validator::new(schema)
        .map_err(|e| anyhow::anyhow!(e.to_string()))
        .context("Failed to compile schema")
}
