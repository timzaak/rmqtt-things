CREATE TABLE IF NOT EXISTS property_latest (
    product_id TEXT NOT NULL,
    device_id    text,
    properties   JSONB NOT NULL,
    updated_time TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE property_latest ADD PRIMARY KEY (product_id, device_id);

CREATE INDEX IF NOT EXISTS idx_property_latest_updated_time
    ON property_latest (updated_time DESC);

-- 属性历史表
CREATE TABLE IF NOT EXISTS property_history (
    id            BIGSERIAL PRIMARY KEY,
    device_id     text  NOT NULL,
    properties    JSONB NOT NULL,
    reported_time TIMESTAMPTZ,
    created_time  TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    product_id TEXT NOT NULL
);

-- 按设备 + 时间索引
CREATE INDEX IF NOT EXISTS idx_property_history_product_device_reported_time
ON property_history (product_id, device_id, reported_time DESC);


-- 事件历史表
CREATE TABLE IF NOT EXISTS event_history (
    id            BIGSERIAL PRIMARY KEY,
    device_id     text  NOT NULL,
    events        JSONB NOT NULL,
    reported_time TIMESTAMPTZ,
    created_time  TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    product_id TEXT NOT NULL
);

-- 按设备 + 时间索引
CREATE INDEX IF NOT EXISTS idx_event_history_product_device_reported_time
ON event_history (product_id, device_id, reported_time DESC);

-- 属性下发表
CREATE TABLE IF NOT EXISTS property_command (
    id           BIGSERIAL PRIMARY KEY,
    device_id    text  NOT NULL,
    command      JSONB NOT NULL, -- 下发属性 {"brightness":80,"mode":"eco"}
    status       int2  NOT NULL DEFAULT 0, -- 0: pending, 1:sent, 2:success, 3:failed
    created_time TIMESTAMPTZ    DEFAULT CURRENT_TIMESTAMP,
    updated_time TIMESTAMPTZ    DEFAULT CURRENT_TIMESTAMP,
    product_id TEXT NOT NULL
);

COMMENT ON COLUMN property_command.status IS '0: pending, 1:sent, 2:success, 3:failed, 4:deleted';


-- 为属性下发表创建索引
CREATE INDEX IF NOT EXISTS idx_property_command_product_device_status
ON property_command (product_id, device_id, status);

CREATE INDEX IF NOT EXISTS idx_property_command_created_time
ON property_command (created_time DESC);

-- 添加 updated_time 索引，因为查询中使用 ORDER BY updated_time DESC
CREATE INDEX IF NOT EXISTS idx_property_command_updated_time
ON property_command (updated_time DESC);

-- 复合索引优化带设备过滤的排序查询
CREATE INDEX IF NOT EXISTS idx_property_command_product_device_updated_time
ON property_command (product_id, device_id, updated_time DESC);



-- 为 property_history 表添加 created_time 索引
CREATE INDEX IF NOT EXISTS idx_property_history_created_time
ON property_history (created_time DESC);

-- 复合索引优化带设备过滤的排序查询
CREATE INDEX IF NOT EXISTS idx_property_history_product_device_created_time
ON property_history (product_id, device_id, created_time DESC);

-- 为 event_history 表添加 created_time 索引
CREATE INDEX IF NOT EXISTS idx_event_history_created_time
ON event_history (created_time DESC);

-- 复合索引优化带设备过滤的排序查询
CREATE INDEX IF NOT EXISTS idx_event_history_product_device_created_time
ON event_history (product_id, device_id, created_time DESC);

-- 设备当前状态表
CREATE TABLE IF NOT EXISTS device_status (
    device_id       TEXT,
    status          INT2 NOT NULL,
    ip_address      TEXT,
    last_online_at  TIMESTAMPTZ,
    last_offline_at TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    product_id      TEXT NOT NULL,
    PRIMARY KEY (product_id, device_id)
);

COMMENT ON COLUMN device_status.status IS 'device status: 0:offline, 1:online';

-- 为 device_status 创建索引
CREATE INDEX IF NOT EXISTS idx_device_status_status ON device_status (status);
CREATE INDEX IF NOT EXISTS idx_device_status_updated_at ON device_status (updated_at DESC);

-- 设备历史状态表
CREATE TABLE IF NOT EXISTS device_status_history (
    id              BIGSERIAL PRIMARY KEY,
    device_id       TEXT NOT NULL,
    status          INT2 NOT NULL,
    ip_address      TEXT,
    reason          TEXT,
    connected_at    TIMESTAMPTZ,
    disconnected_at TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    product_id      TEXT NOT NULL
);

COMMENT ON COLUMN device_status_history.status IS 'device status: 0:offline, 1:online';

-- 为 device_status_history 创建索引
CREATE INDEX IF NOT EXISTS idx_device_status_history_product_device_created_at ON device_status_history (product_id, device_id, created_at DESC);



-- Create event_valid_template table
CREATE TABLE event_valid_template (
    id BIGSERIAL PRIMARY KEY,
    product_id VARCHAR NOT NULL,
    event TEXT NOT NULL,
    description TEXT,
    schema JSONB NOT NULL,
    status SMALLINT DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create a unique index on product_id and event for active templates
CREATE UNIQUE INDEX event_valid_template_active_product_id_event_id_idx ON event_valid_template (product_id, event) WHERE status = 1;

-- Certificate issue table
CREATE TABLE IF NOT EXISTS cert_issue (
    id BIGSERIAL PRIMARY KEY,
    product_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    pub_cert TEXT NOT NULL,
    start_at TIMESTAMPTZ NOT NULL,
    end_at TIMESTAMPTZ NOT NULL,
    status INT2 NOT NULL DEFAULT 0, -- 0: Normal, 2: Revoked
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

COMMENT ON COLUMN cert_issue.status IS '0: Normal, 2: Revoked';

CREATE INDEX IF NOT EXISTS idx_cert_issue_product_id ON cert_issue (product_id);
CREATE INDEX IF NOT EXISTS idx_cert_issue_device_id ON cert_issue (device_id);

-- Create product table
CREATE TABLE product (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    model_no TEXT NOT NULL,
    description TEXT,
    status SMALLINT NOT NULL DEFAULT 0, -- 0: Online, 1: Offline
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create unique index on model_no
CREATE UNIQUE INDEX model_no_index ON product (model_no);

-- 固件版本版本
create table ota_versions
(
    id          serial primary key,
    product_id  text        not null,
    key         text        not null default '',
    version     int         not null,
    max_version int,
    min_version int         not null,
    file_key    text        not null,
    bin_length  bigint      not null default 0,
    bin_md5     text        not null default '',
    log         jsonb,
    device_ids  text[],
    released_at timestamptz not null,
    status      smallint,
    created_at  timestamptz          default now(),
    updated_at  timestamptz          default now()
);
CREATE UNIQUE INDEX ota_versions_product_id_key_idx ON ota_versions (product_id, key);

-- 存储设备上报的软件版本
create table device_versions
(
    id              bigserial primary key,
    product_id      text not null,
    key             text not null default '',
    device_id       text not null,
    version         int  not null,
    last_updated_at timestamptz,
    created_at      timestamptz   default now(),
    updated_at      timestamptz   default now()
);
CREATE UNIQUE INDEX device_versions_product_id_device_id_key_idx ON device_versions (product_id, device_id, key);
