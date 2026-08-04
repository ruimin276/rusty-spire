#!/usr/bin/env bash
set -euo pipefail

GAME_DIR="${1:-/Users/ainmelody/Library/Application Support/Steam/steamapps/common/Slay the Spire 2}"
APP_DIR="$GAME_DIR/SlayTheSpire2.app"
MANAGED_DIR="$APP_DIR/Contents/Resources/data_sts2_macos_arm64"
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUTPUT_DIR="$PROJECT_DIR/dist/sls2_combat_oracle"

DOTNET_BIN="${DOTNET_BIN:-dotnet}"
if ! command -v "$DOTNET_BIN" >/dev/null 2>&1; then
  if [[ -x /usr/local/share/dotnet/dotnet ]]; then
    DOTNET_BIN=/usr/local/share/dotnet/dotnet
  fi
fi

if ! command -v "$DOTNET_BIN" >/dev/null 2>&1; then
  echo "dotnet SDK is required to build the bridge. Install .NET 9 SDK, then rerun." >&2
  exit 1
fi

if [[ ! -f "$MANAGED_DIR/sts2.dll" ]]; then
  echo "Could not find sts2.dll at $MANAGED_DIR" >&2
  exit 1
fi

rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"

"$DOTNET_BIN" build "$PROJECT_DIR/Sls2CombatOracle.Bridge/Sls2CombatOracle.Bridge.csproj" \
  -c Release \
  -p:Sts2ManagedDir="$MANAGED_DIR"

cp "$PROJECT_DIR/manifest.json" "$OUTPUT_DIR/manifest.json"
cp "$PROJECT_DIR/Sls2CombatOracle.Bridge/bin/Release/net9.0/Sls2CombatOracle.Bridge.dll" "$OUTPUT_DIR/sls2_combat_oracle.dll"

echo "$OUTPUT_DIR"
