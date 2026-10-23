#!/usr/bin/env bash
# Builds frontend/public/translations/en.json from frontend/src/translations.ts on the build host,
# copies it back, and checks that the offered languages have every English string, the panel's
# and the mails'. Fails below 100%; build.sh runs it before every build.
#
#   plugged-host/translations.sh            # LANGUAGES (default: every language that must be complete)
#   plugged-host/translations.sh --missing  # also lists the missing keys
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BUILD_HOST=${BUILD_HOST:-plugged-build}
BUILD_DIR=${BUILD_DIR:-/root/panel}
# English is the source; German is complete since v3 Phase 1, the others join in Phase 6.
LANGUAGES=${LANGUAGES:-de}

rsync -a --delete --exclude node_modules --exclude dist "$ROOT/frontend/" "$BUILD_HOST:$BUILD_DIR/frontend/"
ssh "$BUILD_HOST" "set -euo pipefail
  renice -n 15 -p \$\$ >/dev/null; ionice -c 3 -p \$\$
  export PATH=/opt/node/bin:\$PATH
  cd '$BUILD_DIR/frontend'
  pnpm install --frozen-lockfile >/dev/null
  pnpm -s build:translations"
rsync -a "$BUILD_HOST:$BUILD_DIR/frontend/public/translations/en.json" "$ROOT/frontend/public/translations/en.json"

python3 - "$ROOT" "${1:-}" $LANGUAGES <<'PY'
import json, sys

root, flag, languages = sys.argv[1], sys.argv[2], sys.argv[3:]

def flatten(value, prefix=""):
    if isinstance(value, dict):
        out = {}
        for key, item in value.items():
            out.update(flatten(item, f"{prefix}.{key}" if prefix else key))
        return out
    return {prefix: value}

sources = {
    "panel": (f"{root}/frontend/public/translations/en.json", f"{root}/frontend/public/translations/{{}}.json"),
    "mails": (f"{root}/shared/mails/variables/en.json", f"{root}/shared/mails/variables/{{}}.json"),
}
failed = False
for language in languages:
    for name, (english_path, path) in sources.items():
        english = flatten(json.load(open(english_path)))
        try:
            translated = flatten(json.load(open(path.format(language))))
        except FileNotFoundError:
            translated = {}
        missing = [key for key in english if key not in translated or translated[key] in ("", None)]
        share = 100 * (len(english) - len(missing)) / max(len(english), 1)
        print(f"{language} {name}: {share:.1f}% ({len(missing)} of {len(english)} missing)")
        if missing:
            failed = True
            if flag == "--missing":
                for key in missing:
                    print(f"  {key}: {english[key]!r}")
sys.exit(1 if failed else 0)
PY
