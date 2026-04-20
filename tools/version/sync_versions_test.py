from __future__ import annotations

import sys
from pathlib import Path

from tools.version.sync_versions import find_workspace_root, main


def test_root() -> Path:
    return find_workspace_root(Path(__file__))


if __name__ == "__main__":
    sys.exit(main(["--check", "--root", str(test_root())]))
