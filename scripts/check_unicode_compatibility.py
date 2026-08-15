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
    "Scripts.txt",
    "CaseFolding.txt",
    "SpecialCasing.txt",
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
LETTER_CATEGORIES = {"Lu", "Ll", "Lt", "Lm", "Lo"}


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


def special_casing(text):
    rules, languages, contexts, problems = {}, set(), set(), []
    for number, line in enumerate(text.splitlines(), 1):
        content = line.partition("#")[0].strip()
        if not content:
            continue
        fields = [part.strip() for part in content.split(";")]
        if len(fields) < 5:
            problems.append(f"line {number}: fewer than five fields")
            continue

        try:
            source = tuple(int(part, 16) for part in fields[0].split())
            mappings = tuple(
                tuple(int(part, 16) for part in field.split())
                for field in fields[1:4]
            )
        except ValueError:
            problems.append(f"line {number}: invalid code point")
            continue
        if len(source) != 1:
            problems.append(f"line {number}: source does not contain one code point")
            continue

        conditions = []
        valid = True
        for condition in fields[4].split():
            negated = condition.casefold().startswith("not_")
            context = condition[4:] if negated else condition
            if context[:1].isupper() and all(
                char.isascii() and (char.isalnum() or char == "_")
                for char in context
            ):
                context = context.casefold()
                contexts.add(context)
                conditions.append(("context", f"not_{context}" if negated else context))
                continue

            subtags = condition.replace("_", "-").split("-")
            primary = subtags[0]
            if (
                not 2 <= len(primary) <= 8
                or not primary.isascii()
                or not primary.isalpha()
                or any(
                    not 1 <= len(subtag) <= 8
                    or not subtag.isascii()
                    or not subtag.isalnum()
                    for subtag in subtags[1:]
                )
            ):
                problems.append(f"line {number}: unrecognized condition {condition!r}")
                valid = False
                continue
            language = "-".join(subtag.casefold() for subtag in subtags)
            languages.add(language)
            conditions.append(("language", language))

        if valid:
            cp = source[0]
            rule = (*mappings, tuple(sorted(conditions)))
            rules.setdefault(cp, []).append(rule)

    rules = {cp: tuple(sorted(cp_rules)) for cp, cp_rules in rules.items()}
    return rules, languages, contexts, problems


class Unicode:
    def __init__(self, version):
        files = {name: download(version, name) for name in FILES}
        self.chars = characters(files["UnicodeData.txt"])
        self.assigned = {cp for cp in self.chars if not 0xD800 <= cp <= 0xDFFF}
        self.space = property_set(files["PropList.txt"], "White_Space")
        self.soft = property_set(files["PropList.txt"], "Soft_Dotted")
        greek = property_set(files["Scripts.txt"], "Greek")
        self.greek_letter = {
            cp
            for cp in greek
            if cp in self.chars and self.chars[cp][1] in LETTER_CATEGORIES
        }
        self.fold, self.fold_shape_problems = folding(files["CaseFolding.txt"])
        (
            self.special_casing,
            self.special_casing_languages,
            self.special_casing_contexts,
            self.special_casing_parse_problems,
        ) = special_casing(files["SpecialCasing.txt"])
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


def special_casing_rules(value):
    if not value:
        return "none"
    result = []
    for lower, title, upper, conditions in value:
        conditions = " ".join(f"{kind}:{name}" for kind, name in conditions)
        result.append(
            f"lower={codepoints(lower) or 'empty'}, "
            f"title={codepoints(title) or 'empty'}, "
            f"upper={codepoints(upper) or 'empty'}, "
            f"conditions={conditions or 'none'}"
        )
    return " | ".join(result)


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
    special_casing_parse_problems = chain(
        (f"old: {problem}" for problem in old.special_casing_parse_problems),
        (f"new: {problem}" for problem in new.special_casing_parse_problems),
        (
            f"old: U+{cp:04X} is unassigned"
            for cp in old.special_casing.keys() - old.assigned
        ),
        (
            f"new: U+{cp:04X} is unassigned"
            for cp in new.special_casing.keys() - new.assigned
        ),
    )
    special_casing_language_changes = (
        f"{language}: {'removed' if language in old.special_casing_languages else 'added'}"
        for language in sorted(
            old.special_casing_languages ^ new.special_casing_languages
        )
    )
    special_casing_context_changes = (
        f"{context}: {'removed' if context in old.special_casing_contexts else 'added'}"
        for context in sorted(old.special_casing_contexts ^ new.special_casing_contexts)
    )
    special_casing_changes = changes(
        old.assigned,
        lambda cp: old.special_casing.get(cp, ()),
        lambda cp: new.special_casing.get(cp, ()),
        formatter=special_casing_rules,
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
            "is_greek_letter membership changed for old characters",
            (
                f"U+{cp:04X}"
                for cp in sorted((old.greek_letter ^ new.greek_letter) & old.assigned)
            ),
        ),
        (
            "Case folding data no longer has one mapping per assigned character",
            fold_shape_problems,
        ),
        ("Default full case folding changed for old characters", fold_changes),
        ("SpecialCasing.txt condition format changed", special_casing_parse_problems),
        ("SpecialCasing.txt language set changed", special_casing_language_changes),
        ("SpecialCasing.txt casing context set changed", special_casing_context_changes),
        ("Special casing changed for old characters", special_casing_changes),
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
