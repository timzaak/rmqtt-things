#!/usr/bin/env python3
"""Validate that a frontend/miniapp/demo runner item's Expected Test Manifest
is non-empty and references existing authoring items.

Usage:
    uv run scripts/check-test-runner-coverage.py <feature> --layer <backend|frontend|miniapp|demo>

Exit codes:
    0 - coverage manifest is valid
    1 - state file missing, runner item missing, manifest empty, or source item unknown
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


def _repo_root() -> Path:
    return Path.cwd()


def _load_state(feature: str) -> dict:
    state_path = _repo_root() / ".ai" / "task" / feature / ".state.json"
    if not state_path.exists():
        print(f"ERROR: state file not found: {state_path}", file=sys.stderr)
        sys.exit(1)
    return json.loads(state_path.read_text(encoding="utf-8"))


def _all_items_in_layer(state: dict, layer: str) -> dict[str, dict]:
    """Return a flat map of item_id -> item state for the given phase/layer."""
    items: dict[str, dict] = {}
    layer_data = state.get("tasks", {}).get(layer, {})
    if not isinstance(layer_data, dict):
        return items
    for slot_name, slot_data in layer_data.items():
        if not isinstance(slot_data, dict):
            continue
        for item_id, item in slot_data.get("items", {}).items():
            if isinstance(item, dict):
                items[item_id] = item
    return items


def _find_runner_item(feature: str, layer: str) -> Path | None:
    """Find a markdown item file containing an 'Expected Test Manifest' section."""
    layer_dir = _repo_root() / ".ai" / "task" / feature / layer
    if not layer_dir.exists():
        return None
    for slot_dir in layer_dir.iterdir():
        if not slot_dir.is_dir():
            continue
        for md_file in sorted(slot_dir.glob("*.md")):
            try:
                content = md_file.read_text(encoding="utf-8")
            except Exception:
                continue
            if "Expected Test Manifest" in content:
                return md_file
    return None


def _extract_manifest_table(content: str) -> list[dict[str, str]]:
    """Extract the first markdown table found after the 'Expected Test Manifest' heading."""
    marker = "Expected Test Manifest"
    idx = content.find(marker)
    if idx == -1:
        return []

    after = content[idx:]
    # Find the first table block: lines starting with | and containing |
    lines = after.splitlines()
    table_lines: list[str] = []
    in_table = False
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("|") and "|" in stripped[1:]:
            table_lines.append(stripped)
            in_table = True
        elif in_table:
            break

    if len(table_lines) < 3:  # header + separator + at least one row
        return []

    header = [cell.strip() for cell in table_lines[0].split("|")[1:-1]]
    rows = []
    for line in table_lines[2:]:
        cells = [cell.strip() for cell in line.split("|")[1:-1]]
        if len(cells) != len(header):
            continue
        rows.append(dict(zip(header, cells)))
    return rows


def _normalize_source_item(raw: str) -> str:
    return raw.strip().strip("`[]()")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Validate runner Expected Test Manifest coverage."
    )
    parser.add_argument("feature", help="Feature name")
    parser.add_argument(
        "--layer",
        required=True,
        choices=["backend", "frontend", "miniapp", "demo"],
        help="Phase/layer to check",
    )
    args = parser.parse_args(argv)

    state = _load_state(args.feature)
    items = _all_items_in_layer(state, args.layer)

    runner_path = _find_runner_item(args.feature, args.layer)
    if runner_path is None:
        print(
            f"ERROR: no runner item with 'Expected Test Manifest' found for "
            f"{args.feature}/{args.layer}",
            file=sys.stderr,
        )
        return 1

    content = runner_path.read_text(encoding="utf-8")
    manifest = _extract_manifest_table(content)

    if not manifest:
        print(
            f"ERROR: Expected Test Manifest is empty in {runner_path}",
            file=sys.stderr,
        )
        return 1

    errors: list[str] = []
    for row in manifest:
        # The source item column may be named differently in authoring vs runner contexts.
        source_raw = (
            row.get("source_item")
            or row.get("来源 authoring item")
            or row.get("来源")
            or ""
        )
        source = _normalize_source_item(source_raw)
        if not source:
            errors.append(f"row missing source_item: {row}")
            continue
        if source not in items:
            errors.append(
                f"source authoring item '{source}' not found in .state.json "
                f"for {args.layer}"
            )

    if errors:
        print(f"ERROR: coverage check failed for {runner_path}", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        return 1

    print(f"OK: {len(manifest)} test case(s) covered in {runner_path}")
    for row in manifest:
        case_title = (
            row.get("用例标题（describe > it）")
            or row.get("test_case")
            or row.get("用例标题")
            or "<unknown>"
        )
        source = _normalize_source_item(
            row.get("source_item")
            or row.get("来源 authoring item")
            or row.get("来源")
            or ""
        )
        print(f"  - {case_title} (source: {source})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
