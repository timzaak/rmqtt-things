use crate::api::alarm_models::CreateAlarmRuleRequest;
use crate::db::models::{AlarmRecord, AlarmRule};
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};

#[derive(Clone)]
pub struct AlarmRepo {
    pool: PgPool,
}

impl AlarmRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn add_pagination<'a>(builder: &mut QueryBuilder<'a, Postgres>, page: i64, page_size: i64) {
        let offset = (page - 1) * page_size;
        builder.push(" LIMIT ");
        builder.push_bind(page_size);
        builder.push(" OFFSET ");
        builder.push_bind(offset);
    }

    pub async fn create_rule(&self, req: &CreateAlarmRuleRequest) -> anyhow::Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO alarm_rule (product_id, name, description, trigger_type, trigger_config, condition, actions, enabled, throttle_minutes, duration_minutes, clear_condition)
            VALUES ($1, $2, $3, $4, $5, $6, $7, true, $8, $9, $10)
            RETURNING id
            "#,
        )
        .bind(&req.product_id)
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.trigger_type)
        .bind(&req.trigger_config)
        .bind(&req.condition)
        .bind(&req.actions)
        .bind(req.throttle_minutes)
        .bind(req.duration_minutes)
        .bind(&req.clear_condition)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get("id"))
    }

    pub async fn get_rule_by_id(&self, id: i64) -> anyhow::Result<Option<AlarmRule>> {
        let rule = sqlx::query_as::<_, AlarmRule>(
            r#"
            SELECT id, product_id, name, description, trigger_type, trigger_config,
                   condition, actions, enabled, throttle_minutes, duration_minutes, clear_condition,
                   created_at, updated_at
            FROM alarm_rule
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(rule)
    }

    pub async fn update_rule(
        &self,
        id: i64,
        name: Option<&str>,
        description: Option<&str>,
        trigger_config: Option<&JsonValue>,
        condition: Option<&JsonValue>,
        actions: Option<&JsonValue>,
        throttle_minutes: Option<i32>,
        duration_minutes: Option<i32>,
        clear_condition: Option<Option<&JsonValue>>,
    ) -> anyhow::Result<u64> {
        let mut builder: QueryBuilder<Postgres> =
            QueryBuilder::new("UPDATE alarm_rule SET updated_at = NOW()");

        if let Some(name) = name {
            builder.push(", name = ");
            builder.push_bind(name);
        }
        if description.is_some() {
            builder.push(", description = ");
            builder.push_bind(description);
        }
        if let Some(trigger_config) = trigger_config {
            builder.push(", trigger_config = ");
            builder.push_bind(trigger_config);
        }
        if let Some(condition) = condition {
            builder.push(", condition = ");
            builder.push_bind(condition);
        }
        if let Some(actions) = actions {
            builder.push(", actions = ");
            builder.push_bind(actions);
        }
        if let Some(throttle_minutes) = throttle_minutes {
            builder.push(", throttle_minutes = ");
            builder.push_bind(throttle_minutes);
        }
        if let Some(duration_minutes) = duration_minutes {
            builder.push(", duration_minutes = ");
            builder.push_bind(duration_minutes);
        }
        if let Some(clear_condition) = clear_condition {
            builder.push(", clear_condition = ");
            match clear_condition {
                Some(val) => builder.push_bind(val),
                None => builder.push("NULL"),
            };
        }

        builder.push(" WHERE id = ");
        builder.push_bind(id);

        let result = builder.build().execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    pub async fn update_rule_status(&self, id: i64, enabled: bool) -> anyhow::Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE alarm_rule
            SET enabled = $1, updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(enabled)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_rule(&self, id: i64) -> anyhow::Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM alarm_rule
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn query_rules(
        &self,
        product_id: Option<&str>,
        enabled: Option<bool>,
        page: i64,
        page_size: i64,
    ) -> anyhow::Result<(Vec<AlarmRule>, i64)> {
        let mut query_builder = QueryBuilder::new(
            r#"SELECT id, product_id, name, description, trigger_type, trigger_config,
                      condition, actions, enabled, throttle_minutes, duration_minutes, clear_condition,
                      created_at, updated_at
               FROM alarm_rule WHERE 1=1"#,
        );

        if let Some(product_id) = product_id {
            query_builder.push(" AND product_id = ");
            query_builder.push_bind(product_id);
        }
        if let Some(enabled) = enabled {
            query_builder.push(" AND enabled = ");
            query_builder.push_bind(enabled);
        }

        query_builder.push(" ORDER BY updated_at DESC");
        Self::add_pagination(&mut query_builder, page, page_size);

        let rules = query_builder
            .build_query_as::<AlarmRule>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder =
            QueryBuilder::new("SELECT COUNT(*) as count FROM alarm_rule WHERE 1=1");
        if let Some(product_id) = product_id {
            count_builder.push(" AND product_id = ");
            count_builder.push_bind(product_id);
        }
        if let Some(enabled) = enabled {
            count_builder.push(" AND enabled = ");
            count_builder.push_bind(enabled);
        }

        let count_row = count_builder.build().fetch_one(&self.pool).await?;
        let total: i64 = count_row.get("count");

        Ok((rules, total))
    }

    pub async fn query_enabled_rules_by_product(
        &self,
        product_id: &str,
    ) -> anyhow::Result<Vec<AlarmRule>> {
        let rules = sqlx::query_as::<_, AlarmRule>(
            r#"
            SELECT id, product_id, name, description, trigger_type, trigger_config,
                   condition, actions, enabled, throttle_minutes, duration_minutes, clear_condition,
                   created_at, updated_at
            FROM alarm_rule
            WHERE product_id = $1 AND enabled = true
            ORDER BY id
            "#,
        )
        .bind(product_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rules)
    }

    pub async fn insert_alarm(
        &self,
        rule_id: i64,
        rule_name: &str,
        product_id: &str,
        device_id: &str,
        level: i16,
        message: Option<&str>,
        trigger_value: Option<&JsonValue>,
    ) -> anyhow::Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO alarm (rule_id, rule_name, product_id, device_id, level, message, trigger_value)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#,
        )
        .bind(rule_id)
        .bind(rule_name)
        .bind(product_id)
        .bind(device_id)
        .bind(level)
        .bind(message)
        .bind(trigger_value)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get("id"))
    }

    pub async fn update_alarm_webhook_status(
        &self,
        id: i64,
        webhook_status: i16,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE alarm
            SET webhook_status = $1
            WHERE id = $2
            "#,
        )
        .bind(webhook_status)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn query_alarms(
        &self,
        product_id: Option<&str>,
        device_id: Option<&str>,
        level: Option<i16>,
        acknowledged: Option<bool>,
        status: Option<&str>,
        page: i64,
        page_size: i64,
    ) -> anyhow::Result<(Vec<AlarmRecord>, i64)> {
        let mut query_builder = QueryBuilder::new(
            r#"SELECT id, rule_id, rule_name, product_id, device_id, level,
                      message, trigger_value, acknowledged, status, webhook_status, created_at, cleared_at
               FROM alarm WHERE 1=1"#,
        );

        if let Some(product_id) = product_id {
            query_builder.push(" AND product_id = ");
            query_builder.push_bind(product_id);
        }
        if let Some(device_id) = device_id {
            query_builder.push(" AND device_id = ");
            query_builder.push_bind(device_id);
        }
        if let Some(level) = level {
            query_builder.push(" AND level = ");
            query_builder.push_bind(level);
        }
        if let Some(status) = status {
            query_builder.push(" AND status = ");
            query_builder.push_bind(status);
        } else if let Some(acknowledged) = acknowledged {
            // acknowledged is kept for backward compatibility, mapped to status
            if acknowledged {
                query_builder.push(" AND status != 'active'");
            } else {
                query_builder.push(" AND status = 'active'");
            }
        }

        query_builder.push(" ORDER BY created_at DESC");
        Self::add_pagination(&mut query_builder, page, page_size);

        let alarms = query_builder
            .build_query_as::<AlarmRecord>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder = QueryBuilder::new("SELECT COUNT(*) as count FROM alarm WHERE 1=1");
        if let Some(product_id) = product_id {
            count_builder.push(" AND product_id = ");
            count_builder.push_bind(product_id);
        }
        if let Some(device_id) = device_id {
            count_builder.push(" AND device_id = ");
            count_builder.push_bind(device_id);
        }
        if let Some(level) = level {
            count_builder.push(" AND level = ");
            count_builder.push_bind(level);
        }
        if let Some(status) = status {
            count_builder.push(" AND status = ");
            count_builder.push_bind(status);
        } else if let Some(acknowledged) = acknowledged {
            if acknowledged {
                count_builder.push(" AND status != 'active'");
            } else {
                count_builder.push(" AND status = 'active'");
            }
        }

        let count_row = count_builder.build().fetch_one(&self.pool).await?;
        let total: i64 = count_row.get("count");

        Ok((alarms, total))
    }

    pub async fn get_alarm_by_id(&self, id: i64) -> anyhow::Result<Option<AlarmRecord>> {
        let alarm = sqlx::query_as::<_, AlarmRecord>(
            r#"
            SELECT id, rule_id, rule_name, product_id, device_id, level,
                   message, trigger_value, acknowledged, status, webhook_status, created_at, cleared_at
            FROM alarm
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(alarm)
    }

    pub async fn ack_alarm(&self, id: i64) -> anyhow::Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE alarm
            SET acknowledged = true, status = 'acknowledged'
            WHERE id = $1 AND status = 'active'
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn query_active_alarms_for_clear(
        &self,
        rule_id: i64,
        device_id: &str,
    ) -> anyhow::Result<Vec<AlarmRecord>> {
        let alarms = sqlx::query_as::<_, AlarmRecord>(
            r#"
            SELECT id, rule_id, rule_name, product_id, device_id, level,
                   message, trigger_value, acknowledged, status, webhook_status, created_at, cleared_at
            FROM alarm
            WHERE rule_id = $1 AND device_id = $2 AND status IN ('active', 'acknowledged')
            "#,
        )
        .bind(rule_id)
        .bind(device_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(alarms)
    }

    pub async fn clear_alarm(&self, id: i64) -> anyhow::Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE alarm
            SET status = 'cleared', cleared_at = NOW(), acknowledged = true
            WHERE id = $1 AND status != 'cleared'
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
