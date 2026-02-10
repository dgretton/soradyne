#!/usr/bin/env python3
"""Structural diff between two inventory txt files.

Matches entries by description (the text between "} " and " -> "),
then reports added, removed, and changed entries. Ignores line order
and comment lines (starting with #).

Usage:
    python diff_invs.py old.txt new.txt
"""

import json
import re
import sys
from dataclasses import dataclass

ENTRY_RE = re.compile(r"^(\{.*?\})\s+(.*?)\s+->\s+(.*)$")


@dataclass
class Entry:
    category: str
    tags: list[str]
    description: str
    location: str
    line: str  # original line for display

    @property
    def key(self) -> str:
        return self.description.strip().lower()


def parse_file(path: str) -> dict[str, Entry]:
    entries: dict[str, Entry] = {}
    with open(path) as f:
        for lineno, raw in enumerate(f, 1):
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            m = ENTRY_RE.match(line)
            if not m:
                print(f"  warning: skipping unparseable line {lineno}: {line[:80]}", file=sys.stderr)
                continue
            meta_str, desc, loc = m.group(1), m.group(2), m.group(3)
            try:
                meta = json.loads(meta_str)
            except json.JSONDecodeError:
                print(f"  warning: bad JSON on line {lineno}: {meta_str[:80]}", file=sys.stderr)
                continue
            entry = Entry(
                category=meta.get("category", ""),
                tags=sorted(meta.get("tags", [])),
                description=desc,
                location=loc,
                line=line,
            )
            key = entry.key
            if key in entries:
                print(f"  warning: duplicate description on line {lineno}: {desc[:60]}", file=sys.stderr)
            entries[key] = entry
    return entries


def diff_fields(old: Entry, new: Entry) -> list[str]:
    changes = []
    if old.category != new.category:
        changes.append(f"  category: {old.category!r} -> {new.category!r}")
    if old.location != new.location:
        changes.append(f"  location: {old.location!r} -> {new.location!r}")
    if old.tags != new.tags:
        removed_tags = set(old.tags) - set(new.tags)
        added_tags = set(new.tags) - set(old.tags)
        parts = []
        if removed_tags:
            parts.append(f"-[{', '.join(sorted(removed_tags))}]")
        if added_tags:
            parts.append(f"+[{', '.join(sorted(added_tags))}]")
        changes.append(f"  tags: {' '.join(parts)}")
    if old.description != new.description:
        changes.append(f"  description: {old.description!r} -> {new.description!r}")
    return changes


def main():
    if len(sys.argv) != 3:
        print(f"usage: {sys.argv[0]} <old.txt> <new.txt>", file=sys.stderr)
        sys.exit(1)

    old_path, new_path = sys.argv[1], sys.argv[2]
    old = parse_file(old_path)
    new = parse_file(new_path)

    old_keys = set(old)
    new_keys = set(new)

    removed = sorted(old_keys - new_keys, key=lambda k: old[k].description)
    added = sorted(new_keys - old_keys, key=lambda k: new[k].description)
    common = sorted(old_keys & new_keys, key=lambda k: old[k].description)

    changed = []
    for key in common:
        fields = diff_fields(old[key], new[key])
        if fields:
            changed.append((old[key].description, fields))

    # Print report
    if not removed and not added and not changed:
        print("No differences found.")
        return

    if removed:
        print(f"REMOVED ({len(removed)}):")
        for key in removed:
            e = old[key]
            print(f"  - {e.description}  [{e.category}] -> {e.location}")
        print()

    if added:
        print(f"ADDED ({len(added)}):")
        for key in added:
            e = new[key]
            print(f"  + {e.description}  [{e.category}] -> {e.location}")
        print()

    if changed:
        print(f"CHANGED ({len(changed)}):")
        for desc, fields in changed:
            print(f"  ~ {desc}")
            for f in fields:
                print(f"    {f}")
        print()

    print(f"Summary: {len(removed)} removed, {len(added)} added, {len(changed)} changed, {len(common) - len(changed)} unchanged")


if __name__ == "__main__":
    main()
