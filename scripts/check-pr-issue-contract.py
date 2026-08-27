#!/usr/bin/env python3
from __future__ import annotations

import os
import re
import sys

ENV_BODY = "SEYAL_PR_BODY"

ISSUE_SECTION = re.compile(
    r"(?ims)^##[ \t]+Issue[ \t]*\r?$\n(?P<body>.*?)(?=^##[ \t]+|\Z)"
)
OWNING_ISSUE = re.compile(r"(?im)^[ \t]*Owning Issue:[ \t]*#(\d+)[ \t]*\r?$")
RELATION = re.compile(
    r"(?im)^[ \t]*(?:-[ \t]*)?(Closes|Fixes|Resolves|Refs|Part of)[ \t]+#(\d+)[ \t]*\r?$"
)
CLOSING_KEYWORD = re.compile(
    r"(?i)\b(?:close|closes|closed|fix|fixes|fixed|resolve|resolves|resolved)[ \t]+#(\d+)\b"
)
FENCED_CODE = re.compile(r"```.*?```", re.DOTALL)
INLINE_CODE = re.compile(r"`[^`\n]*`")


def fail(message: str) -> None:
    raise SystemExit(f"[seyal PR issue contract] ERROR: {message}")


def strip_code(markdown: str) -> str:
    return INLINE_CODE.sub("", FENCED_CODE.sub("", markdown))


def validate(body: str) -> None:
    if not body.strip():
        fail("pull request body is empty")

    section_match = ISSUE_SECTION.search(body)
    if section_match is None:
        fail("missing required '## Issue' section")

    issue_section = section_match.group("body")
    owners = OWNING_ISSUE.findall(issue_section)
    if len(owners) != 1:
        fail(f"expected exactly one 'Owning Issue: #N' entry in ## Issue; found {len(owners)}")
    owner = owners[0]

    relationships = RELATION.findall(issue_section)
    if len(relationships) != 1:
        fail(f"expected exactly one closing/non-closing Issue relationship in ## Issue; found {len(relationships)}")

    relationship, target = relationships[0]
    if target != owner:
        fail(f"Issue relationship targets #{target}, but owning Issue is #{owner}")

    plain_body = strip_code(body)
    closing_targets = CLOSING_KEYWORD.findall(plain_body)
    is_closing_relationship = relationship.lower() in {"closes", "fixes", "resolves"}

    if is_closing_relationship:
        if closing_targets != [owner]:
            rendered = ", ".join(f"#{number}" for number in closing_targets) or "none"
            fail(
                "a closing PR must contain exactly one active GitHub closing keyword, "
                f"targeting its owning Issue #{owner}; found {rendered}"
            )
    elif closing_targets:
        rendered = ", ".join(f"#{number}" for number in closing_targets)
        fail(
            f"non-closing relationship '{relationship} #{owner}' cannot coexist with active "
            f"GitHub closing keyword(s): {rendered}"
        )

    print(f"[seyal PR issue contract] owning Issue #{owner}; relationship: {relationship} #{target}")


def main() -> None:
    body = os.environ.get(ENV_BODY)
    if body is None:
        fail(f"{ENV_BODY} is not set")
    validate(body)


if __name__ == "__main__":
    main()
