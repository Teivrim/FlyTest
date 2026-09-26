"""Show the regions around text the repair could not account for."""

import subprocess
import sys
from pathlib import Path

NOT_RUSSIAN = set(
    "\u0405\u0406\u0408\u0409\u040a\u040b\u040c\u040d\u040e\u040f"
    "\u0455\u0456\u0458\u0459\u045a\u045b\u045c\u045d\u045e\u045f"
)
# The Russian text uses these legitimately, and the "×" in "Учить ходить ×200"
# would otherwise read as damage.
ALLOWED = set("\u00ab\u00bb\u2014\u2013\u2026\u00d7\u00b7\u00b0\u2116")


def damaged(text: str) -> bool:
    if "\ufffd" in text:
        return True
    if any(0x00C0 <= ord(c) <= 0x00FF and c not in ALLOWED for c in text):
        return True
    return any(c in NOT_RUSSIAN for c in text)


def main() -> None:
    width = int(sys.argv[1])
    out: list[str] = []
    for name in sys.argv[2:]:
        lines = Path(name).read_text(encoding="utf-8").splitlines()
        out.append(f"########## {name}")
        shown: set[int] = set()
        for i, line in enumerate(lines):
            if not damaged(line):
                continue
            for j in range(max(0, i - width), min(len(lines), i + width + 1)):
                if j in shown:
                    continue
                shown.add(j)
                mark = ">>" if j == i else "  "
                text = lines[j]
                if len(text) > 200:
                    text = text[:200] + f"  ...[+{len(lines[j]) - 200} chars]"
                out.append(f"{mark} {j + 1:5}  {text}")
            out.append("")
    Path("runtime-output/orphans.txt").write_text("\n".join(out), encoding="utf-8")
    print(f"wrote {len(out)} lines")


if __name__ == "__main__":
    main()
