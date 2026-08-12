#!/usr/bin/env python3
"""Check whether a chain of Unicode versions is compatible for normalized-path."""

import sys
from itertools import chain
from pathlib import Path
from urllib.request import urlopen

FILES = [
    "UnicodeData.txt",
    "DerivedNormalizationProps.txt",
    "PropList.txt",
    "CaseFolding.txt",
]
DEFAULT_VERSIONS = [
    "4.1.0",
    "5.0.0",
    "5.1.0",
    "5.2.0",
    "6.0.0",
    "6.1.0",
    "6.2.0",
    "6.3.0",
    "7.0.0",
    "8.0.0",
    "9.0.0",
    "10.0.0",
    "11.0.0",
    "12.0.0",
    "12.1.0",
    "13.0.0",
    "14.0.0",
    "15.0.0",
    "15.1.0",
    "16.0.0",
    "17.0.0",
]
URL = "https://www.unicode.org/Public/{}/ucd/{}"
CACHE = Path(__file__).parent / "cache"


def download(version, name):
    path = CACHE / version / name
    if path.exists():
        return path.read_text(encoding="utf-8")
    with urlopen(URL.format(version, name)) as response:
        text = response.read().decode()
    CACHE.mkdir(exist_ok=True)
    path.parent.mkdir(exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return text


def decomposition(field):
    parts = field.split()
    tag = parts.pop(0) if parts and parts[0].startswith("<") else ""
    return tag, tuple(int(part, 16) for part in parts)


def characters(text):
    result, first = {}, None
    for line in text.splitlines():
        fields = line.split(";")
        cp = int(fields[0], 16)
        value = fields[1], fields[2], int(fields[3]), decomposition(fields[5])
        if fields[1].endswith(", First>"):
            first = cp, value
        elif fields[1].endswith(", Last>"):
            start, value = first
            result.update(dict.fromkeys(range(start, cp + 1), value))
            first = None
        else:
            result[cp] = value
    return result


def property_set(text, name):
    result = set()
    for line in text.splitlines():
        fields = line.partition("#")[0].split(";")
        if len(fields) < 2 or fields[1].strip() != name:
            continue
        bounds = [int(part, 16) for part in fields[0].strip().split("..")]
        result.update(range(bounds[0], bounds[-1] + 1))
    return result


def folding(text):
    result, problems = {}, []
    for number, line in enumerate(text.splitlines(), 1):
        fields = [part.strip() for part in line.partition("#")[0].split(";")]
        if len(fields) < 3 or not fields[0]:
            continue
        source = fields[0].split()
        if len(source) != 1:
            problems.append(f"line {number}: multi-character source")
        elif fields[1] in {"C", "F"}:
            cp = int(source[0], 16)
            if cp in result:
                problems.append(f"line {number}: duplicate U+{cp:04X}")
            result[cp] = tuple(int(part, 16) for part in fields[2].split())
    return result, problems


class Unicode:
    def __init__(self, version):
        files = {name: download(version, name) for name in FILES}
        self.chars = characters(files["UnicodeData.txt"])
        self.assigned = {cp for cp in self.chars if not 0xD800 <= cp <= 0xDFFF}
        self.space = property_set(files["PropList.txt"], "White_Space")
        self.soft = property_set(files["PropList.txt"], "Soft_Dotted")
        self.fold, self.fold_shape_problems = folding(files["CaseFolding.txt"])
        excluded = property_set(
            files["DerivedNormalizationProps.txt"], "Full_Composition_Exclusion"
        )
        self.compositions = {
            mapping: cp
            for cp, (_, _, _, (tag, mapping)) in self.chars.items()
            if not tag and len(mapping) == 2 and cp not in excluded
        }


def codepoints(value):
    if value is None:
        return "none"
    if isinstance(value, int):
        value = (value,)
    return " ".join(f"U+{cp:04X}" for cp in value)


def changes(points, old, new, label=lambda cp: f"U+{cp:04X}", formatter=repr):
    return [
        f"{label(point)}: {formatter(old(point))} -> {formatter(new(point))}"
        for point in sorted(points)
        if old(point) != new(point)
    ]


def compare(old_version, old, new_version, new):
    common, missing = old.assigned & new.assigned, old.assigned - new.assigned

    identity = [f"U+{cp:04X} became unassigned" for cp in sorted(missing)]
    identity += changes(common, lambda cp: old.chars[cp][0], lambda cp: new.chars[cp][0])
    ccc = changes(common, lambda cp: old.chars[cp][2], lambda cp: new.chars[cp][2])
    pairs = {
        pair
        for pair in old.compositions.keys() | new.compositions.keys()
        if set(pair) <= old.assigned
    }
    normalization = identity + ccc
    normalization += changes(common, lambda cp: old.chars[cp][3], lambda cp: new.chars[cp][3])
    normalization += changes(
        pairs,
        old.compositions.get,
        new.compositions.get,
        lambda pair: " + ".join(f"U+{cp:04X}" for cp in pair),
        codepoints,
    )

    old_cc = {cp for cp in old.assigned if old.chars[cp][1] == "Cc"}
    new_cc = {cp for cp in new.assigned if new.chars[cp][1] == "Cc"}
    fold_shape_problems = chain(
        (f"old: {problem}" for problem in old.fold_shape_problems),
        (f"new: {problem}" for problem in new.fold_shape_problems),
        (f"old: U+{cp:04X} is unassigned" for cp in old.fold.keys() - old.assigned),
        (f"new: U+{cp:04X} is unassigned" for cp in new.fold.keys() - new.assigned),
    )
    fold_changes = changes(
        old.assigned,
        lambda cp: old.fold.get(cp, (cp,)),
        lambda cp: new.fold.get(cp, (cp,)),
        formatter=codepoints,
    )
    checks = [
        ("Character identity or assignment changed", identity),
        ("Canonical_Combining_Class changed", ccc),
        (
            "CCC=230 membership changed for old characters",
            changes(
                common,
                lambda cp: old.chars[cp][2] == 230,
                lambda cp: new.chars[cp][2] == 230,
            ),
        ),
        ("Normalization behavior changed", normalization),
        (
            "General_Category=Control (Cc) membership changed",
            (f"U+{cp:04X}" for cp in sorted(old_cc ^ new_cc)),
        ),
        (
            "White_Space membership changed for old characters",
            (f"U+{cp:04X}" for cp in sorted((old.space ^ new.space) & old.assigned)),
        ),
        (
            "Soft_Dotted membership changed for old characters",
            (f"U+{cp:04X}" for cp in sorted((old.soft ^ new.soft) & old.assigned)),
        ),
        (
            "Case folding data no longer has one mapping per assigned character",
            fold_shape_problems,
        ),
        ("Default full case folding changed for old characters", fold_changes),
        (
            "Case folding no longer preserves Soft_Dotted",
            (
                f"U+{cp:04X} -> {codepoints(new.fold.get(cp, (cp,)))}"
                for cp in sorted(new.soft)
                if len(new.fold.get(cp, (cp,))) != 1
                or new.fold.get(cp, (cp,))[0] not in new.soft
            ),
        ),
    ]
    checks = [(name, list(problems)) for name, problems in checks]
    failures = [(name, problems) for name, problems in checks if problems]
    passed = len(checks) - len(failures)

    if not failures:
        print(f"Unicode {old_version} -> {new_version}: COMPATIBLE ({passed} checks passed)")
        return True

    print(f"Unicode {old_version} -> {new_version}: INCOMPATIBLE")
    for name, problems in failures:
        print(f"\n{name} ({len(problems)}):")
        for problem in problems[:10]:
            print(f"  {problem}")
        if len(problems) > 10:
            print(f"  ... and {len(problems) - 10} more")
    print(f"\n{passed} checks passed, {len(failures)} failed")
    return False


def main(versions):
    if not versions:
        versions = DEFAULT_VERSIONS
    if len(versions) < 2:
        sys.exit(f"usage: {sys.argv[0]} [VERSION VERSION ...]")

    results = []
    old_version, old = versions[0], Unicode(versions[0])
    for new_version in versions[1:]:
        new = Unicode(new_version)
        if results:
            print()
        results.append(compare(old_version, old, new_version, new))
        old_version, old = new_version, new

    if len(results) > 1:
        compatible = sum(results)
        incompatible = len(results) - compatible
        print(f"\nSummary: {compatible}/{len(results)} compatible, {incompatible} incompatible")
    return 0 if all(results) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
