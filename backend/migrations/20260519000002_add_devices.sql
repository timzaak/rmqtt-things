-- 新增设备身份注册表
CREATE TABLE devices (
    id BIGSERIAL PRIMARY KEY,
    product_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    registration_source INT2 NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(product_id, device_id)
);

CREATE INDEX ix_devices_product_id ON devices (product_id);

-- 产品表增加自动注册开关
ALTER TABLE product ADD COLUMN auto_provisioning BOOLEAN NOT NULL DEFAULT false;
