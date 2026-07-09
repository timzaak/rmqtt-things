use crate::api::web_models::{DeviceConnectRequest, DeviceDisconnectRequest};
use crate::config::AlarmConfig;
use crate::db::alarm::AlarmRepo;
use crate::db::cert_issue::CertIssueRepo;
use crate::db::device::DeviceRepo;
use crate::db::models::*;
use crate::db::ota::OtaRepo;
use crate::db::product::ProductRepo;
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use time::OffsetDateTime;

const UNIX_TIMESTAMP_MS_THRESHOLD: i64 = 9_999_999_999;

/// Error message returned when attempting to update the schema of an active validation template.
/// Referenced from the API handler to avoid fragile string matching.
pub const ACTIVE_TEMPLATE_SCHEMA_ERR: &str = "Cannot update schema of active template";

fn timestamp_to_datetime(ts: i64) -> OffsetDateTime {
    let seconds = if ts > UNIX_TIMESTAMP_MS_THRESHOLD {
        ts / 1000
    } else {
        ts
    };
    OffsetDateTime::from_unix_timestamp(seconds).unwrap()
}

#[derive(Clone)]
pub struct DatabaseService {
    pool: PgPool,
    alarm_config: AlarmConfig,
}

impl DatabaseService {
    pub fn new(pool: PgPool, alarm_config: AlarmConfig) -> Self {
        Self { pool, alarm_config }
    }

    pub fn cert_issue(&self) -> CertIssueRepo {
        CertIssueRepo::new(self.pool.clone())
    }

    pub fn product(&self) -> ProductRepo {
        ProductRepo::new(self.pool.clone())
    }

    pub fn ota(&self) -> OtaRepo {
        OtaRepo::new(self.pool.clone())
    }

    pub fn alarm(&self) -> AlarmRepo {
        AlarmRepo::new(
            self.pool.clone(),
            self.alarm_config.webhook_max_retries,
            self.alarm_config.webhook_retry_interval_seconds,
        )
    }

    pub fn device(&self) -> DeviceRepo {
        DeviceRepo::new(self.pool.clone())
    }

    fn add_device_status_filter<'a>(
        builder: &mut QueryBuilder<'a, Postgres>,
        product_id: Option<&'a str>,
        device_id: Option<&'a str>,
        status: Option<DeviceConnectionStatus>,
        registration_source: Option<RegistrationSource>,
    ) {
        if let Some(product_id) = product_id {
            builder.push(" AND d.product_id = ");
            builder.push_bind(product_id);
        }
        if let Some(device_id) = device_id {
            builder.push(" AND d.device_id = ");
            builder.push_bind(device_id);
        }
        if let Some(status) = status {
            builder.push(" AND ds.status = ");
            builder.push_bind(status);
        }
        if let Some(registration_source) = registration_source {
            builder.push(" AND d.registration_source = ");
            builder.push_bind(registration_source);
        }
    }

    fn add_device_product_filter<'a>(
        builder: &mut QueryBuilder<'a, Postgres>,
        product_id: &'a str,
        device_id: Option<&'a str>,
    ) {
        builder.push(" AND product_id = ");
        builder.push_bind(product_id);
        if let Some(device_id) = device_id {
            builder.push(" AND device_id = ");
            builder.push_bind(device_id);
        }
    }

    // 通用分页构建器
    fn add_pagination<'a>(builder: &mut QueryBuilder<'a, Postgres>, page: i64, page_size: i64) {
        let offset = (page - 1) * page_size;
        builder.push(" LIMIT ");
        builder.push_bind(page_size);
        builder.push(" OFFSET ");
        builder.push_bind(offset);
    }

    // 属性上报 - 更新最新属性表和历史表
    pub async fn upsert_property_latest(
        &self,
        product_id: &str,
        device_id: &str,
        properties: serde_json::Map<String, JsonValue>,
        timestamp: OffsetDateTime,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        // 构建带时间戳的新属性
        let mut properties_with_time = serde_json::Map::new();
        let timestamp_str = timestamp.format(&time::format_description::well_known::Rfc3339)?;
        for (key, value) in properties.iter() {
            let property_with_time = serde_json::json!({
                "value": value,
                "time": &timestamp_str,
            });
            properties_with_time.insert(key.clone(), property_with_time);
        }
        let new_properties = JsonValue::Object(properties_with_time);

        // 使用 PostgreSQL JSON 合并操作更新最新属性表
        sqlx::query(
            r#"
            INSERT INTO property_latest (product_id, device_id, properties, updated_time)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (product_id, device_id)
            DO UPDATE SET 
                properties = COALESCE(property_latest.properties, '{}'::jsonb) || $3,
                updated_time = $4
            "#,
        )
        .bind(product_id)
        .bind(device_id)
        .bind(&new_properties)
        .bind(timestamp)
        .execute(&mut *tx)
        .await?;

        // 插入历史记录
        sqlx::query(
            r#"
            INSERT INTO property_history (product_id, device_id, properties, reported_time)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(product_id)
        .bind(device_id)
        .bind(JsonValue::Object(properties))
        .bind(timestamp)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    // 事件上报
    pub async fn insert_event_history(
        &self,
        product_id: &str,
        device_id: &str,
        events: &JsonValue,
        timestamp: OffsetDateTime,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO event_history (product_id, device_id, events, reported_time)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(product_id)
        .bind(device_id)
        .bind(events)
        .bind(timestamp)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // 更新属性下发结果
    pub async fn update_property_command_status(
        &self,
        command_id: &Vec<i64>,
        product_id: &str,
        device_id: &str,
        status: CommandStatus,
        prev_status: CommandStatus,
    ) -> anyhow::Result<()> {
        // println!("status {product_id} _ {device_id} _ prev_status ${prev_status:?} now:${status:?}");
        sqlx::query(
            r#"
            UPDATE property_command 
            SET status = $1, updated_time = CURRENT_TIMESTAMP
            WHERE id = ANY($2) and status = $3 and product_id = $4 and device_id = $5
            "#,
        )
        .bind(status)
        .bind(command_id)
        .bind(prev_status)
        .bind(product_id)
        .bind(device_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    /*
        pub async fn get_property_latest(
            &self,
            product_id: &str,
            device_id: &str,
        ) -> anyhow::Result<Option<PropertyLatest>> {
            let result = sqlx::query_as::<_, PropertyLatest>(
                "SELECT product_id, device_id, properties, updated_time FROM property_latest WHERE product_id = $1 AND device_id = $2",
            )
            .bind(product_id)
            .bind(device_id)
            .fetch_optional(&self.pool)
            .await?;

            Ok(result)
        }
    */
    // 更新 sent 状态的命令为 failed
    pub async fn update_sent_commands_to_failed(&self, command_ids: &[i64]) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE property_command
            SET status = $1, updated_time = CURRENT_TIMESTAMP
            WHERE id = ANY($2) AND status = $3
            "#,
        )
        .bind(CommandStatus::Failed)
        .bind(command_ids)
        .bind(CommandStatus::Sent)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // 更新 pending 状态的命令为 sent，并返回命令信息
    pub async fn update_pending_commands_to_sent(
        &self,
        product_id: &str,
        device_id: &str,
    ) -> anyhow::Result<Vec<(i64, JsonValue, OffsetDateTime)>> {
        let commands = sqlx::query(
            r#"
            UPDATE property_command 
            SET status = $1, updated_time = CURRENT_TIMESTAMP
            WHERE product_id = $2 AND device_id = $3 AND status = $4
            RETURNING id, command, created_time
            "#,
        )
        .bind(CommandStatus::Sent)
        .bind(product_id)
        .bind(device_id)
        .bind(CommandStatus::Pending)
        .fetch_all(&self.pool)
        .await?;

        let result = commands
            .into_iter()
            .map(|row| (row.get("id"), row.get("command"), row.get("created_time")))
            .collect();

        Ok(result)
    }

    // 查询属性命令（用于管理接口）
    pub async fn query_property_commands(
        &self,
        product_id: &str,
        device_id: Option<&str>,
        status: Option<CommandStatus>,
        page: i64,
        page_size: i64,
    ) -> anyhow::Result<(Vec<PropertyCommand>, i64)> {
        let mut query_builder = QueryBuilder::new(
            "SELECT id, product_id, device_id, command, status, created_time, updated_time FROM property_command WHERE 1=1",
        );

        Self::add_device_product_filter(&mut query_builder, product_id, device_id);

        if let Some(status) = status {
            query_builder.push(" AND status = ");
            query_builder.push_bind(status);
        }

        query_builder.push(" ORDER BY updated_time DESC");
        Self::add_pagination(&mut query_builder, page, page_size);

        let commands = query_builder
            .build_query_as::<PropertyCommand>()
            .fetch_all(&self.pool)
            .await?;

        // 获取总数
        let mut count_builder =
            QueryBuilder::new("SELECT COUNT(*) as count FROM property_command WHERE 1=1");

        Self::add_device_product_filter(&mut count_builder, product_id, device_id);

        if let Some(status) = status {
            count_builder.push(" AND status = ");
            count_builder.push_bind(status);
        }

        let count_row = count_builder.build().fetch_one(&self.pool).await?;

        let total: i64 = count_row.get("count");

        Ok((commands, total))
    }

    // 查询最新属性
    pub async fn query_property_latest(
        &self,
        product_id: &str,
        device_id: Option<&str>,
        page: i64,
        page_size: i64,
    ) -> anyhow::Result<Vec<PropertyLatest>> {
        let mut query_builder = QueryBuilder::new(
            "SELECT product_id, device_id, properties, updated_time FROM property_latest WHERE 1=1",
        );

        Self::add_device_product_filter(&mut query_builder, product_id, device_id);
        query_builder.push(" ORDER BY updated_time DESC");
        Self::add_pagination(&mut query_builder, page, page_size);

        let properties = query_builder
            .build_query_as::<PropertyLatest>()
            .fetch_all(&self.pool)
            .await?;

        Ok(properties)
    }

    // 查询属性历史
    pub async fn query_property_history(
        &self,
        product_id: &str,
        device_id: Option<&str>,
        page: i64,
        page_size: i64,
    ) -> anyhow::Result<Vec<PropertyHistory>> {
        let mut query_builder = QueryBuilder::new(
            "SELECT id, product_id, device_id, properties, reported_time, created_time FROM property_history WHERE 1=1",
        );

        Self::add_device_product_filter(&mut query_builder, product_id, device_id);
        query_builder.push(" ORDER BY created_time DESC");
        Self::add_pagination(&mut query_builder, page, page_size);

        let properties = query_builder
            .build_query_as::<PropertyHistory>()
            .fetch_all(&self.pool)
            .await?;

        Ok(properties)
    }

    // 查询事件历史
    pub async fn query_event_history(
        &self,
        product_id: &str,
        device_id: Option<&str>,
        page: i64,
        page_size: i64,
    ) -> anyhow::Result<Vec<EventHistory>> {
        let mut query_builder = QueryBuilder::new(
            "SELECT id, product_id, device_id, events, reported_time, created_time FROM event_history WHERE 1=1",
        );

        Self::add_device_product_filter(&mut query_builder, product_id, device_id);
        query_builder.push(" ORDER BY created_time DESC");
        Self::add_pagination(&mut query_builder, page, page_size);

        let events = query_builder
            .build_query_as::<EventHistory>()
            .fetch_all(&self.pool)
            .await?;

        Ok(events)
    }

    // 插入属性命令（用于管理接口）
    pub async fn insert_property_command(
        &self,
        product_id: &str,
        device_id: &str,
        command: &JsonValue,
    ) -> anyhow::Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO property_command (product_id, device_id, command)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
        )
        .bind(product_id)
        .bind(device_id)
        .bind(command)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("id"))
    }

    // 删除属性命令（设置为 deleted 状态）
    pub async fn delete_property_commands(&self, ids: &[i64]) -> anyhow::Result<i64> {
        let result = sqlx::query(
            r#"
            UPDATE property_command 
            SET status = $1, updated_time = CURRENT_TIMESTAMP
            WHERE id = ANY($2) AND status = $3
            "#,
        )
        .bind(CommandStatus::Deleted)
        .bind(ids)
        .bind(CommandStatus::Pending)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as i64)
    }

    // Upsert device status on connect
    pub async fn upsert_device_status_connect(
        &self,
        req: &DeviceConnectRequest,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        let now = OffsetDateTime::now_utc();
        let connected_at = timestamp_to_datetime(req.connected_at);
        let ip = req.ipaddress.rsplit_once(":").unwrap().0;
        // Upsert into device_status
        sqlx::query(
            r#"
            INSERT INTO device_status (product_id, device_id, status, ip_address, last_online_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $6)
            ON CONFLICT (product_id, device_id) DO UPDATE SET
                status = $3,
                ip_address = $4,
                last_online_at = $5,
                updated_at = $6
            "#,
        )
        .bind(&req.product_id)
        .bind(&req.device_id)
        .bind(DeviceConnectionStatus::Online)
        .bind(ip)
        .bind(connected_at)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        // Insert into device_status_history
        sqlx::query(
            r#"
            INSERT INTO device_status_history (product_id, device_id, status, ip_address, connected_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(&req.product_id)
        .bind(&req.device_id)
        .bind(DeviceConnectionStatus::Online)
        .bind(&req.ipaddress)
        .bind(connected_at)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    // Update device status on disconnect
    pub async fn update_device_status_disconnect(
        &self,
        req: &DeviceDisconnectRequest,
    ) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        let now = OffsetDateTime::now_utc();
        let disconnected_at = timestamp_to_datetime(req.disconnected_at);

        // Update device_status
        sqlx::query(
            r#"
            UPDATE device_status
            SET status = $1, last_offline_at = $2, updated_at = $3
            WHERE product_id = $4 AND device_id = $5
            "#,
        )
        .bind(DeviceConnectionStatus::Offline)
        .bind(disconnected_at)
        .bind(now)
        .bind(&req.product_id)
        .bind(&req.device_id)
        .execute(&mut *tx)
        .await?;

        // Insert into device_status_history
        sqlx::query(
            r#"
            INSERT INTO device_status_history (product_id, device_id, status, ip_address, reason, disconnected_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(&req.product_id)
        .bind(&req.device_id)
        .bind(DeviceConnectionStatus::Offline)
        .bind(&req.ipaddress)
        .bind(&req.reason)
        .bind(disconnected_at)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    // Query device status
    pub async fn query_device_status(
        &self,
        product_id: Option<&str>,
        device_id: Option<&str>,
        status: Option<DeviceConnectionStatus>,
        registration_source: Option<RegistrationSource>,
        page: i64,
        page_size: i64,
    ) -> anyhow::Result<(Vec<DeviceStatusWithSource>, i64)> {
        let mut query_builder = QueryBuilder::new(
            "SELECT d.product_id, d.device_id, ds.status, ds.ip_address, ds.last_online_at, ds.last_offline_at, d.registration_source, d.created_at, COALESCE(ds.updated_at, d.updated_at) as updated_at FROM devices d LEFT JOIN device_status ds ON d.product_id = ds.product_id AND d.device_id = ds.device_id WHERE 1=1",
        );

        Self::add_device_status_filter(
            &mut query_builder,
            product_id,
            device_id,
            status,
            registration_source,
        );

        query_builder.push(" ORDER BY updated_at DESC");
        Self::add_pagination(&mut query_builder, page, page_size);

        let devices = query_builder
            .build_query_as::<DeviceStatusWithSource>()
            .fetch_all(&self.pool)
            .await?;

        // Get total count
        let total: i64 = if status.is_none() {
            let mut count_builder =
                QueryBuilder::new("SELECT COUNT(*) as count FROM devices d WHERE 1=1");
            if let Some(product_id) = product_id {
                count_builder.push(" AND d.product_id = ");
                count_builder.push_bind(product_id);
            }
            if let Some(device_id) = device_id {
                count_builder.push(" AND d.device_id = ");
                count_builder.push_bind(device_id);
            }
            if let Some(registration_source) = registration_source {
                count_builder.push(" AND d.registration_source = ");
                count_builder.push_bind(registration_source);
            }
            count_builder
                .build()
                .fetch_one(&self.pool)
                .await?
                .get("count")
        } else {
            let mut count_builder = QueryBuilder::new(
                "SELECT COUNT(*) as count FROM devices d LEFT JOIN device_status ds ON d.product_id = ds.product_id AND d.device_id = ds.device_id WHERE 1=1",
            );
            Self::add_device_status_filter(
                &mut count_builder,
                product_id,
                device_id,
                status,
                registration_source,
            );
            count_builder
                .build()
                .fetch_one(&self.pool)
                .await?
                .get("count")
        };

        Ok((devices, total))
    }

    // Query device status history
    pub async fn query_device_status_history(
        &self,
        product_id: &str,
        device_id: Option<&str>,
        page: i64,
        page_size: i64,
    ) -> anyhow::Result<Vec<DeviceStatusHistory>> {
        let mut query_builder = QueryBuilder::new(
            "SELECT id, product_id, device_id, status, ip_address, reason, connected_at, disconnected_at, created_at FROM device_status_history WHERE 1=1",
        );

        Self::add_device_product_filter(&mut query_builder, product_id, device_id);

        query_builder.push(" ORDER BY created_at DESC");
        Self::add_pagination(&mut query_builder, page, page_size);

        let history = query_builder
            .build_query_as::<DeviceStatusHistory>()
            .fetch_all(&self.pool)
            .await?;

        Ok(history)
    }

    // Query event valid templates
    pub async fn query_event_valid_templates(
        &self,
        product_id: Option<&str>,
        event: Option<&str>,
        page: i64,
        page_size: i64,
    ) -> anyhow::Result<(Vec<EventValidTemplate>, i64)> {
        let mut query_builder = QueryBuilder::new(
            "SELECT id, product_id, event, description, schema, status, created_at, updated_at FROM event_valid_template WHERE 1=1",
        );

        if let Some(product_id) = product_id {
            query_builder.push(" AND product_id = ");
            query_builder.push_bind(product_id);
        }

        if let Some(event) = event {
            query_builder.push(" AND event = ");
            query_builder.push_bind(event);
        }

        query_builder.push(" ORDER BY updated_at DESC");
        Self::add_pagination(&mut query_builder, page, page_size);

        let templates = query_builder
            .build_query_as::<EventValidTemplate>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder =
            QueryBuilder::new("SELECT COUNT(*) as count FROM event_valid_template WHERE 1=1");

        if let Some(product_id) = product_id {
            count_builder.push(" AND product_id = ");
            count_builder.push_bind(product_id);
        }

        if let Some(event) = event {
            count_builder.push(" AND event = ");
            count_builder.push_bind(event);
        }

        let count_row = count_builder.build().fetch_one(&self.pool).await?;
        let total: i64 = count_row.get("count");

        Ok((templates, total))
    }

    // Get event valid template by id
    pub async fn get_event_valid_template_by_id(
        &self,
        id: i64,
    ) -> anyhow::Result<Option<EventValidTemplate>> {
        let template = sqlx::query_as::<_, EventValidTemplate>(
            "SELECT id, product_id, event, description, schema, status, created_at, updated_at FROM event_valid_template WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(template)
    }

    // Delete event valid template by id. Returns the number of rows deleted
    // (0 when the id does not exist). The caller is responsible for cache
    // invalidation when the deleted template was an Active property schema.
    pub async fn delete_event_valid_template(&self, id: i64) -> anyhow::Result<u64> {
        let result = sqlx::query("DELETE FROM event_valid_template WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    // get event valid template by id for update
    pub async fn get_event_valid_template_by_id_for_update<'a>(
        &self,
        id: i64,
        tx: &mut sqlx::Transaction<'a, Postgres>,
    ) -> anyhow::Result<Option<EventValidTemplate>> {
        let template = sqlx::query_as::<_, EventValidTemplate>(
            "SELECT id, product_id, event, description, schema, status, created_at, updated_at FROM event_valid_template WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?;
        Ok(template)
    }

    // Insert event valid template
    pub async fn insert_event_valid_template(
        &self,
        product_id: &str,
        event: &str,
        description: Option<&str>,
        schema: &JsonValue,
    ) -> anyhow::Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO event_valid_template (product_id, event, description, schema)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#,
        )
        .bind(product_id)
        .bind(event)
        .bind(description)
        .bind(schema)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get("id"))
    }

    // Update event valid template status
    pub async fn update_event_valid_template_status(
        &self,
        id: i64,
        status: EventValidTemplateStatus,
    ) -> anyhow::Result<u64> {
        let mut tx = self.pool.begin().await?;

        let template = self
            .get_event_valid_template_by_id_for_update(id, &mut tx)
            .await?;
        if template.is_none() {
            return Ok(0);
        }
        let template = template.unwrap();

        if status == EventValidTemplateStatus::Active {
            // set other active to inactive
            sqlx::query(
                r#"
                UPDATE event_valid_template
                SET status = $1
                WHERE product_id = $2 AND status = $3 AND id != $4
                "#,
            )
            .bind(EventValidTemplateStatus::Inactive)
            .bind(&template.product_id)
            .bind(EventValidTemplateStatus::Active)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }

        let result = sqlx::query(
            r#"
            UPDATE event_valid_template
            SET status = $1
            WHERE id = $2
            "#,
        )
        .bind(status)
        .bind(id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(result.rows_affected())
    }

    // Update event valid template schema
    pub async fn update_event_valid_template(
        &self,
        id: i64,
        schema: Option<&JsonValue>,
        description: Option<&str>,
    ) -> anyhow::Result<u64> {
        let mut tx = self.pool.begin().await?;

        let template = self
            .get_event_valid_template_by_id_for_update(id, &mut tx)
            .await?;
        if template.is_none() {
            return Ok(0);
        }
        let template = template.unwrap();

        if schema.is_none() && description.is_none() {
            return Ok(0);
        }

        let mut query_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("UPDATE event_valid_template SET ");

        let mut needs_comma = false;

        if let Some(schema) = schema {
            if template.status == EventValidTemplateStatus::Active {
                return Err(anyhow::anyhow!(ACTIVE_TEMPLATE_SCHEMA_ERR));
            }
            query_builder.push("schema = ");
            query_builder.push_bind(schema);
            needs_comma = true;
        }

        if let Some(description) = description {
            if needs_comma {
                query_builder.push(", ");
            }
            query_builder.push("description = ");
            query_builder.push_bind(description);
        }

        query_builder.push(" WHERE id = ");
        query_builder.push_bind(id);

        let result = query_builder.build().execute(&mut *tx).await?;

        tx.commit().await?;
        Ok(result.rows_affected())
    }

    // Get property schema by product_id
    pub async fn get_property_schema(
        &self,
        product_id: &str,
    ) -> anyhow::Result<Option<EventValidTemplate>> {
        let template = sqlx::query_as::<_, EventValidTemplate>(
            r#"
            SELECT id, product_id, event, description, schema, status, created_at, updated_at
            FROM event_valid_template
            WHERE product_id = $1 AND event = 'property' AND status = $2
            "#,
        )
        .bind(product_id)
        .bind(EventValidTemplateStatus::Active)
        .fetch_optional(&self.pool)
        .await?;
        Ok(template)
    }

    // Get active event validation template by (product_id, event identifier).
    // Returns None when no Active template matches, in which case callers
    // (event_post) should allow the event through without validation, matching
    // property_post's "no schema = no validation" semantics for absent schemas.
    pub async fn get_event_valid_template(
        &self,
        product_id: &str,
        event: &str,
    ) -> anyhow::Result<Option<EventValidTemplate>> {
        let template = sqlx::query_as::<_, EventValidTemplate>(
            r#"
            SELECT id, product_id, event, description, schema, status, created_at, updated_at
            FROM event_valid_template
            WHERE product_id = $1 AND event = $2 AND status = $3
            "#,
        )
        .bind(product_id)
        .bind(event)
        .bind(EventValidTemplateStatus::Active)
        .fetch_optional(&self.pool)
        .await?;
        Ok(template)
    }

    // Read a single property_latest row (reported snapshot) by (product_id, device_id).
    // Dedicated point-read for Get-Delta; does not reuse the paginated query_property_latest.
    pub async fn get_property_latest_one(
        &self,
        product_id: &str,
        device_id: &str,
    ) -> anyhow::Result<Option<PropertyLatest>> {
        let row = sqlx::query_as::<_, PropertyLatest>(
            r#"
            SELECT product_id, device_id, properties, updated_time
            FROM property_latest
            WHERE product_id = $1 AND device_id = $2
            "#,
        )
        .bind(product_id)
        .bind(device_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    // Read a single property_desired row by (product_id, device_id).
    // Returns None when no desired state has been persisted for the device.
    pub async fn get_property_desired(
        &self,
        product_id: &str,
        device_id: &str,
    ) -> anyhow::Result<Option<PropertyDesired>> {
        let row = sqlx::query_as::<_, PropertyDesired>(
            r#"
            SELECT product_id, device_id, desired, updated_time
            FROM property_desired
            WHERE product_id = $1 AND device_id = $2
            "#,
        )
        .bind(product_id)
        .bind(device_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    // RFC 7396 subset merge upsert for desired state.
    //
    // desired stores bare property values. The patch follows RFC 7396 subset:
    //   - non-null value -> set/overwrite that desired key
    //   - null value      -> remove that desired key
    // JSONB `||` cannot express deletion, so the merged document is computed in
    // Rust and written back as a whole-document update.
    //
    // Concurrency: a Set-Desired patch is a read-merge-write over the whole
    // `desired` column. To keep concurrent patches on the SAME device from
    // clobbering each other, the row is locked for the duration of the
    // transaction. `FOR UPDATE` only locks an existing row, so the first write
    // for a device is bootstrapped with an `INSERT ... ON CONFLICT DO NOTHING`
    // of an empty document; whichever transaction wins the insert creates the
    // row, and the loser's insert becomes a no-op. Both then `SELECT ... FOR
    // UPDATE`, where the loser blocks until the winner commits, so it merges
    // against the just-written document instead of a stale `{}()`. This matches
    // the `get_event_valid_template_by_id_for_update` locking convention.
    pub async fn upsert_property_desired(
        &self,
        product_id: &str,
        device_id: &str,
        desired_patch: &serde_json::Map<String, JsonValue>,
    ) -> anyhow::Result<PropertyDesired> {
        let mut tx = self.pool.begin().await?;

        // Ensure the row exists. ON CONFLICT DO NOTHING makes the insert a no-op
        // if a concurrent transaction already created it; the value written here
        // is a placeholder and is always overwritten by the locked read-merge-write
        // below, so it never leaks into a response.
        sqlx::query(
            r#"
            INSERT INTO property_desired (product_id, device_id, desired, updated_time)
            VALUES ($1, $2, '{}'::jsonb, CURRENT_TIMESTAMP)
            ON CONFLICT (product_id, device_id) DO NOTHING
            "#,
        )
        .bind(product_id)
        .bind(device_id)
        .execute(&mut *tx)
        .await?;

        // Lock the row and read the committed document. A concurrent patch on
        // the same device blocks here until the holder commits, then sees the
        // freshly-written desired state.
        let existing = sqlx::query_as::<_, PropertyDesired>(
            r#"
            SELECT product_id, device_id, desired, updated_time
            FROM property_desired
            WHERE product_id = $1 AND device_id = $2
            FOR UPDATE
            "#,
        )
        .bind(product_id)
        .bind(device_id)
        .fetch_one(&mut *tx)
        .await?;

        let current_map = existing.desired.as_object().cloned().unwrap_or_default();

        let merged = merge_desired(&current_map, desired_patch);
        let merged_value = JsonValue::Object(merged);
        let now = OffsetDateTime::now_utc();

        let row = sqlx::query_as::<_, PropertyDesired>(
            r#"
            UPDATE property_desired
            SET desired = $3, updated_time = $4
            WHERE product_id = $1 AND device_id = $2
            RETURNING product_id, device_id, desired, updated_time
            "#,
        )
        .bind(product_id)
        .bind(device_id)
        .bind(&merged_value)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(row)
    }
}

/// RFC 7396 subset merge for desired state (pure in-memory computation).
///
/// - non-null patch value -> overwrite that key in `current`
/// - null patch value     -> remove that key from `current` (deletion)
///
/// Kept at the db layer because it is invoked by `upsert_property_desired`;
/// `compute_delta` lives in the api layer (`api/shadow.rs`) for the Get-Delta
/// handler.
fn merge_desired(
    current: &serde_json::Map<String, JsonValue>,
    patch: &serde_json::Map<String, JsonValue>,
) -> serde_json::Map<String, JsonValue> {
    let mut result = current.clone();
    for (key, val) in patch.iter() {
        if val.is_null() {
            result.remove(key);
        } else {
            result.insert(key.clone(), val.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::merge_desired;
    use serde_json::{Value as JsonValue, json};

    fn map_of(value: JsonValue) -> serde_json::Map<String, JsonValue> {
        match value {
            JsonValue::Object(map) => map,
            _ => panic!("expected JSON object"),
        }
    }

    #[test]
    fn merge_desired_null_value_deletes_key() {
        // null in patch removes the key from current.
        let current = map_of(json!({"brightness": 80, "mode": "eco"}));
        let patch = map_of(json!({"brightness": null}));
        let result = merge_desired(&current, &patch);
        assert_eq!(JsonValue::Object(result), json!({"mode": "eco"}));
    }

    #[test]
    fn merge_desired_non_null_value_overrides_existing() {
        let current = map_of(json!({"brightness": 80}));
        let patch = map_of(json!({"brightness": 50}));
        let result = merge_desired(&current, &patch);
        assert_eq!(JsonValue::Object(result), json!({"brightness": 50}));
    }

    #[test]
    fn merge_desired_inserts_new_key() {
        let current = map_of(json!({"brightness": 80}));
        let patch = map_of(json!({"color": "red"}));
        let result = merge_desired(&current, &patch);
        assert_eq!(
            JsonValue::Object(result),
            json!({"brightness": 80, "color": "red"})
        );
    }

    #[test]
    fn merge_desired_empty_patch_leaves_current_unchanged() {
        let current = map_of(json!({"brightness": 80, "mode": "eco"}));
        let patch = serde_json::Map::new();
        let result = merge_desired(&current, &patch);
        assert_eq!(
            JsonValue::Object(result),
            json!({"brightness": 80, "mode": "eco"})
        );
    }

    #[test]
    fn merge_desired_all_null_patch_clears_document() {
        let current = map_of(json!({"brightness": 80, "mode": "eco"}));
        let patch = map_of(json!({"brightness": null, "mode": null}));
        let result = merge_desired(&current, &patch);
        assert_eq!(JsonValue::Object(result), json!({}));
    }
}
