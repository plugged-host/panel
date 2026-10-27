#!/usr/bin/env bash
# Installs a built panel binary and restarts the panel, rolling back if it does not come up.
#
#   plugged-host/deploy.sh [binary]          # the dev panel (default binary target/plugged-host/panel-rs)
#   plugged-host/deploy.sh --prod [binary]   # the prod panel: emergency fixes only
#
# Two panels: dev (unit calagopus-dev, /opt/calagopus-dev, /etc/calagopus-dev/panel.env, one box)
# and prod (unit calagopus, /opt/calagopus, /etc/calagopus/panel.env). A release is v{VERSION}, so a
# prod fix is a new VERSION (3.0.0 -> 3.0.1): --prod refuses a release prod already ran unless
# SAME_RELEASE=1. A box with one panel only uses --prod.
#
# Each deploy lives in <base>/releases/<release>-<UTC time>/panel-rs and <base>/current points at
# the running one; the last KEEP deploys stay for rollbacks.
#
# Prod on two boxes: /etc/calagopus/deploy.env (not in git) names the other box as an ssh alias,
# PEER_SSH=<alias>. The release is copied there too; where the `calagopus` unit is enabled it is
# restarted after this box's came up, and `calagopus-tasks` (background tasks, one box at a time)
# is restarted wherever it runs. Migrations: with DATABASE_MIGRATE=true in the panel.env the panel
# runs them on start (one box); with false they run once here, before any restart.
set -euo pipefail

# `panel-rs version` parses the environment first; it needs these set but connects to nothing.
release_of() {
  env -i PATH=/usr/bin:/bin APP_ENCRYPTION_KEY=version-check DATABASE_URL=postgresql://version@127.0.0.1:1/version \
    "$1" version | awk '/^release /{print $2}'
}

ROOT=$(cd "$(dirname "$0")/.." && pwd)
PROD=0
[ "${1:-}" = --prod ] && { PROD=1; shift; }
BINARY=${1:-$ROOT/target/plugged-host/panel-rs}
PEER_SSH=
if [ "$PROD" = 1 ]; then
  BASE=/opt/calagopus
  UNIT=calagopus
  TASKS=calagopus-tasks
  ENV_FILE=/etc/calagopus/panel.env
  KEEP=${KEEP:-3}
  HEALTH_URL=${HEALTH_URL:-http://127.0.0.1:8730/api/settings}
  [ -f /etc/calagopus/deploy.env ] && . /etc/calagopus/deploy.env
else
  BASE=/opt/calagopus-dev
  UNIT=calagopus-dev
  TASKS=
  ENV_FILE=/etc/calagopus-dev/panel.env
  KEEP=${KEEP:-2}
  HEALTH_URL=${HEALTH_URL:-http://127.0.0.1:8732/api/settings}
  [ -f "/etc/systemd/system/$UNIT.service" ] || { echo "no dev panel on this box ($UNIT); --prod deploys the prod panel" >&2; exit 1; }
fi
peer() { ssh -o BatchMode=yes -o ConnectTimeout=10 "$PEER_SSH" "$@"; }

release=$(release_of "$BINARY")
[ -n "$release" ] || { echo "cannot read the release of $BINARY" >&2; exit 1; }

if [ "$PROD" = 1 ] && [ "${SAME_RELEASE:-0}" != 1 ] && compgen -G "$BASE/releases/$release-*" >/dev/null; then
  echo "prod already ran $release: a prod fix is a new VERSION (bump it, commit, build), or SAME_RELEASE=1 to redeploy $release" >&2
  exit 1
fi

# The release is only v{VERSION}, so every deploy gets its own directory.
dir="$BASE/releases/$release-$(date -u +%Y%m%d-%H%M%S)"
install -d -m 755 "$dir"
install -m 755 "$BINARY" "$dir/panel-rs.new"
mv -f "$dir/panel-rs.new" "$dir/panel-rs"

switch_to() { # switch_to <release dir>: this box and the peer point at it
  ln -sfn "$1" "$BASE/current.new"
  mv -Tf "$BASE/current.new" "$BASE/current"
  if [ -n "$PEER_SSH" ]; then
    peer "test -x '$1/panel-rs' && ln -sfn '$1' '$BASE/current.new' && mv -Tf '$BASE/current.new' '$BASE/current'"
  fi
}

previous=$(readlink "$BASE/current" 2>/dev/null || true)
peer_running=0
if [ -n "$PEER_SSH" ]; then
  peer "install -d -m 755 '$dir'"
  rsync -a "$dir/panel-rs" "$PEER_SSH:$dir/panel-rs"
  peer "systemctl is-enabled -q $UNIT" 2>/dev/null && peer_running=1
fi
switch_to "$dir"
echo "==> $release (was ${previous##*/})$([ -n "$PEER_SSH" ] && echo ", also on $PEER_SSH")"

# Migrations once, before any instance restarts, when the panel does not run them on start.
if grep -qE '^DATABASE_MIGRATE="?false' "$ENV_FILE"; then
  echo "==> migrations"
  systemd-run --quiet --wait --pipe --collect -p User=calagopus -p WorkingDirectory=/var/lib/calagopus \
    -p EnvironmentFile="$ENV_FILE" "$dir/panel-rs" database-migrator migrate
fi

up() { # up <command prefix>: waits for the panel's health URL (60 tries)
  for _ in $(seq 1 60); do
    if $1 curl -fs -o /dev/null "$HEALTH_URL"; then return 0; fi
    sleep 2
  done
  return 1
}

rollback() {
  echo "==> $release did not come up ($1), rolling back to ${previous##*/}" >&2
  if [ -n "$previous" ]; then
    switch_to "$previous"
    systemctl restart "$UNIT"
    [ "$peer_running" = 1 ] && peer "systemctl restart $UNIT"
    [ -n "$TASKS" ] && { systemctl try-restart "$TASKS" 2>/dev/null || true; }
    [ -n "$PEER_SSH" ] && [ -n "$TASKS" ] && { peer "systemctl try-restart $TASKS" 2>/dev/null || true; }
  fi
  exit 1
}

systemctl restart "$UNIT"
up "" || { journalctl -u "$UNIT" -n 30 --no-pager >&2 || true; rollback "this box"; }
echo "==> up"
if [ "$peer_running" = 1 ]; then
  peer "systemctl restart $UNIT"
  up "peer" || { peer "journalctl -u $UNIT -n 30 --no-pager" >&2 || true; rollback "$PEER_SSH"; }
  echo "==> up on $PEER_SSH"
fi
[ -n "$TASKS" ] && { systemctl try-restart "$TASKS" 2>/dev/null || true; }
[ -n "$PEER_SSH" ] && [ -n "$TASKS" ] && { peer "systemctl try-restart $TASKS" 2>/dev/null || true; }

# Keep the newest releases (and whatever is running), on both boxes.
prune="ls -1dt '$BASE'/releases/*/ | tail -n +$((KEEP + 1)) | while read -r old; do [ \"\${old%/}\" = '$dir' ] || rm -rf \"\$old\"; done"
bash -c "$prune"
[ -n "$PEER_SSH" ] && peer "$prune"
exit 0
