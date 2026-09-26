"""Copy named blocks out of a historical revision, for the repair to graft.

The anchor-based repair put the wrong string in the wrong place more than
once, because an ASCII window can match a neighbouring table. So instead of
guessing, the correct text is read out of the last clean revision, block by
block, and written where it belongs by hand.
"""

import re
import subprocess
import sys
from pathlib import Path


def block(lines: list[str], start: int, size: int) -> list[str]:
    return lines[start : start + size]


def main() -> None:
    commit = sys.argv[1]
    source = sys.argv[2]
    out: list[str] = []
    for spec in sys.argv[3:]:
        marker, _, size = spec.partition(":")
        blob = subprocess.run(
            ["git", "show", f"{commit}:{source}"], capture_output=True, check=True
        )
        lines = blob.stdout.decode("utf-8").splitlines()
        hit = next(
            (i for i, line in enumerate(lines) if re.search(marker, line)),
            None,
        )
        if hit is None:
            out.append(f"##### {marker}: NOT FOUND")
            out.append("")
            continue
        out.append(f"##### {marker} at {hit + 1}")
        out.extend(block(lines, hit, int(size)))
        out.append("")
    Path("runtime-output/blocks.txt").write_text("\n".join(out), encoding="utf-8")
    print(f"wrote {len(out)} lines")


if __name__ == "__main__":
    main()
