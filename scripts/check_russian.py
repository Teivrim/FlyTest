"""Check that the Russian in the repository is real Russian and not mojibake.

The console in this environment transcodes command strings, so a word typed
into a shell command arrives mangled and a search for it finds nothing. This
script therefore holds its expectations as escape sequences, which are pure
ASCII and survive any transport.

Mojibake has a shape: a real Russian name is Cyrillic letters and a handful
of typographic marks. Damaged text is Latin-1 supplement, box drawing,
U+FFFD, or Cyrillic letters that are not Russian, such as U+0455.
"""

import sys
import unicodedata
from pathlib import Path

# Real Russian words that must be present, as escape sequences so that this
# file is pure ASCII and cannot itself be the thing that is broken.
EXPECTED = {
    "src/editor.rs": [
        "\u0441\u0442\u0440\u0430\u0445",  # strakh, fear
        "\u0440\u0430\u0434\u043e\u0441\u0442\u044c",  # radost, joy
        "\u0441\u043a\u0443\u043a\u0430",  # skuka, boredom
        "\u0434\u043e\u0432\u0435\u0440\u0438\u0435",  # doverie, trust
        "\u0442\u043e\u0441\u043a\u0430",  # toska, longing
        "\u043e\u0441\u0432\u043e\u0438\u043b\u0430",  # osvoila, has learned
    ],
    "src/tfly.rs": [
        "\u0442\u043e\u0440\u043e\u043f\u043b\u0438\u0432\u044b\u0439",  # toroplivyy, hurried
        "\u0440\u043e\u0432\u043d\u044b\u0439",  # rovnyy, steady
        "\u043f\u0435\u0442\u043b\u044f\u044e\u0449\u0438\u0439",  # petlyayushchiy, weaving
    ],
    "editor/app.js": [
        "\u043f\u0440\u043e\u0441\u044b\u043f\u0430\u0435\u0442\u0441\u044f",  # prosypayetsya
        "\u043d\u0435\u0442 \u043f\u0430\u0440\u044b",  # net pary, no partner
    ],
    "editor/index.html": [
        "\u042d\u041c\u041e\u0426\u0418\u0418",  # EMOTsii
        "\u041e\u0421\u0410\u041d\u041a\u0410",  # OSANKA, posture
    ],
    "README.md": [
        "\u043f\u043e\u0445\u043e\u0434\u043a\u0430",  # pohodka, gait
        "\u043f\u0435\u0440\u0441\u043e\u043d\u0430\u0436\u0438",  # personazhi, characters
    ],
}

# Cyrillic letters that exist in the block but are not Russian, which is what
# a mis-decoded string is made of.
NOT_RUSSIAN_CYRILLIC = set(
    "\u0405\u0406\u0408\u0409\u040a\u040b\u040c\u040d\u040e\u040f"
    "\u0455\u0456\u0458\u0459\u045a\u045b\u045c\u045d\u045e\u045f"
)

# Latin-1 and general punctuation that the Russian text legitimately uses.
# Without this the check reports "U+00D7" for the multiplication sign in
# "Учить ходить ×200" as damage, and a false alarm is worse than none.
ALLOWED_NON_ASCII = set(
    "\u00ab\u00bb\u2014\u2013\u2026\u00d7\u00b7\u00b0\u2116\u00ab"
)


def audit(path: Path) -> tuple[int, dict[str, int]]:
    text = path.read_text(encoding="utf-8")
    counts = {
        "cyrillic": 0,
        "replacement": 0,
        "latin1": 0,
        "box": 0,
        "not_russian_cyrillic": 0,
    }
    for ch in text:
        code = ord(ch)
        if code == 0xFFFD:
            counts["replacement"] += 1
        elif 0x00C0 <= code <= 0x00FF:
            if ch not in ALLOWED_NON_ASCII:
                counts["latin1"] += 1
        elif 0x2500 <= code <= 0x257F:
            counts["box"] += 1
        elif 0x0400 <= code <= 0x04FF:
            counts["cyrillic"] += 1
            if ch in NOT_RUSSIAN_CYRILLIC:
                counts["not_russian_cyrillic"] += 1
    return len(text), counts


def main() -> int:
    bad = 0
    for name, words in EXPECTED.items():
        path = Path(name)
        if not path.exists():
            print(f"{name}: MISSING")
            bad += 1
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError as exc:
            print(f"{name}: NOT VALID UTF-8 ({exc})")
            bad += 1
            continue
        # Case-insensitive, because most of these words are also headings and
        # a capitalised "Персонажи" is the same evidence as a lowercase one.
        haystack = text.lower()
        missing = [w for w in words if w.lower() not in haystack]
        _, counts = audit(path)
        problems = []
        if missing:
            problems.append(f"missing {len(missing)}/{len(words)} words")
        if counts["replacement"]:
            problems.append(f"{counts['replacement']} replacement chars")
        if counts["latin1"]:
            problems.append(f"{counts['latin1']} latin1 chars")
        if counts["not_russian_cyrillic"]:
            problems.append(f"{counts['not_russian_cyrillic']} non-Russian Cyrillic")
        status = "OK  " if not problems else "BAD "
        print(
            f"{status}{name:<22} cyrillic={counts['cyrillic']:<7}"
            + ("; ".join(problems) if problems else "clean")
        )
        if problems:
            bad += 1
    print("all clean" if bad == 0 else f"{bad} file(s) still damaged")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
