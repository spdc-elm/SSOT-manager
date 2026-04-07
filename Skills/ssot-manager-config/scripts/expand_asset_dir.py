#!/usr/bin/env python3

from __future__ import annotations

import argparse
import sys
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Expand one asset directory into a flat ssot-manager profile snippet."
    )
    parser.add_argument("--root", required=True, help="Real source root to scan.")
    parser.add_argument(
        "--under",
        required=True,
        help="Directory under --root whose immediate children should become rules.",
    )
    parser.add_argument("--profile", required=True, help="Profile name to emit.")
    parser.add_argument(
        "--to",
        action="append",
        required=True,
        help="Destination path. Repeat to emit multiple destinations per rule.",
    )
    parser.add_argument(
        "--mode",
        choices=["symlink", "copy", "hardlink"],
        default="symlink",
        help="Materialization mode for generated rules.",
    )
    parser.add_argument(
        "--match",
        choices=["directories", "files", "all"],
        default="directories",
        help="Which immediate children to expand.",
    )
    parser.add_argument(
        "--tag",
        action="append",
        default=[],
        help="Tag to attach to every generated rule. Repeatable.",
    )
    parser.add_argument(
        "--require",
        action="append",
        default=[],
        help="Composition requirement to emit on the profile. Repeatable.",
    )
    parser.add_argument(
        "--disabled",
        action="append",
        default=[],
        help="Basename to emit as enabled: false. Repeatable.",
    )
    parser.add_argument(
        "--profile-source-root",
        help="Optional profile-level source_root value to emit in the snippet.",
    )
    parser.add_argument(
        "--include-hidden",
        action="store_true",
        help="Include dot-prefixed children.",
    )
    parser.add_argument(
        "--wrap-profiles",
        action="store_true",
        help="Wrap output under a top-level profiles: mapping.",
    )
    args = parser.parse_args()

    root = Path(args.root).expanduser().resolve()
    scan_dir = root / args.under
    if not root.exists() or not root.is_dir():
        raise SystemExit(f"Error: root does not exist or is not a directory: {root}")
    if not scan_dir.exists() or not scan_dir.is_dir():
        raise SystemExit(
            f"Error: scan directory does not exist or is not a directory: {scan_dir}"
        )

    children = []
    for child in sorted(scan_dir.iterdir(), key=lambda path: path.name):
        if not args.include_hidden and child.name.startswith("."):
            continue
        if args.match == "directories" and not child.is_dir():
            continue
        if args.match == "files" and not child.is_file():
            continue
        if args.match == "all" and not (child.is_file() or child.is_dir()):
            continue
        children.append(child.name)

    lines: list[str] = []
    if args.wrap_profiles:
        lines.append("profiles:")
        profile_indent = "  "
        field_indent = "    "
        list_indent = "      "
        item_indent = "        "
    else:
        profile_indent = ""
        field_indent = "  "
        list_indent = "    "
        item_indent = "      "

    lines.append(f"{profile_indent}{args.profile}:")
    if args.profile_source_root:
        lines.append(f"{field_indent}source_root: {args.profile_source_root}")
    if args.require:
        lines.append(f"{field_indent}requires:")
        for requirement in args.require:
            lines.append(f"{list_indent}- {requirement}")
    lines.append(f"{field_indent}rules:")

    disabled = set(args.disabled)
    under = args.under.rstrip("/")
    for name in children:
        lines.append(f"{list_indent}- select: {under}/{name}")
        lines.append(f"{item_indent}to:")
        for destination in args.to:
            lines.append(f"{item_indent}- {destination}")
        lines.append(f"{item_indent}mode: {args.mode}")
        if name in disabled:
            lines.append(f"{item_indent}enabled: false")
        if args.tag:
            lines.append(f"{item_indent}tags:")
            for tag in args.tag:
                lines.append(f"{item_indent}- {tag}")

    sys.stdout.write("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
