#!/usr/bin/env python3

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path
from typing import Sequence


MODULE_PATH = Path("MODULE.bazel")
CARGO_PATH = Path("Cargo.toml")


def normalize_root(root: Path) -> Path:
    resolved_root = root.resolve()
    return resolved_root.parent if resolved_root.is_file() else resolved_root


def find_workspace_root(start: Path) -> Path:
    path = start.resolve()
    if path.is_file():
        path = path.parent

    for candidate in (path, *path.parents):
        if (candidate / MODULE_PATH).is_file() and (candidate / CARGO_PATH).is_file():
            return candidate

    raise ValueError(f"could not find workspace root from {start}")


def default_root() -> Path:
    workspace_directory = os.environ.get("BUILD_WORKSPACE_DIRECTORY")
    if workspace_directory:
        return find_workspace_root(Path(workspace_directory))

    for candidate in (Path.cwd(), Path(__file__)):
        try:
            return find_workspace_root(candidate)
        except ValueError:
            continue

    raise ValueError("could not determine workspace root; pass --root explicitly")


def read_module_version(root: Path) -> str:
    module_text = (root / MODULE_PATH).read_text(encoding="utf-8")
    block_match = re.search(r"(?ms)^module\((.*?)^\)", module_text)
    if block_match is None:
        raise ValueError(f"could not find module() block in {MODULE_PATH}")

    version_match = re.search(
        r'(?m)^\s*version\s*=\s*"([^"]+)"\s*,?\s*$',
        block_match.group(1),
    )
    if version_match is None:
        raise ValueError(f"could not find module version in {MODULE_PATH}")
    return version_match.group(1)


def update_workspace_package_version(text: str, version: str) -> str:
    lines = text.splitlines(keepends=True)
    in_workspace_package = False

    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_workspace_package = stripped == "[workspace.package]"
            continue
        if in_workspace_package and stripped.startswith('version = "'):
            lines[index] = f'version = "{version}"\n'
            return "".join(lines)

    raise ValueError("could not find version in [workspace.package] section of Cargo.toml")


def sync_versions(root: Path, check: bool) -> int:
    version = read_module_version(root)
    dirty = False

    cargo_path = root / CARGO_PATH
    dirty |= sync_file(
        cargo_path,
        update_workspace_package_version(
            cargo_path.read_text(encoding="utf-8"),
            version,
        ),
        check,
    )

    if check and dirty:
        print(f"expected version: {version}")
        return 1

    if not dirty:
        print(f"version metadata already matches {version}")

    return 0


def sync_file(path: Path, updated_text: str, check: bool) -> bool:
    current_text = path.read_text(encoding="utf-8")
    if current_text == updated_text:
        return False

    if check:
        print(f"out of sync: {path}")
        return True

    path.write_text(updated_text, encoding="utf-8")
    print(f"updated {path}")
    return True


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Sync the Cargo workspace version from MODULE.bazel.",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail instead of rewriting files when versions are out of sync",
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=None,
        help="workspace root containing MODULE.bazel",
    )
    args = parser.parse_args(argv)

    root = default_root() if args.root is None else find_workspace_root(normalize_root(args.root))
    return sync_versions(root, args.check)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except ValueError as error:
        print(error, file=sys.stderr)
        sys.exit(1)
