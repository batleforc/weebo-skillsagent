#!/usr/bin/env bash
# End-to-end eval: run weebo-techdoc twice on the fixture through `claude -p`.
# Run 1 must produce a valid tree that names the real routes and grades the
# missing gen:api-docs task ❌. Between runs we add a hand-written note and a new
# handler, and delete the web component; run 2 must keep the note, pick up the
# handler, and keep web's page with an #orphaned warning (merge --partial: its
# other blocks untouched and not reported stale). crates/auth-core/ is a nested
# component: its page must be named auth-core.md, at components/ or grouped in a
# subfolder, and must survive run 2 without being duplicated.
# Costs API credits (two runs, capped by BUDGET each); not for CI.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
skill="$(cd "$here/.." && pwd)"
budget="${BUDGET:-3}"
work="$(mktemp -d)"
logs="$(mktemp -d)"  # outside the repo, so logs never show up in git status
echo "work dir: $work, logs: $logs"

fail() { echo "EVAL FAIL: $*" >&2; exit 1; }

cp -r "$here/fixtures/sample/." "$work/"
cd "$work"
mkdir -p .claude/skills
ln -s "$skill" .claude/skills/weebo-techdoc
git init -q
git add -A
git -c user.name=eval -c user.email=eval@local commit -qm init

run() {
  claude -p "Use the weebo-techdoc skill on this repository." \
    --permission-mode bypassPermissions --max-budget-usd "$budget" > "$logs/run-$1.log" 2>&1 \
    || fail "claude run $1 exited non-zero (see $logs/run-$1.log)"
}

check() {
  python3 "$skill/scripts/techdoc.py" check docs/techdoc || fail "techdoc.py check failed after run $1"
}

run 1
check 1
# Pages are named after the component's directory (SKILL.md, step 1).
api_page=docs/techdoc/components/api.md
web_page=docs/techdoc/components/web.md
[ -f "$api_page" ] || fail "no page for component api at $api_page"
[ -f "$web_page" ] || fail "no page for component web at $web_page"
grep -q list_items "$api_page" || fail "list_items not documented in $api_page"
grep -q listItems "$web_page" || fail "listItems not documented in $web_page"
# Nested component: named after its own directory, grouping subfolder optional.
auth_pages="$(find docs/techdoc/components -name auth-core.md)"
[ "$(printf '%s' "$auth_pages" | grep -c .)" -eq 1 ] \
  || fail "expected one auth-core.md page for crates/auth-core/, found: ${auth_pages:-none}"
auth_page="$auth_pages"
grep -q verify_token "$auth_page" || fail "verify_token not documented in $auth_page"
grep -q AUTH_SECRET "$auth_page" || fail "AUTH_SECRET not documented in $auth_page"
grep -rq DATABASE_URL docs/techdoc || fail "DATABASE_URL not documented"
grep -i 'api docs\|gen:api-docs' docs/techdoc/maintenance.md | grep -q '❌' \
  || fail "gen:api-docs row is not ❌"
outside="$(git status --porcelain -uall | grep -v ' docs/techdoc/' | grep -v ' \.claude/' || true)"
[ -z "$outside" ] || fail "files changed outside docs/techdoc/: $outside"

echo "MY HAND NOTE" >> docs/techdoc/index.md
echo "WEB HAND NOTE" >> "$web_page"
rm -rf web
python3 - <<'EOF'
from pathlib import Path
p = Path("api/src/main.rs")
s = p.read_text()
s = s.replace("#[tokio::main]", '''async fn delete_item(State(db): State<PgPool>, axum::extract::Path(id): axum::extract::Path<i64>) {
    sqlx::query("DELETE FROM items WHERE id = $1").bind(id).execute(&db).await.unwrap();
}

#[tokio::main]''')
s = s.replace('.route("/items", get(list_items).post(create_item))',
              '.route("/items", get(list_items).post(create_item))\n        .route("/items/{id}", axum::routing::delete(delete_item))')
p.write_text(s)
EOF
git add -A
git -c user.name=eval -c user.email=eval@local commit -qm "add delete_item"

run 2
check 2
grep -q 'MY HAND NOTE' docs/techdoc/index.md || fail "hand-written note was lost"
grep -q delete_item "$api_page" || fail "delete_item not picked up in $api_page on re-run"
[ -f "$auth_page" ] || fail "nested component page $auth_page moved or deleted on re-run"
[ "$(find docs/techdoc/components -name '*.md' | wc -l)" -eq 3 ] \
  || fail "re-run created extra component pages: $(find docs/techdoc/components -name '*.md')"
[ -f "$web_page" ] || fail "orphaned page $web_page was deleted"
grep -q 'techdoc:begin [^ ]*#orphaned -->' "$web_page" || fail "no #orphaned block in $web_page"
grep -q listItems "$web_page" || fail "orphaned page lost its generated content"
grep -q 'WEB HAND NOTE' "$web_page" || fail "hand-written note lost from orphaned page"
! grep -i '^stale blocks:.*#' "$logs/run-2.log" | grep -q "components/web#" \
  || fail "orphaned page's blocks reported stale (merge --partial not used)"

echo "EVAL PASS ($work)"
