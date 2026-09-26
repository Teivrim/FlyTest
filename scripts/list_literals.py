"""List every non-ASCII string literal, so the repaired text can be read.

Repairing by anchor can put a plausible-looking string in the wrong place: a
window of ASCII that happens to match will happily copy a neighbouring table.
The only defence is to read what actually ended up in the file.
"""

import re
import sys
from pathlib import Path

LITERAL = re.compile(r'"((?:[^"\\]|\\.)*)"')


def main() -> None:
    limit = int(sys.argv[1])
    out: list[str] = []
    for name in sys.argv[2:]:
        out.append(f"########## {name}")
        for number, line in enumerate(
            Path(name).read_text(encoding="utf-8").splitlines(), start=1
        ):
            if not any(ord(c) > 127 for c in line):
                continue
            for literal in LITERAL.findall(line):
                if not any(ord(c) > 127 for c in literal):
                    continue
                text = literal if len(literal) <= limit else literal[:limit] + " ..."
                out.append(f"{number:5}  {text}")
        out.append("")
    Path("runtime-output/literals.txt").write_text("\n".join(out), encoding="utf-8")
    print(f"wrote {len(out)} lines")


if __name__ == "__main__":
    main()
