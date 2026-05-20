#!/usr/bin/env python
import subprocess
import sys
import time
import urllib.error
import urllib.request

from lib import docker
from lib.herald import init_permissions as init_herald_permissions
from lib.net import is_port_open
from lib.paths import SCRIPTS_DIR, TEST_CONFIG_DIR, ensure_dir

POSTGRES_CONTAINER = "rmqtt-things-test-postgres"
PGDOG_CONTAINER = "rmqtt-things-test-pgdog"
LOCALSTACK_CONTAINER = "rmqtt-things-test-localstack"
RMQTT_CONTAINER = "rmqtt-things-test-rmqtt"
HERALD_CONTAINER = "rmqtt-things-test-herald"
REDIS_CONTAINER = "rmqtt-things-test-redis"

POSTGRES_PORT = 15433
PGDOG_PORT = 16432
LOCALSTACK_PORT = 14566
RMQTT_MQTT_PORT = 11883
RMQTT_HTTP_PORT = 16060
HERALD_PORT = 13000
REDIS_PORT = 16379
TEST_BACKEND_PORT = 18080

POSTGRES_USER = "rmqtt_user"
POSTGRES_PASSWORD = "rmqtt_pass"
POSTGRES_DB = "rmqtt_things"


def _ports_free() -> bool:
    ports = [POSTGRES_PORT, PGDOG_PORT, LOCALSTACK_PORT, RMQTT_MQTT_PORT, RMQTT_HTTP_PORT]
    occupied = [port for port in ports if is_port_open("127.0.0.1", port)]
    if not occupied:
        return True
    print("ERROR: Occupied test ports:", ", ".join(str(p) for p in occupied))
    return False


def _build_pgdog_bootstrap_command(pgdog_config: str, users_config: str) -> str:
    return f"""cat > /tmp/pgdog.toml <<'PGDOG_CONFIG'
{pgdog_config}
PGDOG_CONFIG
cat > /tmp/users.toml <<'USERS_CONFIG'
{users_config}
USERS_CONFIG
exec /usr/local/bin/pgdog -c /tmp/pgdog.toml -u /tmp/users.toml run
"""


def _print_pgdog_failure_diagnostics(last_probe_output: str) -> None:
    if last_probe_output:
        print(f"PgDog last probe output: {last_probe_output}")

    logs = subprocess.run(
        ["docker", "logs", PGDOG_CONTAINER, "--tail", "50"],
        capture_output=True,
        text=True,
    )
    log_output = (logs.stdout or logs.stderr).strip()
    if log_output:
        print("PgDog logs:")
        print(log_output)


def _start_pgdog() -> bool:
    print("Starting PgDog proxy...")

    pgdog_config = f"""[general]
host = "0.0.0.0"
port = 6432
workers = 2
default_pool_size = 32
min_pool_size = 1
checkout_timeout = 30000
idle_timeout = 600000
healthcheck_timeout = 5000
healthcheck_interval = 10000

[[databases]]
name = "{POSTGRES_DB}"
host = "host.docker.internal"
port = {POSTGRES_PORT}
database_name = "{POSTGRES_DB}"
user = "{POSTGRES_USER}"
password = "{POSTGRES_PASSWORD}"
pool_size = 32
min_pool_size = 1
"""

    users_config = f"""[[users]]
name = "{POSTGRES_USER}"
password = "{POSTGRES_PASSWORD}"
database = "{POSTGRES_DB}"
pooler_mode = "session"
pool_size = 32
min_pool_size = 1
"""

    bootstrap_cmd = _build_pgdog_bootstrap_command(pgdog_config, users_config)
    if not docker.run_detached(
        [
            "--name",
            PGDOG_CONTAINER,
            "--memory=256m",
            "--cpus=0.25",
            "--restart=unless-stopped",
            "--log-opt",
            "max-size=10m",
            "--log-opt",
            "max-file=3",
            "--add-host",
            "host.docker.internal:host-gateway",
            "-e",
            "RUST_LOG=error",
            "-e",
            "RUST_BACKTRACE=0",
            "-p",
            f"{PGDOG_PORT}:6432",
            "--entrypoint",
            "sh",
            "ghcr.io/pgdogdev/pgdog:v0.1.35",
            "-lc",
            bootstrap_cmd,
        ]
    ):
        print("ERROR: PgDog container failed to start")
        return False

    last_probe_output = ""
    for _ in range(30):
        code, out = docker.exec_check(
            POSTGRES_CONTAINER,
            [
                "psql",
                f"postgresql://{POSTGRES_USER}:{POSTGRES_PASSWORD}@host.docker.internal:{PGDOG_PORT}/{POSTGRES_DB}?sslmode=disable",
                "-c",
                "select 1",
            ],
        )
        last_probe_output = out
        if code == 0 and "1" in out:
            print("PgDog is ready")
            return True
        time.sleep(1)

    print("ERROR: PgDog failed to start")
    _print_pgdog_failure_diagnostics(last_probe_output)
    return False


def _cleanup_test_schemas() -> None:
    cleanup_cmd = [
        "docker",
        "exec",
        POSTGRES_CONTAINER,
        "psql",
        "-U",
        POSTGRES_USER,
        "-d",
        POSTGRES_DB,
        "-c",
        """DO $$
DECLARE
    schema_record RECORD;
BEGIN
    FOR schema_record IN
        SELECT schema_name FROM information_schema.schemata
        WHERE schema_name LIKE 'test_%'
    LOOP
        EXECUTE 'DROP SCHEMA IF EXISTS "' || schema_record.schema_name || '" CASCADE';
        RAISE NOTICE 'Dropped schema: %', schema_record.schema_name;
    END LOOP;
END $$;""",
    ]
    last_error = ""
    for _ in range(10):
        result = subprocess.run(cleanup_cmd, capture_output=True, text=True)
        if result.returncode == 0:
            print("Test schema cleanup completed")
            return
        last_error = result.stderr
        time.sleep(1)
    print(f"WARN: Test schema cleanup had issues: {last_error}")


def _start_localstack() -> bool:
    print("Starting LocalStack...")
    if not docker.run_detached(
        [
            "--name",
            LOCALSTACK_CONTAINER,
            "--memory=512m",
            "--cpus=0.5",
            "--restart=unless-stopped",
            "--log-opt",
            "max-size=10m",
            "--log-opt",
            "max-file=3",
            "-e",
            "SERVICES=s3",
            "-p",
            f"{LOCALSTACK_PORT}:4566",
            "localstack/localstack:4.9.2",
        ]
    ):
        print("ERROR: LocalStack test container failed to start")
        return False

    health_url = f"http://127.0.0.1:{LOCALSTACK_PORT}/_localstack/health"
    for _ in range(60):
        try:
            with urllib.request.urlopen(health_url, timeout=2) as response:
                if response.status == 200:
                    print("LocalStack is ready")
                    return True
        except (ConnectionError, TimeoutError, urllib.error.URLError):
            pass
        time.sleep(1)

    print("ERROR: LocalStack failed to start")
    return False


def _generate_rmqtt_test_config(backend_port: int) -> object:
    """Generate RMQTT test config files with webhook URLs pointing to the test backend."""
    from pathlib import Path

    conf_dir = ensure_dir(TEST_CONFIG_DIR / "rmqtt")
    plugins_dir = ensure_dir(conf_dir / "plugins")

    # rmqtt.toml - same as conf/rmqtt.toml but with test settings
    (conf_dir / "rmqtt.toml").write_text(
        f"""task.exec_workers = 2000
task.exec_queue_max = 300_000

node.id = 1
node.busy.check_enable = true
node.busy.update_interval = "2s"
node.busy.loadavg = 80.0
node.busy.cpuloadavg = 90.0
node.busy.handshaking = 0

rpc.server_addr = "0.0.0.0:5363"
rpc.server_workers = 4

log.to = "console"
log.level = "warn"
log.dir = "/var/log/rmqtt"
log.file = "rmqtt.log"

plugins.dir = "conf/plugins/"
plugins.default_startups = [
    "rmqtt-http-api",
    "rmqtt-auth-http",
    "rmqtt-counter",
    "rmqtt-web-hook",
    "rmqtt-auto-subscription",
]

mqtt.delayed_publish_max = 100_000
mqtt.delayed_publish_immediate = true
mqtt.max_sessions = 0

listener.tcp.external.addr = "0.0.0.0:1883"
listener.tcp.external.workers = 4
listener.tcp.external.max_connections = 10000
listener.tcp.external.max_handshaking_limit = 500
listener.tcp.external.handshake_timeout = "30s"
listener.tcp.external.max_packet_size = "1MB"
listener.tcp.external.backlog = 1024
listener.tcp.external.nodelay = false
listener.tcp.external.allow_anonymous = true
listener.tcp.external.allow_zero_keepalive = true
listener.tcp.external.min_keepalive = 0
listener.tcp.external.max_keepalive = 65535
listener.tcp.external.keepalive_backoff = 0.75
listener.tcp.external.max_inflight = 16
listener.tcp.external.max_mqueue_len = 1000
listener.tcp.external.mqueue_rate_limit = "1000,1s"
listener.tcp.external.max_clientid_len = 65535
listener.tcp.external.max_qos_allowed = 2
listener.tcp.external.max_topic_levels = 0
listener.tcp.external.session_expiry_interval = "2h"
listener.tcp.external.max_session_expiry_interval = "0h"
listener.tcp.external.message_retry_interval = "20s"
listener.tcp.external.message_expiry_interval = "5m"
listener.tcp.external.max_subscriptions = 0
listener.tcp.external.shared_subscription = true
listener.tcp.external.max_topic_aliases = 32
listener.tcp.external.limit_subscription = false
listener.tcp.external.delayed_publish = false
""",
        encoding="utf-8",
    )

    backend_host = f"host.docker.internal:{backend_port}"

    # rmqtt-http-api.toml
    (plugins_dir / "rmqtt-http-api.toml").write_text(
        """http_laddr = "0.0.0.0:6060"
http_request_log = false
message_expiry_interval = "5m"
max_row_limit = 10_000
""",
        encoding="utf-8",
    )

    # rmqtt-auth-http.toml - point to test backend
    (plugins_dir / "rmqtt-auth-http.toml").write_text(
        f"""http_timeout = "5s"
http_headers.accept = "*/*"
http_headers.Cache-Control = "no-cache"
http_headers.User-Agent = "RMQTT/0.20.0"
http_headers.Connection = "keep-alive"
disconnect_if_pub_rejected = true
disconnect_if_expiry = false
deny_if_error = true

http_auth_req.url = "http://{backend_host}/api/access/auth"
http_auth_req.method = "post"
http_auth_req.headers.content-type = "application/json"
http_auth_req.headers.x-real-ip = "127.0.0.1"
http_auth_req.params = {{ client_id = "%c", username = "%u", password = "%P", protocol = "%r", ipaddress = "%a" }}

http_acl_req.url = "http://{backend_host}/api/access/acl"
http_acl_req.method = "post"
http_acl_req.headers.content-type = "application/json"
http_acl_req.headers.x-real-ip = "127.0.0.1"
http_acl_req.params = {{ access = "%A", username = "%u", client_id = "%c", ip = "%a", topic = "%t", protocol = "%r" }}
""",
        encoding="utf-8",
    )

    # rmqtt-web-hook.toml - point to test backend
    (plugins_dir / "rmqtt-web-hook.toml").write_text(
        f"""worker_threads = 4
queue_capacity = 300_000
concurrency_limit = 128
http_timeout = "8s"
retry_max_elapsed_time = "60s"
retry_multiplier = 2.5

rule.client_connected = [{{action = "client_connected", urls = ["http://{backend_host}/api/device/connect"]}}]
rule.client_disconnected = [{{action = "client_disconnected", urls = ["http://{backend_host}/api/device/disconnect"]}}]

rule.message_publish = [
    {{action = "message_publish", topics=["+/+/thing/event/property/post"], urls = ["http://{backend_host}/api/thing/property/post"]}},
    {{action = "message_publish", topics=["+/+/thing/event/test/post"], urls = ["http://{backend_host}/api/thing/event/post"]}},
    {{action = "message_publish", topics=["+/+/thing/service/property/set_reply"], urls = ["http://{backend_host}/api/thing/property/set_reply"]}},
    {{action = "message_publish", topics=["+/+/thing/file/upload"], urls = ["http://{backend_host}/api/thing/file/upload"]}},
    {{action = "message_publish", topics=["+/+/ota/version"], urls = ["http://{backend_host}/api/ota/version"]}}
]

rule.client_subscribe = [
    {{action = "client_subscribe", topics=["+/+/thing/service/property/set"], urls = ["http://{backend_host}/api/thing/property/set_subscribe"]}}
]
""",
        encoding="utf-8",
    )

    # rmqtt-auto-subscription.toml - same as demo
    (plugins_dir / "rmqtt-auto-subscription.toml").write_text(
        """subscribes = [
    {topic_filter = "+/${clientid}/thing/service/property/set", qos = 2, no_local = true},
    {topic_filter = "+/${clientid}/thing/event/property/post_reply", qos = 2, no_local = true},
    {topic_filter = "+/${clientid}/thing/event/file/upload_reply", qos = 2, no_local = true},
    {topic_filter = "+/${clientid}/ota/upgrade", qos = 2, no_local = true},
    {topic_filter = "+/${clientid}/ota/version_reply", qos = 2, no_local = true}
]
""",
        encoding="utf-8",
    )

    # rmqtt-acl.toml - deny by default, allow devices on their own topics
    (plugins_dir / "rmqtt-acl.toml").write_text(
        """disconnect_if_pub_rejected = true
rules = [
    ["deny", "all", "subscribe", ["$SYS/#", { eq = "#" }]],
    ["allow", "all", "pubsub", ["%c/#", "/%c/#"]],
    ["deny", "all"]
]
""",
        encoding="utf-8",
    )

    return conf_dir


def _start_redis() -> bool:
    print("Starting Redis...")
    if not docker.run_detached(
        [
            "--name",
            REDIS_CONTAINER,
            "--memory=128m",
            "--cpus=0.25",
            "--restart=unless-stopped",
            "--log-opt",
            "max-size=10m",
            "--log-opt",
            "max-file=3",
            "-p",
            f"{REDIS_PORT}:6379",
            "redis:8.4-alpine",
        ]
    ):
        print("ERROR: Redis test container failed to start")
        return False

    for _ in range(30):
        result = subprocess.run(
            ["docker", "exec", REDIS_CONTAINER, "redis-cli", "ping"],
            capture_output=True,
            text=True,
        )
        if result.returncode == 0 and "PONG" in result.stdout:
            print("Redis is ready")
            return True
        time.sleep(1)

    print("ERROR: Redis failed to start")
    return False


def _generate_herald_config() -> str:
    """Generate Herald config directory with database and redis settings."""
    conf_dir = ensure_dir(TEST_CONFIG_DIR / "herald")
    (conf_dir / "config.toml").write_text(
        f"""[database]
url = "postgresql://{POSTGRES_USER}:{POSTGRES_PASSWORD}@host.docker.internal:{POSTGRES_PORT}/herald_test?sslmode=disable"

[redis]
url = "redis://host.docker.internal:{REDIS_PORT}"

[server]
bind_address = "0.0.0.0:3000"
log_level = "warn"
app_env = "test"

[frontend]
url = "http://localhost:3000"
""",
        encoding="utf-8",
    )
    return str(conf_dir.resolve())


def _start_herald() -> bool:
    print("Starting Herald...")
    conf_dir = _generate_herald_config()

    cid = docker.create_container(
        [
            "--name",
            HERALD_CONTAINER,
            "--memory=512m",
            "--cpus=0.5",
            "--restart=unless-stopped",
            "--add-host",
            "host.docker.internal:host-gateway",
            "--log-opt",
            "max-size=10m",
            "--log-opt",
            "max-file=3",
            "-e",
            "HERALD_CONFIG=/app/config/config.toml",
            "-p",
            f"{HERALD_PORT}:3000",
            "ghcr.io/timzaak/herald:0.1.4",
        ]
    )
    if not cid:
        print("ERROR: Herald container create failed")
        return False

    config_file = str((TEST_CONFIG_DIR / "herald" / "config.toml").resolve())
    if not docker.copy_into_container(cid, config_file, "/app/config/config.toml"):
        print("ERROR: Herald config copy failed")
        return False

    if not docker.start_container(cid):
        print("ERROR: Herald container start failed")
        return False

    health_url = f"http://127.0.0.1:{HERALD_PORT}/health"
    for _ in range(60):
        try:
            with urllib.request.urlopen(health_url, timeout=2) as response:
                if response.status == 200:
                    print("Herald is ready")
                    return True
        except (ConnectionError, TimeoutError, urllib.error.URLError):
            pass
        time.sleep(1)

    print("ERROR: Herald failed to start")
    return False


def _start_rmqtt() -> bool:
    print("Starting RMQTT...")
    backend_port = TEST_BACKEND_PORT

    # Generate test-specific config
    conf_dir = _generate_rmqtt_test_config(backend_port)

    # Create + copy config + start (same pattern as demo_env.py)
    cid = docker.create_container(
        [
            "--name",
            RMQTT_CONTAINER,
            "--memory=512m",
            "--cpus=0.5",
            "--restart=unless-stopped",
            "--add-host",
            "host.docker.internal:host-gateway",
            "--log-opt",
            "max-size=10m",
            "--log-opt",
            "max-file=3",
            "-p",
            f"{RMQTT_MQTT_PORT}:1883",
            "-p",
            f"{RMQTT_HTTP_PORT}:6060",
            "rmqtt/rmqtt:0.20.0",
            "-f",
            "conf/rmqtt.toml",
        ]
    )
    if not cid:
        print("ERROR: RMQTT container create failed")
        return False

    conf_dir_str = str(conf_dir.resolve())
    if not docker.copy_into_container(cid, conf_dir_str, "/app/rmqtt/conf"):
        print("ERROR: RMQTT config copy failed")
        return False

    if not docker.start_container(cid):
        print("ERROR: RMQTT container start failed")
        return False

    stats_url = f"http://127.0.0.1:{RMQTT_HTTP_PORT}/api/v1/stats"
    for _ in range(60):
        try:
            with urllib.request.urlopen(stats_url, timeout=2) as response:
                if response.status == 200:
                    print("RMQTT is ready")
                    return True
        except (ConnectionError, TimeoutError, urllib.error.URLError):
            pass
        time.sleep(1)

    print("ERROR: RMQTT failed to start")
    return False


def main() -> int:
    stop_result = subprocess.run([sys.executable, str(SCRIPTS_DIR / "test-stop.py")])
    if stop_result.returncode != 0:
        return stop_result.returncode

    if not _ports_free():
        return 1

    if not docker.run_detached(
        [
            "--name",
            POSTGRES_CONTAINER,
            "--memory=1g",
            "--cpus=0.5",
            "--restart=unless-stopped",
            "--add-host",
            "host.docker.internal:host-gateway",
            "--log-opt",
            "max-size=10m",
            "--log-opt",
            "max-file=3",
            "-e",
            f"POSTGRES_USER={POSTGRES_USER}",
            "-e",
            f"POSTGRES_PASSWORD={POSTGRES_PASSWORD}",
            "-e",
            f"POSTGRES_DB={POSTGRES_DB}",
            "-p",
            f"{POSTGRES_PORT}:5432",
            "postgres:18-alpine",
        ]
    ):
        print("ERROR: PostgreSQL test container failed to start")
        return 1

    if not docker.wait_pg_ready(POSTGRES_CONTAINER, POSTGRES_USER):
        print("ERROR: PostgreSQL test container failed to start")
        return 1

    _cleanup_test_schemas()

    # Create Herald database in shared PostgreSQL
    print("Creating Herald database...")
    code, out = docker.exec_check(
        POSTGRES_CONTAINER,
        ["psql", "-U", POSTGRES_USER, "-d", POSTGRES_DB, "-c", "CREATE DATABASE herald_test"],
    )
    if code == 0 or "already exists" in out:
        print("Herald database ready")
    else:
        print(f"WARN: Failed to create Herald database: {out}")

    if not _start_pgdog():
        return 1

    code, out = docker.exec_check(
        POSTGRES_CONTAINER,
        [
            "psql",
            f"postgresql://{POSTGRES_USER}:{POSTGRES_PASSWORD}@host.docker.internal:{PGDOG_PORT}/{POSTGRES_DB}?sslmode=disable",
            "-c",
            "select 1",
        ],
    )
    if code != 0 or "1" not in out:
        print("ERROR: PgDog verification failed")
        return 1

    if not _start_localstack():
        return 1

    if not _start_rmqtt():
        return 1

    if not _start_redis():
        return 1

    if not _start_herald():
        return 1

    if not init_herald_permissions(POSTGRES_CONTAINER, POSTGRES_USER, "herald_test"):
        return 1

    print(
        "Test environment is ready. "
        f"PgDog=localhost:{PGDOG_PORT} LocalStack=localhost:{LOCALSTACK_PORT} "
        f"RMQTT=localhost:{RMQTT_MQTT_PORT}/{RMQTT_HTTP_PORT} Herald=localhost:{HERALD_PORT}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
