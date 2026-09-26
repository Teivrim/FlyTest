"""Fix separators that a mis-decoding write turned into mixed scripts.

`Р“вЂ”` and `Р’В·` are what `·` and `×` become when UTF-8 is read back
through an ANSI codepage: valid Russian letters with a stray curly quote
glued between them. The bytes are well-formed and every letter is one the
Russian alphabet has, so neither the compiler nor an encoding check can see
it. Only a rule about which characters are allowed next to each other finds
it.

Replaced by line number, because the damaged text cannot be matched as a
search string.
"""

from pathlib import Path

# 1-based line number -> (broken, whole). Written with chr() because a
# backslash escape in this file would itself have to survive being written as
# text, and getting that wrong fails silently.
MANGLED_X = "".join(chr(c) for c in (0x420, 0x201C, 0x432, 0x402, 0x201D))
MANGLED_DOT = " " + "".join(chr(c) for c in (0x420, 0x2019, 0x412, 0xB7)) + " "
FIX = {
    # "1.00x", a multiplication sign after a speed.
    1161: (MANGLED_X, chr(0xD7)),
    # " - ", a middot between a label and a raw key.
    1162: (MANGLED_DOT, " " + chr(0xB7) + " "),
    1179: (MANGLED_DOT, " " + chr(0xB7) + " "),
}


def main() -> None:
    path = Path("editor/app.js")
    lines = path.read_text(encoding="utf-8").splitlines()
    for number, (broken, whole) in FIX.items():
        line = lines[number - 1]
        if broken not in line:
            raise SystemExit(f"line {number} does not contain the damage")
        lines[number - 1] = line.replace(broken, whole)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"fixed {len(FIX)} lines")


if __name__ == "__main__":
    main()
