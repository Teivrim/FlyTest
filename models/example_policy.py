"""Reference JSONL brain adapter.

The runtime currently writes observations/actions as JSONL. This example shows
where a PyTorch/ONNX/custom model belongs without requiring a framework.
"""

from __future__ import annotations

import json
import sys
from typing import Any, TextIO


def step(observation: dict[str, Any]) -> dict[str, Any]:
    odor = float(observation.get("odor", 0.0))
    light = float(observation.get("light", 0.0))
    touch = float(observation.get("touch", 0.0))
    return {
        "tick": int(observation.get("tick", 0)),
        "throttle": max(0.0, min(1.0, 0.25 + light * 0.4)),
        "turn": max(-1.0, min(1.0, (odor - 0.5) * 1.5 - touch)),
        "vertical": 0.0,
        "wingbeat_hz": 95.0 + 30.0 * light,
        "hormone_pulses": [],
    }


def main() -> None:
    source: TextIO = sys.stdin if not sys.argv[1:] else open(sys.argv[1], encoding="utf-8")
    try:
        for line in source:
            if line.strip():
                observation = json.loads(line)
                json.dump(step(observation), sys.stdout)
                sys.stdout.write("\n")
                sys.stdout.flush()
    finally:
        if source is not sys.stdin:
            source.close()


if __name__ == "__main__":
    main()
