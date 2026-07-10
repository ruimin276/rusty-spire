# STS2 Oracle Bridge Spike Notes

Observed local install:

```text
/Users/ainmelody/Library/Application Support/Steam/steamapps/common/Slay the Spire 2/SlayTheSpire2.app
```

Observed release metadata:

```json
{
  "commit": "460a0ece",
  "version": "v0.103.3",
  "date": "2026-05-29T13:36:05-07:00",
  "branch": "v0.103.3",
  "main_assembly_hash": 418053415
}
```

Relevant bundle facts:

- Main executable: `Contents/MacOS/Slay the Spire 2`
- Managed game assembly appears in both architecture resource folders as `sts2.dll`.
- `GodotSharp.dll`, `0Harmony.dll`, and `MonoMod.*.dll` are bundled.
- Runtime target is `.NETCoreApp,Version=v9.0/osx-arm64`.
- No obvious app-bundle mod/workshop directory was found during the read-only inspection.

Managed API findings from `sts2.dll` metadata:

- Mod namespace exists: `MegaCrit.Sts2.Core.Modding`.
- `ModInitializerAttribute` is valid on classes and takes one string constructor argument.
- Manifest fields serialize as `id`, `name`, `author`, `description`, `version`,
  `has_pck`, `has_dll`, `dependencies`, and `affects_gameplay`.
- `ModManager` exposes `ReadModsInDirRecursive`, `ReadModManifest`,
  `TryLoadMod`, `CallModInitializer`, and `GetLoadedMods`.
- `ModManager.Initialize` scans `Path.GetDirectoryName(OS.GetExecutablePath())`
  plus `mods`. On the inspected macOS Steam build this is
  `SlayTheSpire2.app/Contents/MacOS/mods`, not the Steam game-root `mods`
  directory.
- `TryLoadMod` resolves the DLL as `Path.Combine(mod.path, modId + ".dll")`.
  For manifest id `sls2_combat_oracle`, the DLL must be named
  `sls2_combat_oracle.dll`.
- `ModSettings` contains `PlayerAgreedToModLoading` and `ModList`.
- `SettingsSaveMod` contains `Id`, `Source`, and `IsEnabled`.
- Embedded strings include `mods_enabled`, `mod_list`, and `is_enabled`; this
  means a mod folder can be discovered without necessarily being loaded.
- `SettingsSave` stores this under top-level `mod_settings`. On the inspected
  Steam settings file it was initially `null`, so the bridge includes
  `bridge/enable_bridge_settings.py` to write the active settings file:
  `{"mods_enabled": true, "mod_list": [{"id": "sls2_combat_oracle", "source": "mods_directory", "is_enabled": true}]}`.
- `CombatManager.Instance.DebugOnlyGetState()` exposes the active combat state.
- `NetCombatCardDb.Instance.TryGetCardId(card, out id)` exposes stable live
  combat-card ids used by network play-card actions.
- `CardModel.CanPlayTargeting(target)` gives the game's own legal-target check.
- `NetFullCombatState.FromRun(runState, justFinishedAction)` snapshots creatures,
  combat piles, relics, powers, and RNG for multiplayer checksum/rejoin messages,
  but the decompiled code does not expose a corresponding apply/restore method.
  Search hits only serialize, deserialize, checksum, and diagnostic paths.
- `CombatReplay` and `CombatReplayWriter` record/replay global run actions; they
  rebuild through `RunManager` and game UI state rather than giving a detached
  combat clone suitable for search branching.
- A first install into the Steam game-root `mods/` directory was not loaded
  because that is not the directory scanned by the macOS executable.
- Direct `open SlayTheSpire2.app` logged Steamworks `No appID found`; runtime
  validation should launch from Steam, then check the bridge log and HTTP
  health endpoint.

Implication:

- The Python solver side is ready to talk to an oracle over HTTP.
- A C# bridge mod source now exists under `bridge/`.
- Current bridge status supports health/current export/action listing/hash and
  writes load diagnostics to `~/Library/Logs/sls2-combat-oracle.log`.
- Live mutation endpoints now exist for experiments: `/live_step`,
  `/live_checkpoint`, and `/live_restore_checkpoint`. These intentionally do
  not satisfy `/step` because they mutate/reload the visible run.
- Full solver integration still needs branchable `/step`. The practical options
  are now either a custom hydrator from exported/net combat state into a detached
  `CombatState`, or a guarded live-mutation endpoint for manual experiments only.
  The latter is not safe for solver search because it mutates the user's active
  combat.

Do not patch or overwrite the Steam app bundle until the preferred mod loading
path is confirmed.
