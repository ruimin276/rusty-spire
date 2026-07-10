# STS2 Combat Oracle Bridge

This directory contains the C# side of the solver bridge.

It is an STS2 DLL mod that starts a local HTTP server on
`http://127.0.0.1:17351` by default. The Python solver talks to this server.

## Build

Requires .NET 9 SDK.

```bash
bridge/build_bridge.sh "/Users/ainmelody/Library/Application Support/Steam/steamapps/common/Slay the Spire 2"
```

The staged mod is written to:

```text
bridge/dist/sls2_combat_oracle
```

That folder contains:

- `manifest.json`
- `Sls2CombatOracle.Bridge.dll`

## Install

For the local Steam macOS install inspected during development, `ModManager`
uses `Path.GetDirectoryName(OS.GetExecutablePath())/mods`. That means the loader
scans the executable-adjacent directory:

```text
/Users/ainmelody/Library/Application Support/Steam/steamapps/common/Slay the Spire 2/SlayTheSpire2.app/Contents/MacOS/mods
```

Install the staged mod there:

```bash
bridge/install_bridge.sh "/Users/ainmelody/Library/Application Support/Steam/steamapps/common/Slay the Spire 2/SlayTheSpire2.app/Contents/MacOS/mods"
```

The installer can also try common locations automatically:

```bash
bridge/install_bridge.sh
```

After installation, launch STS2 and open the in-game Modding screen from
Settings. The game assembly exposes `mods_enabled`, `mod_list`, and per-mod
`is_enabled` settings, so placing files in `mods/` is not enough by itself if
the mod has not been enabled/accepted in that screen.

If the Modding screen does not persist the bridge setting, the helper below
updates the same settings file shape used by STS2. Quit the game before running
it. When Steam is initialized, it targets the newest
`~/Library/Application Support/SlayTheSpire2/steam/*/settings.save`; otherwise
it falls back to `default/1/settings.save`:

```bash
python3 bridge/enable_bridge_settings.py
```

To back out without removing files:

```bash
python3 bridge/enable_bridge_settings.py --disable
```

The DLL filename must match the manifest id. For this bridge, the installed
files are:

- `manifest.json`
- `sls2_combat_oracle.dll`

Do not patch the Steam app bundle directly unless the official mod loading path
cannot be made to work and you have a backup.

## Verify

Quit STS2, then launch it from Steam. Directly opening
`SlayTheSpire2.app` on the inspected macOS build logged `No appID found` from
Steamworks and did not prove mod loading correctly.

With the mod enabled, the bridge writes a diagnostic line as soon as its
initializer runs:

```text
~/Library/Logs/sls2-combat-oracle.log
```

Then run:

```bash
PYTHONPATH=src python3 -m sls2_combat_solver.cli oracle-health
```

Enter a combat and export the current state:

```bash
PYTHONPATH=src python3 -m sls2_combat_solver.cli export --output scenario.json
```

## Implemented Endpoints

- `POST /health`
- `POST /export_state`
- `POST /legal_actions`
- `POST /state_hash`
- `POST /live_step`
- `POST /live_checkpoint`
- `POST /live_restore_checkpoint`

`POST /step` is intentionally not complete yet. A solver needs branchable state
transitions, not mutation of the live combat. `NetFullCombatState` can snapshot
combat state for checksum/rejoin diagnostics, but the inspected game code does
not expose a matching apply/restore method. The next bridge task is a custom
detached combat hydrator, or a separately guarded live-step endpoint for manual
experiments only.

## Live Mutation Workflow

These endpoints mutate the active game. They are guarded by
`allow_live_mutation=true` and should only be used for experiments.

After entering the combat you want to test:

```bash
PYTHONPATH=src python3 -m sls2_combat_solver.cli export --output scenario.json
PYTHONPATH=src python3 -m sls2_combat_solver.cli live-checkpoint --allow-live-mutation
```

Apply one legal action by id:

```bash
PYTHONPATH=src python3 -m sls2_combat_solver.cli live-step \
  --scenario scenario.json \
  --action-id 'play:2:enemy_0' \
  --allow-live-mutation \
  --output after-bash.json
```

Return to the saved live checkpoint:

```bash
PYTHONPATH=src python3 -m sls2_combat_solver.cli live-restore-checkpoint \
  --allow-live-mutation \
  --output restored.json
```

The restore path captures `RunManager.ToSave(currentCombatRoom)` and reloads it
through the game's saved-run loader. It needs live validation in STS2 before it
is used for beam-search timing.
