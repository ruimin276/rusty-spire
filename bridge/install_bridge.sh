#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_DIR="$PROJECT_DIR/dist/sls2_combat_oracle"
DEST_ROOT="${1:-}"

if [[ ! -d "$SOURCE_DIR" ]]; then
  echo "Bridge has not been built. Run bridge/build_bridge.sh first." >&2
  exit 1
fi

if [[ -z "$DEST_ROOT" ]]; then
  CANDIDATES=(
    "$HOME/Library/Application Support/Steam/steamapps/common/Slay the Spire 2/SlayTheSpire2.app/Contents/MacOS/mods"
    "$HOME/Library/Application Support/Mega Crit/Slay the Spire 2/mods"
    "$HOME/Library/Application Support/MegaCrit/Slay the Spire 2/mods"
    "$HOME/Library/Application Support/Slay the Spire 2/mods"
    "$HOME/Library/Application Support/Steam/steamapps/common/Slay the Spire 2/mods"
  )
  for candidate in "${CANDIDATES[@]}"; do
    if [[ -d "$candidate" ]]; then
      DEST_ROOT="$candidate"
      break
    fi
  done
fi

if [[ -z "$DEST_ROOT" ]]; then
  echo "Could not locate STS2 mods directory automatically." >&2
  echo "For the Steam macOS build, pass the executable-adjacent mods directory:" >&2
  echo "  bridge/install_bridge.sh \"/path/to/SlayTheSpire2.app/Contents/MacOS/mods\"" >&2
  exit 1
fi

mkdir -p "$DEST_ROOT"
rm -rf "$DEST_ROOT/sls2_combat_oracle"
cp -R "$SOURCE_DIR" "$DEST_ROOT/sls2_combat_oracle"

echo "$DEST_ROOT/sls2_combat_oracle"
echo "Enable 'SLS2 Combat Oracle' in STS2's in-game Modding screen, then check ~/Library/Logs/sls2-combat-oracle.log."
