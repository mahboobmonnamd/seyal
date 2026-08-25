#!/usr/bin/env python3
"""Export/import/reconcile GitHub issues for Seyal legacy repositories.

Usage examples:
  GH_TOKEN=... python migration/legacy-issues/migrate_issues.py export \
    --repo mahboobmonnamd/RILL --out migration/legacy-issues/rill-issues.json

  GH_TOKEN=... python migration/legacy-issues/migrate_issues.py import \
    --in migration/legacy-issues/rill-issues.json --dest mahboobmonnamd/seyal \
    --map migration/legacy-issues/issue-map.json

  GH_TOKEN=... python migration/legacy-issues/migrate_issues.py reconcile \
    --in migration/legacy-issues/rill-issues.json --dest mahboobmonnamd/seyal \
    --map migration/legacy-issues/issue-map.json

The importer is intentionally conservative:
- preserves source identity in every destination issue body;
- keeps legacy decisions historical unless current Seyal authority accepts them;
- preserves closed/rejected/deferred issues rather than dropping them;
- never deletes or mutates the source repositories;
- is idempotent via the mapping file and source marker search.
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


def export_repo(repo: str, out_path: Path):
    # REST /issues includes PRs, so filter pull_request records.
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
    export = {
        "schema": 1,
        "source_repo": repo,
        "issue_count": len(issues),
        "issues": issues,
    }
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(export, indent=2, ensure_ascii=False) + "\n")
    print(f"exported {len(issues)} issues from {repo} -> {out_path}")


def load_json(path: Path):
    return json.loads(path.read_text())


def load_map(path: Path):
    if not path.exists():
        return {"schema": 1, "mappings": {}}
    return load_json(path)


def save_map(path: Path, mapping):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(mapping, indent=2, ensure_ascii=False) + "\n")


def source_key(issue):
    return f"{issue['source_repo']}#{issue['source_number']}"


def marker(issue):
    return f"Legacy-Source: {source_key(issue)}"


def migrated_body(issue):
    header = f"""> [!IMPORTANT]\n> Migrated historical issue. Preservation does **not** make legacy architecture current Seyal authority. Current Seyal ADRs/specs/constitution win on conflict.\n\n<!-- {marker(issue)} -->\n\n## Legacy source\n\n- Repository: `{issue['source_repo']}`\n- Issue: #{issue['source_number']}\n- URL: {issue['source_url']}\n- Original state: `{issue['state']}`\n- State reason: `{issue.get('state_reason')}`\n- Created: `{issue.get('created_at')}`\n- Updated: `{issue.get('updated_at')}`\n- Closed: `{issue.get('closed_at')}`\n- Original labels: {', '.join(issue.get('labels') or []) or '(none)'}\n- Original milestone: {issue.get('milestone') or '(none)'}\n- Migration classification: `{issue.get('migration_classification', 'historical-unreviewed')}`\n\n## Original issue body\n\n"""
    return header + (issue.get("body") or "(empty)")


def find_existing(dest: str, issue):
    q = urllib.parse.quote(f'"{marker(issue)}" repo:{dest} is:issue')
    payload, _ = request("GET", f"/search/issues?q={q}&per_page=5")
    items = payload.get("items", []) if payload else []
    return items[0] if items else None


def import_export(in_path: Path, dest: str, map_path: Path):
    export = load_json(in_path)
    mapping = load_map(map_path)
    for index, issue in enumerate(export["issues"], start=1):
        key = source_key(issue)
        if key in mapping["mappings"]:
            continue
        existing = find_existing(dest, issue)
        if existing:
            dest_issue = existing
        else:
            dest_issue, _ = request(
                "POST",
                f"/repos/{dest}/issues",
                {"title": f"[legacy:{issue['source_repo'].split('/')[-1]}#{issue['source_number']}] {issue['title']}", "body": migrated_body(issue)},
            )
            # Recreate discussion as comments with preserved authors/timestamps in text.
            for c in issue.get("comments", []):
                comment = (
                    f"_Migrated comment by @{c.get('author') or 'unknown'}; original timestamp "
                    f"`{c.get('created_at')}`._\n\n{c.get('body') or '(empty)'}"
                )
                request("POST", f"/repos/{dest}/issues/{dest_issue['number']}/comments", {"body": comment})
            if issue["state"] == "closed":
                request("PATCH", f"/repos/{dest}/issues/{dest_issue['number']}", {"state": "closed"})
        mapping["mappings"][key] = {
            "source_url": issue["source_url"],
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
        if marker(issue) not in (dest_issue.get("body") or ""):
            failures.append(f"missing source marker: {key}")
        comments = paged(f"/repos/{dest}/issues/{m['dest_number']}/comments")
        if len(comments) < len(issue.get("comments", [])):
            failures.append(
                f"comment count short: {key}: source={len(issue.get('comments', []))} dest={len(comments)}"
            )
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
