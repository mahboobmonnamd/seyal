#!/usr/bin/env python3
"""Export/import/reconcile GitHub issues for Seyal legacy repositories.

The migration is intentionally conservative:
- exports every GitHub Issue with state=all (open and closed); PR objects are excluded;
- independently verifies the exported Issue count through GitHub search;
- preserves source identity and original status metadata;
- preserves every issue comment with a stable source-comment marker;
- keeps legacy decisions historical unless current Seyal authority accepts them;
- never deletes or mutates source repositories;
- is idempotent and repairs missing migrated comments on re-run.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

API = "https://api.github.com"
RILL_REPO = "mahboobmonnamd/RILL"
RILL_EXPECTED_INVENTORY_ROWS = 216


def token() -> str:
    value = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if not value:
        raise SystemExit("GH_TOKEN or GITHUB_TOKEN is required")
    return value


def request(method: str, path: str, data=None):
    body = None if data is None else json.dumps(data).encode()
    req = urllib.request.Request(API + path, data=body, method=method)
    req.add_header("Accept", "application/vnd.github+json")
    req.add_header("Authorization", f"Bearer {token()}")
    req.add_header("X-GitHub-Api-Version", "2022-11-28")
    if body is not None:
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req) as resp:
            payload = resp.read()
            return json.loads(payload) if payload else None, resp.headers
    except urllib.error.HTTPError as exc:
        details = exc.read().decode(errors="replace")
        raise RuntimeError(f"GitHub {method} {path} failed: {exc.code} {details}") from exc


def paged(path: str):
    page = 1
    out = []
    while True:
        sep = "&" if "?" in path else "?"
        payload, _ = request("GET", f"{path}{sep}per_page=100&page={page}")
        if not payload:
            return out
        out.extend(payload)
        if len(payload) < 100:
            return out
        page += 1


def independent_issue_count(repo: str) -> int:
    q = urllib.parse.quote(f"repo:{repo} is:issue")
    payload, _ = request("GET", f"/search/issues?q={q}&per_page=1")
    return int((payload or {}).get("total_count", 0))


def export_repo(repo: str, out_path: Path):
    # REST /issues includes PRs. state=all includes both open and closed Issues.
    items = paged(f"/repos/{repo}/issues?state=all&direction=asc&sort=created")
    issues = []
    for item in items:
        if "pull_request" in item:
            continue
        number = item["number"]
        comments = paged(f"/repos/{repo}/issues/{number}/comments")
        issues.append(
            {
                "source_repo": repo,
                "source_number": number,
                "source_url": item["html_url"],
                "author": (item.get("user") or {}).get("login"),
                "title": item["title"],
                "body": item.get("body") or "",
                "state": item["state"],
                "state_reason": item.get("state_reason"),
                "labels": [x["name"] for x in item.get("labels", [])],
                "assignees": [x["login"] for x in item.get("assignees", [])],
                "milestone": (item.get("milestone") or {}).get("title"),
                "created_at": item.get("created_at"),
                "updated_at": item.get("updated_at"),
                "closed_at": item.get("closed_at"),
                "comments": [
                    {
                        "id": c.get("id"),
                        "author": (c.get("user") or {}).get("login"),
                        "created_at": c.get("created_at"),
                        "updated_at": c.get("updated_at"),
                        "body": c.get("body") or "",
                    }
                    for c in comments
                ],
                "migration_classification": "historical-unreviewed",
            }
        )

    independent_count = independent_issue_count(repo)
    if independent_count != len(issues):
        raise SystemExit(
            f"issue export count mismatch for {repo}: REST export={len(issues)} search={independent_count}"
        )

    inventory_count = None
    if repo == RILL_REPO:
        inventory_count = sum(1 for i in issues if "inventory" in i.get("labels", []))
        if inventory_count != RILL_EXPECTED_INVENTORY_ROWS:
            raise SystemExit(
                f"RILL inventory mismatch: expected={RILL_EXPECTED_INVENTORY_ROWS} exported={inventory_count}"
            )

    export = {
        "schema": 2,
        "source_repo": repo,
        "issue_count": len(issues),
        "independent_issue_count": independent_count,
        "rill_inventory_count": inventory_count,
        "issues": issues,
    }
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(export, indent=2, ensure_ascii=False) + "\n")
    print(f"exported {len(issues)} issues from {repo} -> {out_path}")


def load_json(path: Path):
    return json.loads(path.read_text())


def load_map(path: Path):
    if not path.exists():
        return {"schema": 2, "mappings": {}}
    return load_json(path)


def save_map(path: Path, mapping):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(mapping, indent=2, ensure_ascii=False) + "\n")


def source_key(issue):
    return f"{issue['source_repo']}#{issue['source_number']}"


def marker(issue):
    return f"Legacy-Source: {source_key(issue)}"


def comment_marker(comment):
    return f"Legacy-Comment-Id: {comment.get('id')}"


def migrated_body(issue):
    labels = ", ".join(issue.get("labels") or []) or "(none)"
    assignees = ", ".join(issue.get("assignees") or []) or "(none)"
    header = f"""> [!IMPORTANT]\n> Migrated historical issue. Preservation does **not** make legacy architecture current Seyal authority. A legacy `closed/completed` state also does **not** mean this capability is implemented in current Seyal. Current Seyal ADRs/specs/constitution win on conflict.\n\n<!-- {marker(issue)} -->\n\n## Legacy source\n\n- Repository: `{issue['source_repo']}`\n- Issue: #{issue['source_number']}\n- URL: {issue['source_url']}\n- Original author: `{issue.get('author')}`\n- Original state: `{issue['state']}`\n- State reason: `{issue.get('state_reason')}`\n- Created: `{issue.get('created_at')}`\n- Updated: `{issue.get('updated_at')}`\n- Closed: `{issue.get('closed_at')}`\n- Original labels: {labels}\n- Original assignees: {assignees}\n- Original milestone: {issue.get('milestone') or '(none)'}\n- Migration classification: `{issue.get('migration_classification', 'historical-unreviewed')}`\n\n## Original issue body\n\n"""
    return header + (issue.get("body") or "(empty)")


def migrated_comment(comment):
    return (
        f"<!-- {comment_marker(comment)} -->\n"
        f"_Migrated comment by @{comment.get('author') or 'unknown'}; original created "
        f"`{comment.get('created_at')}`; original updated `{comment.get('updated_at')}`._\n\n"
        f"{comment.get('body') or '(empty)'}"
    )


def find_existing(dest: str, issue):
    q = urllib.parse.quote(f'"{marker(issue)}" repo:{dest} is:issue')
    payload, _ = request("GET", f"/search/issues?q={q}&per_page=5")
    items = payload.get("items", []) if payload else []
    return items[0] if items else None


def ensure_comments(dest: str, dest_number: int, issue):
    existing = paged(f"/repos/{dest}/issues/{dest_number}/comments")
    bodies = [c.get("body") or "" for c in existing]
    for c in issue.get("comments", []):
        cm = comment_marker(c)
        if any(cm in body for body in bodies):
            continue
        body = migrated_comment(c)
        request("POST", f"/repos/{dest}/issues/{dest_number}/comments", {"body": body})
        bodies.append(body)


def ensure_state(dest: str, dest_issue, issue):
    desired = issue["state"]
    if dest_issue.get("state") == desired:
        return
    request("PATCH", f"/repos/{dest}/issues/{dest_issue['number']}", {"state": desired})


def import_export(in_path: Path, dest: str, map_path: Path):
    export = load_json(in_path)
    mapping = load_map(map_path)
    mapping["schema"] = 2
    for index, issue in enumerate(export["issues"], start=1):
        key = source_key(issue)
        mapped = mapping["mappings"].get(key)
        if mapped:
            dest_issue, _ = request("GET", f"/repos/{dest}/issues/{mapped['dest_number']}")
        else:
            dest_issue = find_existing(dest, issue)
            if not dest_issue:
                dest_issue, _ = request(
                    "POST",
                    f"/repos/{dest}/issues",
                    {
                        "title": f"[legacy:{issue['source_repo'].split('/')[-1]}#{issue['source_number']}] {issue['title']}",
                        "body": migrated_body(issue),
                    },
                )

        # Re-running repairs any missing discussion/state instead of trusting a partial prior run.
        ensure_comments(dest, dest_issue["number"], issue)
        ensure_state(dest, dest_issue, issue)

        mapping["mappings"][key] = {
            "source_url": issue["source_url"],
            "source_state": issue["state"],
            "source_state_reason": issue.get("state_reason"),
            "dest_number": dest_issue["number"],
            "dest_url": dest_issue["html_url"],
            "expected_comments": len(issue.get("comments", [])),
        }
        save_map(map_path, mapping)
        print(f"[{index}/{len(export['issues'])}] {key} -> {dest_issue['html_url']}")
        time.sleep(0.05)


def reconcile(in_path: Path, dest: str, map_path: Path):
    export = load_json(in_path)
    mapping = load_map(map_path).get("mappings", {})
    failures = []
    seen_dest = set()

    if export.get("issue_count") != len(export.get("issues", [])):
        failures.append(
            f"export issue_count mismatch: header={export.get('issue_count')} actual={len(export.get('issues', []))}"
        )
    if export.get("independent_issue_count") != len(export.get("issues", [])):
        failures.append(
            "independent source count mismatch: "
            f"search={export.get('independent_issue_count')} export={len(export.get('issues', []))}"
        )
    if export.get("source_repo") == RILL_REPO:
        actual_inventory = sum(1 for i in export["issues"] if "inventory" in i.get("labels", []))
        if actual_inventory != RILL_EXPECTED_INVENTORY_ROWS:
            failures.append(
                f"RILL inventory mismatch: expected={RILL_EXPECTED_INVENTORY_ROWS} export={actual_inventory}"
            )

    for issue in export["issues"]:
        key = source_key(issue)
        m = mapping.get(key)
        if not m:
            failures.append(f"unmapped: {key}")
            continue
        if m["dest_number"] in seen_dest:
            failures.append(f"duplicate destination issue: #{m['dest_number']}")
        seen_dest.add(m["dest_number"])

        dest_issue, _ = request("GET", f"/repos/{dest}/issues/{m['dest_number']}")
        body = dest_issue.get("body") or ""
        if marker(issue) not in body:
            failures.append(f"missing source marker: {key}")
        if dest_issue.get("state") != issue.get("state"):
            failures.append(
                f"state mismatch: {key}: source={issue.get('state')} dest={dest_issue.get('state')}"
            )

        comments = paged(f"/repos/{dest}/issues/{m['dest_number']}/comments")
        comment_bodies = [c.get("body") or "" for c in comments]
        for source_comment in issue.get("comments", []):
            cm = comment_marker(source_comment)
            if not any(cm in candidate for candidate in comment_bodies):
                failures.append(f"missing migrated comment: {key} comment={source_comment.get('id')}")

    print(f"source issues: {len(export['issues'])}")
    print(f"mapped issues: {len([i for i in export['issues'] if source_key(i) in mapping])}")
    if failures:
        print("RECONCILIATION FAILED", file=sys.stderr)
        for item in failures:
            print(f"- {item}", file=sys.stderr)
        raise SystemExit(1)
    print("RECONCILIATION PASSED")


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("export")
    p.add_argument("--repo", required=True)
    p.add_argument("--out", type=Path, required=True)
    p = sub.add_parser("import")
    p.add_argument("--in", dest="in_path", type=Path, required=True)
    p.add_argument("--dest", required=True)
    p.add_argument("--map", dest="map_path", type=Path, required=True)
    p = sub.add_parser("reconcile")
    p.add_argument("--in", dest="in_path", type=Path, required=True)
    p.add_argument("--dest", required=True)
    p.add_argument("--map", dest="map_path", type=Path, required=True)
    args = parser.parse_args()
    if args.cmd == "export":
        export_repo(args.repo, args.out)
    elif args.cmd == "import":
        import_export(args.in_path, args.dest, args.map_path)
    else:
        reconcile(args.in_path, args.dest, args.map_path)


if __name__ == "__main__":
    main()
