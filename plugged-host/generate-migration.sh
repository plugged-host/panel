#!/usr/bin/env bash
# Generates a database migration from database/src/schema.ts on the build host and copies it back.
#
#   plugged-host/generate-migration.sh <name>          # -> database/migrations/<timestamp>_<name>/
#   plugged-host/generate-migration.sh <name> <id>     # same, keeping snapshot id <id> (see below)
#   CUSTOM=1 plugged-host/generate-migration.sh <name> # an empty one to write SQL into (data changes)
#
# drizzle-kit only diffs the schema against the newest snapshot; it connects to nothing. Name the
# migration after the feature, without a prefix (`status`, `server_suspensions`).
#
# The panel records applied migrations by their snapshot id. To move a migration that already ran
# somewhere to the end of the chain (after an upstream merge forked it), delete its directory and
# regenerate it with its old id: it is then not applied again, and the chain is linear.
set -euo pipefail

NAME=${1:?usage: generate-migration.sh <name> [snapshot-id]}
KEEP_ID=${2:-}
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BUILD_HOST=${BUILD_HOST:-plugged-build}
BUILD_DIR=${BUILD_DIR:-/root/panel}

rsync -a --delete --exclude node_modules "$ROOT/database/" "$BUILD_HOST:$BUILD_DIR/database/"

ssh "$BUILD_HOST" "set -euo pipefail
  renice -n 15 -p \$\$ >/dev/null; ionice -c 3 -p \$\$
  export PATH=/opt/node/bin:\$PATH
  cd '$BUILD_DIR/database'
  pnpm install --frozen-lockfile >/dev/null
  DATABASE_URL=postgresql://generate@127.0.0.1:1/generate pnpm -s kit:generate --name '$NAME' ${CUSTOM:+--custom}"

new=$(ssh "$BUILD_HOST" "cd '$BUILD_DIR/database/migrations' && ls -1d *_'$NAME' | tail -1")
[ -n "$new" ] || { echo "no migration was generated" >&2; exit 1; }
rsync -a "$BUILD_HOST:$BUILD_DIR/database/migrations/$new" "$ROOT/database/migrations/"

if [ -n "$KEEP_ID" ]; then
  # Only the id changes; the rest stays as drizzle-kit wrote it.
  python3 - "$ROOT/database/migrations/$new/snapshot.json" "$KEEP_ID" <<'EOF'
import json, sys
path, keep = sys.argv[1], sys.argv[2]
text = open(path).read()
old = json.loads(text)["id"]
open(path, "w").write(text.replace(f'"id": "{old}"', f'"id": "{keep}"', 1))
EOF
fi

echo "==> database/migrations/$new"
cat "$ROOT/database/migrations/$new/migration.sql"
