"""
Demo 环境管理模块。

提供启动、停止和检查 Demo 测试环境健康状态的功能。
"""

import os
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

from . import docker
from .cli import require_executable
from .net import wait_for_http_ok, wait_for_tcp
from .paths import LOG_DIR, REPO_ROOT, SCRIPTS_DIR, ensure_dir
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .logger import Logger


@dataclass
class HealthStatus:
    """健康检查结果。"""

    healthy: bool = False
    services: dict[str, str] = field(default_factory=dict)
    errors: list[str] = field(default_factory=list)

    def add_service(self, name: str, status: str) -> None:
        """添加服务状态。"""
        self.services[name] = status

    def add_error(self, error: str) -> None:
        """添加错误信息。"""
        self.errors.append(error)

# Demo 环境端口和容器名称
BACKEND_PORT = 8080
FRONTEND_PORT = 3000
RMQTT_HTTP_PORT = 6060
POSTGRES_CONTAINER = "t-demo-postgres"
REDIS_CONTAINER = "t-demo-redis"
RMQTT_CONTAINER = "t-demo-rmqtt"
RMQTT_IMAGE = "rmqtt/rmqtt:0.20.0"
LOCALSTACK_CONTAINER = "t-demo-localstack"
LOCALSTACK_PORT = 4566


def check_postgres_container(status: HealthStatus) -> bool:
    """检查 PostgreSQL 容器状态。"""
    if not docker.container_exists(POSTGRES_CONTAINER):
        status.add_error(f"PostgreSQL container '{POSTGRES_CONTAINER}' not found")
        status.add_service("postgres", "not found")
        return False

    if not docker.container_running(POSTGRES_CONTAINER):
        status.add_error(f"PostgreSQL container '{POSTGRES_CONTAINER}' not running")
        status.add_service("postgres", "stopped")
        return False

    code, _ = docker.exec_check(POSTGRES_CONTAINER, ["pg_isready", "-U", "postgres"])
    if code != 0:
        status.add_error(f"PostgreSQL not ready (pg_isready failed)")
        status.add_service("postgres", "not ready")
        return False

    status.add_service("postgres", "healthy")
    return True


def check_redis_container(status: HealthStatus) -> bool:
    """检查 Redis 容器状态。"""
    if not docker.container_exists(REDIS_CONTAINER):
        status.add_error(f"Redis container '{REDIS_CONTAINER}' not found")
        status.add_service("redis", "not found")
        return False

    if not docker.container_running(REDIS_CONTAINER):
        status.add_error(f"Redis container '{REDIS_CONTAINER}' not running")
        status.add_service("redis", "stopped")
        return False

    code, out = docker.exec_check(REDIS_CONTAINER, ["redis-cli", "ping"])
    if code != 0 or out != "PONG":
        status.add_error(f"Redis not ready (ping failed: {out})")
        status.add_service("redis", "not ready")
        return False

    status.add_service("redis", "healthy")
    return True


def check_rmqtt_container(status: HealthStatus) -> bool:
    """检查 RMQTT 容器状态。"""
    if not docker.container_exists(RMQTT_CONTAINER):
        status.add_error(f"RMQTT container '{RMQTT_CONTAINER}' not found")
        status.add_service("rmqtt", "not found")
        return False

    if not docker.container_running(RMQTT_CONTAINER):
        status.add_error(f"RMQTT container '{RMQTT_CONTAINER}' not running")
        status.add_service("rmqtt", "stopped")
        return False

    if not wait_for_http_ok(f"http://127.0.0.1:{RMQTT_HTTP_PORT}/api/v1/stats", 2):
        status.add_error("RMQTT HTTP API health check failed")
        status.add_service("rmqtt", "health check failed")
        return False

    status.add_service("rmqtt", "healthy")
    return True


def check_localstack_container(status: HealthStatus) -> bool:
    """检查 LocalStack 容器状态。"""
    if not docker.container_exists(LOCALSTACK_CONTAINER):
        status.add_error(f"LocalStack container '{LOCALSTACK_CONTAINER}' not found")
        status.add_service("localstack", "not found")
        return False

    if not docker.container_running(LOCALSTACK_CONTAINER):
        status.add_error(f"LocalStack container '{LOCALSTACK_CONTAINER}' not running")
        status.add_service("localstack", "stopped")
        return False

    if not wait_for_http_ok(f"http://127.0.0.1:{LOCALSTACK_PORT}/_localstack/health", 2):
        status.add_error("LocalStack health check failed")
        status.add_service("localstack", "health check failed")
        return False

    status.add_service("localstack", "healthy")
    return True


def check_backend_process(status: HealthStatus) -> bool:
    """检查后端进程状态。"""
    # Skip state file check - check port and health endpoint directly
    # 检查端口是否可访问
    if not wait_for_tcp("127.0.0.1", BACKEND_PORT, 1):
        status.add_error(f"Backend port {BACKEND_PORT} not accessible")
        status.add_service("backend", "port not accessible")
        return False

    # 检查健康端点
    if not wait_for_http_ok(f"http://127.0.0.1:{BACKEND_PORT}/api/health", 2):
        status.add_error("Backend health check failed")
        status.add_service("backend", "health check failed")
        return False

    status.add_service("backend", "healthy")
    return True


def check_frontend_process(status: HealthStatus) -> bool:
    """检查前端进程状态（可选，不阻塞）。"""
    # Skip state file check - check port directly
    if not wait_for_http_ok(f"http://127.0.0.1:{FRONTEND_PORT}", 2):
        status.add_error("Frontend not ready")
        status.add_service("frontend", "not ready")
        return False

    status.add_service("frontend", "healthy")
    return True


def check_environment_health(require_frontend: bool = False) -> HealthStatus:
    """检查 Demo 环境是否运行且健康。

    Args:
        require_frontend: 是否要求前端必须健康（默认为 False，因为前端不是必需的）

    Returns:
        HealthStatus 对象，包含健康状态和服务详情
    """
    status = HealthStatus()

    # 检查 PostgreSQL
    pg_ok = check_postgres_container(status)

    # 检查 Redis
    redis_ok = check_redis_container(status)

    # 检查 RMQTT
    rmqtt_ok = check_rmqtt_container(status)

    # 检查 LocalStack
    localstack_ok = check_localstack_container(status)

    # 检查后端
    backend_ok = check_backend_process(status)

    # 检查前端（可选）
    if require_frontend:
        frontend_ok = check_frontend_process(status)
    else:
        # 不需要前端，直接跳过检查（避免不必要的网络等待）
        frontend_ok = True

    # 更新环境状态
    if pg_ok and redis_ok and rmqtt_ok and localstack_ok and backend_ok:
        status.healthy = True

    # Skip environment state file updating - it may be inaccurate
    return status


def seed_demo_data(logger: "Logger") -> bool:
    """Insert deterministic demo rows after backend migrations have run."""
    sql = r"""
INSERT INTO product (name, model_no, description, status)
VALUES ('Demo Smart Light', 'demo_product', 'Default product for RMQTT Things demo', 0)
ON CONFLICT (model_no) DO UPDATE
SET name = EXCLUDED.name,
    description = EXCLUDED.description,
    status = EXCLUDED.status,
    updated_at = NOW();

INSERT INTO device_status (product_id, device_id, status, ip_address, last_online_at, updated_at)
VALUES ('demo_product', 'demo-device', 1, '127.0.0.1', NOW(), NOW())
ON CONFLICT (product_id, device_id) DO UPDATE
SET status = EXCLUDED.status,
    ip_address = EXCLUDED.ip_address,
    last_online_at = EXCLUDED.last_online_at,
    updated_at = NOW();

INSERT INTO property_latest (product_id, device_id, properties, updated_time)
VALUES ('demo_product', 'demo-device', '{"temperature":23.5,"humidity":48,"power":true}'::jsonb, NOW())
ON CONFLICT (product_id, device_id) DO UPDATE
SET properties = EXCLUDED.properties,
    updated_time = NOW();

INSERT INTO property_history (product_id, device_id, properties, reported_time)
VALUES ('demo_product', 'demo-device', '{"temperature":23.5,"humidity":48,"power":true}'::jsonb, NOW());

INSERT INTO event_history (product_id, device_id, events, reported_time)
VALUES ('demo_product', 'demo-device', '{"event":"boot","reason":"demo-seed"}'::jsonb, NOW());

INSERT INTO event_valid_template (product_id, event, description, schema, status)
VALUES (
    'demo_product',
    'property',
    'Demo property schema',
    '{"type":"object","properties":{"temperature":{"type":"number"},"humidity":{"type":"number"},"power":{"type":"boolean"}}}'::jsonb,
    1
)
ON CONFLICT (product_id, event) WHERE status = 1 DO UPDATE
SET description = EXCLUDED.description,
    schema = EXCLUDED.schema,
    updated_at = NOW();

DELETE FROM ota_versions WHERE product_id = 'demo_product' AND key = 'main' AND version = 100001 AND status = 0;
INSERT INTO ota_versions (product_id, key, version, min_version, max_version, file_key, log, bin_length, bin_md5, released_at, status)
VALUES (
    'demo_product',
    'main',
    100001,
    100000,
    NULL,
    'public/demo-firmware.bin',
    '{"notes":"Demo OTA package"}'::jsonb,
    1024,
    'demo-md5',
    NOW(),
    0
);

INSERT INTO device_status_history (product_id, device_id, status, ip_address, connected_at, created_at)
VALUES ('demo_product', 'demo-device', 1, '127.0.0.1', NOW(), NOW());

INSERT INTO property_command (product_id, device_id, command, status, created_time, updated_time)
VALUES ('demo_product', 'demo-device', '{"power":false,"brightness":80}'::jsonb, 0, NOW(), NOW());
"""
    code, out = docker.exec_check(
        POSTGRES_CONTAINER,
        ["psql", "-U", "postgres", "-d", "t_demo", "-v", "ON_ERROR_STOP=1", "-c", sql],
    )
    if code != 0:
        logger.error(f"Demo data initialization failed: {out}")
        return False
    logger.verbose_info("Demo data initialized")
    return True


def start_rmqtt_container(logger: "Logger") -> bool:
    # Use create + cp + start instead of bind mount,
    # because Docker Desktop WSL2 may not support C: drive bind mounts.
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
            "1883:1883",
            "-p",
            f"{RMQTT_HTTP_PORT}:6060",
            RMQTT_IMAGE,
            "-f",
            "conf/rmqtt.toml",
        ]
    )
    if not cid:
        logger.error("RMQTT container create failed")
        return False

    conf_dir = str((REPO_ROOT / "conf").resolve())
    if not docker.copy_into_container(cid, conf_dir, "/app/rmqtt/conf"):
        logger.error("RMQTT config copy failed")
        return False

    if not docker.start_container(cid):
        logger.error("RMQTT container start failed")
        return False

    return True


def start_environment(
    logger: "Logger",
    timeout: int = 60,
) -> bool:
    """启动 Demo 环境并验证健康状态。

    Args:
        logger: Logger 实例（用于详细日志和性能分析）
        timeout: 启动超时时间（秒）

    Returns:
        启动成功返回 True，否则返回 False
    """
    logger.info("Starting Demo environment...")

    # Skip environment state file tracking - it may be inaccurate
    total_steps = 8

    # Step 1: Stop old environment (if running)
    with logger.step(1, total_steps, "Stopping old environment"):
        logger.verbose_info("Executing demo-stop.py script...")
        try:
            stop_result = subprocess.run(
                [sys.executable, str(SCRIPTS_DIR / "demo-stop.py"), "--quiet"],
                capture_output=True,
                timeout=30,  # Add 30 second timeout
            )
            logger.verbose_info(f"demo-stop.py completed with exit code: {stop_result.returncode}")
            if stop_result.stdout:
                logger.verbose_info(f"Stop stdout: {stop_result.stdout.decode('utf-8', errors='replace')}")
            if stop_result.stderr:
                logger.verbose_info(f"Stop stderr: {stop_result.stderr.decode('utf-8', errors='replace')}")

            if stop_result.returncode != 0:
                logger.error(f"Failed to stop old environment (exit code: {stop_result.returncode})")
                if stop_result.stderr:
                    logger.error(f"Stop error output: {stop_result.stderr.decode('utf-8', errors='replace')}")
                return False
        except subprocess.TimeoutExpired:
            logger.error("Old environment stop timed out after 30 seconds")
            return False
        except Exception as e:
            logger.error(f"Failed to stop old environment: {e}")
            return False

    # Step 2: Start PostgreSQL container
    with logger.step(2, total_steps, "Starting PostgreSQL"):
        if not docker.run_detached(
            [
                "--name",
                POSTGRES_CONTAINER,
                "--memory=1g",
                "--cpus=0.5",
                "--restart=unless-stopped",
                "--log-opt",
                "max-size=10m",
                "--log-opt",
                "max-file=3",
                "-e",
                "POSTGRES_USER=postgres",
                "-e",
                "POSTGRES_PASSWORD=postgres",
                "-e",
                "POSTGRES_DB=t_demo",
                "-p",
                "5432:5432",
                "postgres:18-alpine",
            ]
        ):
            logger.error("PostgreSQL container start failed")
            return False

        if not docker.wait_pg_ready(POSTGRES_CONTAINER, "postgres", logger=logger):
            logger.error("PostgreSQL failed to start")
            return False

    # Step 3: Start Redis container
    with logger.step(3, total_steps, "Starting Redis"):
        if not docker.run_detached(
            [
                "--name",
                REDIS_CONTAINER,
                "--memory=256m",
                "--cpus=0.25",
                "--restart=unless-stopped",
                "--log-opt",
                "max-size=10m",
                "--log-opt",
                "max-file=3",
                "-p",
                "6379:6379",
                "redis:8.4-alpine",
            ]
        ):
            logger.error("Redis container start failed")
            return False

        if not docker.wait_redis_ready(REDIS_CONTAINER, logger=logger):
            logger.error("Redis failed to start")
            return False

        # Wait for Redis to fully initialize
        time.sleep(5)

    # Step 4: Start LocalStack container
    with logger.step(4, total_steps, "Starting LocalStack"):
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
            logger.error("LocalStack container start failed")
            return False

        if not wait_for_http_ok(
            f"http://127.0.0.1:{LOCALSTACK_PORT}/_localstack/health",
            60,
            logger=logger,
        ):
            logger.error("LocalStack failed to start")
            return False

    # Prepare log paths
    backend_log_base = LOG_DIR / "backend-demo.log"
    frontend_log_base = LOG_DIR / "frontend-demo.log"
    backend_out = backend_log_base.with_suffix(".log.out")
    backend_err = backend_log_base.with_suffix(".log.err")
    frontend_out = frontend_log_base.with_suffix(".log.out")
    frontend_err = frontend_log_base.with_suffix(".log.err")

    cargo = require_executable("cargo")
    npm = require_executable("npm", windows_fallback="npm.cmd")

    # Step 5: Start backend process
    with logger.step(5, total_steps, "Starting backend"):
        backend_env = dict(os.environ)
        backend_env["APP_CONFIG"] = str((REPO_ROOT / "backend" / "config.demo.toml").resolve())
        backend_env["TOTP_SECRET_KEY"] = "demo-totp-encryption-key-32-bytes-long"
        backend_env["ADMIN_REALM_ID"] = "admin"

        from .proc import spawn_background

        spawn_background(
            name=None,
            command=[cargo, "run", "--bin", "rmqtt-things"],
            cwd=REPO_ROOT / "backend",
            stdout_path=backend_out,
            stderr_path=backend_err,
            env=backend_env,
        )

        backend_timeout = max(timeout, 180)
        if not wait_for_tcp("127.0.0.1", BACKEND_PORT, backend_timeout, logger=logger):
            logger.error(f"Backend start failed. Check {backend_out}")
            return False

        # Wait for backend to be fully healthy
        time.sleep(5)
        if not wait_for_http_ok(f"http://127.0.0.1:{BACKEND_PORT}/api/health", 30, logger=logger):
            logger.error(f"Backend health check failed. Check {backend_out}")
            return False

    # Step 6: Seed demo data
    with logger.step(6, total_steps, "Initializing demo data"):
        if not seed_demo_data(logger):
            return False

    # Step 7: Start RMQTT container
    with logger.step(7, total_steps, "Starting RMQTT"):
        if not start_rmqtt_container(logger):
            return False

        if not wait_for_http_ok(f"http://127.0.0.1:{RMQTT_HTTP_PORT}/api/v1/stats", 60, logger=logger):
            logger.error("RMQTT HTTP API failed to start")
            return False

    # Step 8: Start frontend process
    with logger.step(8, total_steps, "Starting frontend"):
        spawn_background(
            name=None,
            command=[npm, "run", "dev"],
            cwd=REPO_ROOT / "frontend",
            stdout_path=frontend_out,
            stderr_path=frontend_err,
        )

        if not wait_for_http_ok(f"http://127.0.0.1:{FRONTEND_PORT}", timeout, logger=logger):
            logger.warning(f"Frontend not ready within {timeout} seconds")
            logger.verbose_info(f"Check logs: {frontend_out}, {frontend_err}")

    # Verify health
    health_status = check_environment_health(require_frontend=True)
    if not health_status.healthy:
        logger.error("Environment health check failed")
        for error in health_status.errors:
            logger.error(f"  - {error}")
        return False

    # Skip environment state file saving - it may be inaccurate
    logger.info("Demo Environment started")
    return True


def stop_environment() -> bool:
    """优雅停止 Demo 环境。

    Returns:
        停止成功返回 True，否则返回 False
    """
    print("Stopping Demo environment...")

    # Skip environment state file clearing - it may be inaccurate

    # 停止后端进程 - use port instead of state file
    print("  Stopping backend...")
    # Kill processes by port 8080
    try:
        if sys.platform == "win32":
            # Windows
            result = subprocess.run(
                ["powershell", "-NoProfile", "-Command",
                 f"Get-NetTCPConnection -LocalPort {BACKEND_PORT} -State Listen -ErrorAction SilentlyContinue | "
                 "Select-Object -ExpandProperty OwningProcess | ForEach-Object { taskkill /PID $_ /F /T }"],
                capture_output=True,
                check=False,
                timeout=5,
            )
        else:
            # Unix-like
            for cmd in [["lsof", "-ti", f":{BACKEND_PORT}"], ["fuser", "-k", f"{BACKEND_PORT}/tcp"]]:
                try:
                    subprocess.run(cmd, capture_output=True, check=False)
                    break
                except FileNotFoundError:
                    continue
    except Exception:
        pass  # Ignore errors, best-effort cleanup

    # 停止前端进程 - use demo ports instead of state file
    print("  Stopping frontend...")
    for port in [3000, 3001, 3002, 3003]:
        try:
            if sys.platform == "win32":
                result = subprocess.run(
                    ["powershell", "-NoProfile", "-Command",
                     f"Get-NetTCPConnection -LocalPort {port} -State Listen -ErrorAction SilentlyContinue | "
                     "Select-Object -ExpandProperty OwningProcess | ForEach-Object { taskkill /PID $_ /F /T }"],
                    capture_output=True,
                    check=False,
                    timeout=5,
                )
            else:
                for cmd in [["lsof", "-ti", f":{port}"], ["fuser", "-k", f"{port}/tcp"]]:
                    try:
                        subprocess.run(cmd, capture_output=True, check=False)
                        break
                    except FileNotFoundError:
                        continue
        except Exception:
            pass  # Ignore errors, best-effort cleanup

    # 停止 Docker 容器
    print("  Stopping containers...")
    if docker.container_exists(LOCALSTACK_CONTAINER):
        docker.stop_container(LOCALSTACK_CONTAINER)
    if docker.container_exists(REDIS_CONTAINER):
        docker.stop_container(REDIS_CONTAINER)
    if docker.container_exists(POSTGRES_CONTAINER):
        docker.stop_container(POSTGRES_CONTAINER)

    time.sleep(1.0)

    if docker.container_exists(LOCALSTACK_CONTAINER):
        docker.rm_force_container(LOCALSTACK_CONTAINER)
    if docker.container_exists(REDIS_CONTAINER):
        docker.rm_force_container(REDIS_CONTAINER)
    if docker.container_exists(POSTGRES_CONTAINER):
        docker.rm_force_container(POSTGRES_CONTAINER)

    print("Demo environment stopped")
    return True


