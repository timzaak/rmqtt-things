#!/usr/bin/env python
import sys

from lib import docker

TEST_CONTAINERS = [
    "rmqtt-things-test-pgdog",
    "rmqtt-things-test-postgres",
    "rmqtt-things-test-localstack",
    "rmqtt-things-test-rmqtt",
]


def main() -> int:
    for container in TEST_CONTAINERS:
        if docker.container_running(container):
            docker.stop_container(container)
        if docker.container_exists(container):
            docker.rm_container(container)

    print("Backend test environment stopped")
    return 0


if __name__ == "__main__":
    sys.exit(main())
