#!/usr/bin/env python3
"""Generate a Shields endpoint badge from `cargo crap --format json` output."""

import argparse
import json
from pathlib import Path


def badge_color(crappy_count: int, total_count: int) -> str:
    if crappy_count == 0:
        return "brightgreen"

    ratio = crappy_count / total_count if total_count else 1.0
    if ratio <= 0.05:
        return "yellowgreen"
    if ratio <= 0.15:
        return "yellow"
    if ratio <= 0.25:
        return "orange"
    return "red"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("report", type=Path, help="cargo-crap JSON report path")
    parser.add_argument("badge", type=Path, help="badge JSON output path")
    parser.add_argument(
        "--threshold",
        type=float,
        default=30.0,
        help="CRAP threshold used for the report",
    )
    args = parser.parse_args()

    report = json.loads(args.report.read_text(encoding="utf-8"))
    entries = report.get("entries", [])
    total = len(entries)
    crappy = sum(1 for entry in entries if entry.get("crap", 0.0) > args.threshold)

    badge = {
        "schemaVersion": 1,
        "label": "cargo crap",
        "message": f"{crappy}/{total} > {args.threshold:g}",
        "color": badge_color(crappy, total),
    }

    args.badge.parent.mkdir(parents=True, exist_ok=True)
    args.badge.write_text(json.dumps(badge, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
