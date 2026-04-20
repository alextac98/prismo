#!/usr/bin/env python3

from __future__ import annotations

import os
import pathlib
import re
import subprocess
import sys


MODULE_PATH = pathlib.Path("MODULE.bazel")


def extract_version(text: str) -> str:
    module_match = re.search(r"(?ms)^module\((.*?)^\)", text)
    if module_match is None:
        raise SystemExit("failed to find module() block in MODULE.bazel")

    version_match = re.search(
        r'(?m)^\s*version\s*=\s*"([^"]+)"\s*,?\s*$',
        module_match.group(1),
    )
    if version_match is None:
        raise SystemExit("failed to find version in MODULE.bazel")

    return version_match.group(1)


def main() -> int:
    current_version = extract_version(MODULE_PATH.read_text())
    base_sha = os.environ.get("BASE_SHA", "")
    previous_version = ""
    version_changed = True

    if base_sha and base_sha != ("0" * 40):
        try:
            previous_text = subprocess.check_output(
                ["git", "show", f"{base_sha}:{MODULE_PATH.as_posix()}"],
                text=True,
            )
        except subprocess.CalledProcessError:
            pass
        else:
            previous_version = extract_version(previous_text)
            version_changed = previous_version != current_version

    print(f"version={current_version}")
    print(f"tag=v{current_version}")
    print(f"previous_version={previous_version}")
    print(f"version_changed={'true' if version_changed else 'false'}")
    print(f"prerelease={'true' if '-' in current_version else 'false'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
