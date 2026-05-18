#!/usr/bin/env python
import subprocess
import sys

from lib import docker
from lib.cli import require_executable
from lib.herald import init_permissions as init_herald_permissions
from lib.net import wait_for_http_ok
from lib.paths import LOG_DIR, REPO_ROOT, RUNTIME_DIR, ensure_dir
from lib.proc import spawn_background


def main() -> int:
    ensure_dir(LOG_DIR)
    backend_log = LOG_DIR / "backend.log"
    frontend_log = LOG_DIR / "frontend.log"

    if docker.container_exists("t-dev-postgres"):
        docker.rm_force_container("t-dev-postgres")
    if not docker.run_detached(
        [
            "--name",
            "t-dev-postgres",
            "-e",
            "POSTGRES_USER=postgres",
            "-e",
            "POSTGRES_PASSWORD=password",
            "-e",
            "POSTGRES_DB=t_db",
            "-p",
            "5432:5432",
            "postgres:18-alpine",
        ]
    ):
        print("ERROR: PostgreSQL container start failed")
        return 1
    if not docker.wait_pg_ready("t-dev-postgres", "postgres"):
        print("ERROR: PostgreSQL failed to start")
        return 1

    # Create Herald database in shared PostgreSQL
    import subprocess
    result = subprocess.run(
        ["docker", "exec", "t-dev-postgres", "psql", "-U", "postgres", "-c", "CREATE DATABASE herald_dev"],
        capture_output=True, text=True,
    )
    if result.returncode == 0 or "already exists" in result.stderr:
        print("Herald database ready")
    else:
        print(f"WARN: Failed to create Herald database: {result.stderr.strip()}")

    if docker.container_exists("t-dev-redis"):
        docker.rm_force_container("t-dev-redis")
    if not docker.run_detached(["--name", "t-dev-redis", "-p", "6379:6379", "redis:8.4-alpine"]):
        print("ERROR: Redis container start failed")
        return 1
    if not docker.wait_redis_ready("t-dev-redis"):
        print("ERROR: Redis failed to start")
        return 1

    if docker.container_exists("t-dev-herald"):
        docker.rm_force_container("t-dev-herald")

    # Generate Herald config
    herald_conf_dir = ensure_dir(RUNTIME_DIR / "dev-config" / "herald")
    (herald_conf_dir / "config.toml").write_text(
        """\
[database]
url = "postgresql://postgres:password@host.docker.internal:5432/herald_dev?sslmode=disable"

[redis]
url = "redis://host.docker.internal:6379"

[server]
bind_address = "0.0.0.0:3000"
log_level = "warn"
app_env = "development"

[frontend]
url = "http://localhost:3000"
""",
        encoding="utf-8",
    )

    cid = docker.create_container(
        [
            "--name",
            "t-dev-herald",
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
            "13000:3000",
            "ghcr.io/timzaak/herald:0.1.4",
        ]
    )
    if not cid:
        print("ERROR: Herald container create failed")
        return 1

    if not docker.copy_into_container(cid, str((herald_conf_dir / "config.toml").resolve()), "/app/config/config.toml"):
        print("ERROR: Herald config copy failed")
        return 1

    if not docker.start_container(cid):
        print("ERROR: Herald container start failed")
        return 1

    # Wait for Herald health check
    if not wait_for_http_ok("http://127.0.0.1:13000/health", 60):
        print("ERROR: Herald failed to start")
        return 1
    print("Herald is ready")

    if not init_herald_permissions("t-dev-postgres", "postgres", "herald_dev"):
        return 1

    cargo = require_executable("cargo")
    npm = require_executable("npm", windows_fallback="npm.cmd")

    spawn_background(
        name="dev-backend",
        command=[cargo, "run", "--bin", "rmqtt-things"],
        cwd=REPO_ROOT / "backend",
        stdout_path=backend_log,
    )

    spawn_background(
        name="dev-frontend",
        command=[npm, "run", "dev"],
        cwd=REPO_ROOT / "frontend",
        stdout_path=frontend_log,
    )
    print(
        f"Development environment started. Frontend=http://localhost:3000 Backend=http://localhost:8080 "
        f"Herald=http://localhost:13000 Logs={backend_log},{frontend_log}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
