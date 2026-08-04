using System.Collections;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Context;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Multiplayer;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.GameActions;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Models.Characters;
using MegaCrit.Sts2.Core.Models.Encounters;
using MegaCrit.Sts2.Core.Modding;
using MegaCrit.Sts2.Core.Multiplayer;
using MegaCrit.Sts2.Core.Nodes;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Saves;
using MegaCrit.Sts2.Core.TestSupport;
using MegaCrit.Sts2.Core.Unlocks;

namespace Sls2CombatOracle.Bridge;

internal sealed class ReflectionCombatBridge : ICombatBridge
{
    private readonly Assembly _gameAssembly;
    private readonly Type? _combatManagerType;
    private SerializableRun? _liveCheckpoint;
    private DateTimeOffset? _liveCheckpointCreatedAt;

    public ReflectionCombatBridge(Assembly gameAssembly)
    {
        _gameAssembly = gameAssembly;
        _combatManagerType = _gameAssembly.GetType("MegaCrit.Sts2.Core.Combat.CombatManager");
        BridgeLog.Info($"loaded game assembly {_gameAssembly.FullName}");
    }

    public object ExportState()
    {
        var manager = CurrentCombatManager();
        var state = CurrentCombatState(manager);
        return ExportStateObject(state);
    }

    public object ExportSimulatorSnapshot()
    {
        EnsureQuiescentDecisionBoundary();
        return ExportSimulatorSnapshotObject();
    }

    public object RngVector(uint seed, int count, int maxExclusive)
    {
        var safeCount = Math.Clamp(count, 1, 1_000);
        if (maxExclusive < 1)
        {
            throw new OracleHttpException(400, "max_exclusive must be positive");
        }
        var rng = new MegaCrit.Sts2.Core.Random.Rng(seed);
        var values = new List<int>(safeCount);
        for (var index = 0; index < safeCount; index++)
        {
            values.Add(rng.NextInt(maxExclusive));
        }
        return new Dictionary<string, object?>
        {
            ["algorithm"] = RngAlgorithm(),
            ["seed"] = seed,
            ["max_exclusive"] = maxExclusive,
            ["values"] = values,
            ["counter"] = rng.Counter
        };
    }

    public object StartDebugNibbit(bool allowLiveMutation, int timeoutMilliseconds)
    {
        if (!allowLiveMutation)
        {
            throw new InvalidOperationException(
                "/debug_start_nibbit replaces the active run. Set allow_live_mutation=true to acknowledge this."
            );
        }
        if (CurrentRunState() != null)
        {
            throw new OracleHttpException(
                409,
                "A run is already active. Restart STS2 at the main menu before creating the debug fixture."
            );
        }

        TestMode.IsOn = true;
        LocalContext.NetId = 1uL;
        var player = Player.CreateForNewRun<Ironclad>(UnlockState.all, 1uL);
        var runState = RunState.CreateForTest(
            players: new List<Player> { player },
            ascensionLevel: 8,
            seed: "SIM-FIXTURE"
        );
        RunManager.Instance.SetUpTest(
            runState,
            new NetSingleplayerGameService(),
            disableCombatStateSync: true,
            shouldSave: false
        );

        var encounter = ModelDb.Encounter<NibbitsWeak>().ToMutable();
        encounter.GenerateMonstersWithSlots(runState);
        var combat = new CombatState(encounter: encounter, runState: runState);
        combat.AddPlayer(player);
        foreach (var (monster, slot) in encounter.MonstersWithSlots)
        {
            var enemy = combat.CreateCreature(monster, CombatSide.Enemy, slot);
            combat.AddCreature(enemy);
        }
        CombatManager.Instance.SetUpCombat(combat);
        CombatManager.Instance.StartCombatInternal().GetAwaiter().GetResult();
        WaitForDecisionBoundary(timeoutMilliseconds);
        return ExportSimulatorSnapshotObject();
    }

    public IReadOnlyList<object> LegalActions(JsonElement state)
    {
        try
        {
            var liveActions = LiveLegalActions();
            if (liveActions.Count > 0)
            {
                return liveActions;
            }
        }
        catch (Exception error)
        {
            BridgeLog.Error("live legal action reflection failed; falling back to exported JSON", error);
        }
        return JsonLegalActions(state);
    }

    private IReadOnlyList<object> LiveLegalActions()
    {
        var manager = CurrentCombatManager();
        var state = CurrentCombatState(manager);
        var player = GetEnumerableProperty(state, "Players").FirstOrDefault();
        if (player == null)
        {
            return [];
        }

        var combat = GetProperty(player, "PlayerCombatState");
        if (combat == null)
        {
            return [];
        }

        var enemies = GetEnumerableProperty(state, "Enemies")
            .Where(creature => creature != null && !IsCreatureDead(creature))
            .Select((creature, index) => new TargetRef(
                $"enemy_{index}",
                GetAnyProperty(creature!, "Id", "NetId", "ModelId")?.ToString() ?? $"enemy_{index}",
                index,
                GetAnyProperty(creature!, "CombatId")?.ToString(),
                creature
            ))
            .ToList();

        var actions = new List<object>();
        var handIndex = 0;
        foreach (var card in EnumerateObject(GetProperty(combat, "Hand")))
        {
            if (card == null)
            {
                handIndex++;
                continue;
            }

            var cardIndex = TryGetCombatCardIndex(card) ?? $"hand_{handIndex}";
            var cardId = GetAnyProperty(card, "Id", "ModelId")?.ToString();
            var targetType = GetAnyProperty(card, "TargetType")?.ToString();
            var energyCost = GetEnergyCost(card);
            if (CanPlayTargeting(card, null))
            {
                actions.Add(CardAction(cardIndex, handIndex, cardId, targetType, energyCost, null));
            }
            foreach (var enemy in enemies)
            {
                if (enemy.LiveCreature != null && CanPlayTargeting(card, enemy.LiveCreature))
                {
                    actions.Add(CardAction(cardIndex, handIndex, cardId, targetType, energyCost, enemy));
                }
            }
            handIndex++;
        }

        actions.Add(new { id = "end_turn", type = "end_turn" });
        return actions;
    }

    private IReadOnlyList<object> JsonLegalActions(JsonElement state)
    {
        var actions = new List<object>();
        if (!state.TryGetProperty("hand", out var hand) || hand.ValueKind != JsonValueKind.Array)
        {
            return actions;
        }

        var enemies = LiveEnemyTargets(state).ToList();
        var handIndex = 0;
        foreach (var card in hand.EnumerateArray())
        {
            var cardIndex = CardActionIndex(card, handIndex);
            var cardId = StringProperty(card, "id") ?? StringProperty(card, "model_id");
            var targetType = StringProperty(card, "target_type");
            var energyCost = NullableIntProperty(card, "cost");
            if (enemies.Count == 0)
            {
                actions.Add(CardAction(cardIndex, handIndex, cardId, targetType, energyCost, null));
            }
            else
            {
                foreach (var enemy in enemies)
                {
                    actions.Add(CardAction(cardIndex, handIndex, cardId, targetType, energyCost, enemy));
                }
            }
            handIndex++;
        }

        actions.Add(new { id = "end_turn", type = "end_turn" });
        return actions;
    }

    public object Step(JsonElement state, JsonElement action)
    {
        throw new NotSupportedException(
            "Branchable /step requires combat-state clone/restore. Current bridge supports /health, /export_state, /legal_actions, and /state_hash."
        );
    }

    public object LiveStep(JsonElement action, bool allowLiveMutation, int timeoutMilliseconds)
    {
        if (!allowLiveMutation)
        {
            throw new InvalidOperationException(
                "/live_step mutates the active combat. Set allow_live_mutation=true to acknowledge this."
            );
        }

        var state = CurrentCombatState(CurrentCombatManager());
        var typedState = state as CombatState
            ?? throw new InvalidOperationException("Active combat state was not a CombatState");
        var player = typedState.Players.FirstOrDefault()
            ?? throw new InvalidOperationException("No player is available in the active combat");
        var gameAction = CreateLiveAction(typedState, player, action);

        BridgeLog.Info($"live_step enqueueing {gameAction}");
        RunManager.Instance.ActionQueueSynchronizer.RequestEnqueue(gameAction);
        WaitForLiveAction(gameAction, timeoutMilliseconds);
        return ExportStateObject(CurrentCombatState(CurrentCombatManager()));
    }

    public object LiveTraceStep(JsonElement action, bool allowLiveMutation, int timeoutMilliseconds)
    {
        if (!allowLiveMutation)
        {
            throw new InvalidOperationException(
                "/live_trace_step mutates the active combat. Set allow_live_mutation=true to acknowledge this."
            );
        }
        EnsureQuiescentDecisionBoundary();
        var before = ExportSimulatorSnapshotObject();
        var beforeChecksum = CurrentGameChecksum();
        var beforeRng = ExportRngStreams();

        var state = CurrentCombatState(CurrentCombatManager()) as CombatState
            ?? throw new InvalidOperationException("Active combat state was not a CombatState");
        var player = state.Players.FirstOrDefault()
            ?? throw new InvalidOperationException("No player is available in the active combat");
        var gameAction = CreateLiveAction(state, player, action);
        BridgeLog.Info($"live_trace_step enqueueing {gameAction}");
        RunManager.Instance.ActionQueueSynchronizer.RequestEnqueue(gameAction);
        WaitForQuiescence(gameAction, timeoutMilliseconds);

        var after = ExportSimulatorSnapshotObject();
        var afterRng = ExportRngStreams();
        return new Dictionary<string, object?>
        {
            ["before"] = before,
            ["action"] = action.Clone(),
            ["after"] = after,
            ["legal_actions"] = LiveLegalActions(),
            ["rng_deltas"] = RngDeltas(beforeRng, afterRng),
            ["before_game_checksum"] = beforeChecksum,
            ["after_game_checksum"] = CurrentGameChecksum()
        };
    }

    public object SaveLiveCheckpoint(bool allowLiveMutation)
    {
        if (!allowLiveMutation)
        {
            throw new InvalidOperationException(
                "/live_checkpoint captures mutable run state for later live restore. Set allow_live_mutation=true to acknowledge this."
            );
        }
        if (CurrentRunState()?.CurrentRoom is not CombatRoom room)
        {
            throw new InvalidOperationException("A combat room must be active to save a live checkpoint");
        }
        _liveCheckpoint = RunManager.Instance.ToSave(room);
        _liveCheckpointCreatedAt = DateTimeOffset.UtcNow;
        BridgeLog.Info("saved live combat checkpoint");
        return new
        {
            ok = true,
            created_at = _liveCheckpointCreatedAt,
            state = ExportStateObject(CurrentCombatState(CurrentCombatManager()))
        };
    }

    public object RestoreLiveCheckpoint(bool allowLiveMutation, int timeoutMilliseconds)
    {
        if (!allowLiveMutation)
        {
            throw new InvalidOperationException(
                "/live_restore_checkpoint replaces the active run state. Set allow_live_mutation=true to acknowledge this."
            );
        }
        var checkpoint = _liveCheckpoint
            ?? throw new InvalidOperationException("No live checkpoint has been saved");
        var timeout = Math.Clamp(timeoutMilliseconds, 1_000, 120_000);
        BridgeLog.Info("restoring live combat checkpoint");
        var task = RestoreLiveCheckpointAsync(checkpoint);
        var completed = Task.WhenAny(task, Task.Delay(timeout)).GetAwaiter().GetResult();
        if (completed != task)
        {
            throw new TimeoutException($"Timed out restoring live checkpoint after {timeout}ms");
        }
        task.GetAwaiter().GetResult();
        return ExportStateObject(CurrentCombatState(CurrentCombatManager()));
    }

    public string StateHash(JsonElement state)
    {
        var canonical = JsonCanonicalizer.Canonicalize(state);
        var bytes = SHA256.HashData(Encoding.UTF8.GetBytes(canonical));
        return Convert.ToHexString(bytes).ToLowerInvariant();
    }

    private static GameAction CreateLiveAction(CombatState state, Player player, JsonElement action)
    {
        var type = StringProperty(action, "type");
        var id = StringProperty(action, "id");
        if (type == "end_turn" || id == "end_turn")
        {
            return new EndPlayerTurnAction(player, state.RoundNumber);
        }
        if (type != "card")
        {
            throw new InvalidOperationException($"Unsupported live action type: {type ?? "<missing>"}");
        }

        var cardIndex = StringProperty(action, "combat_card_index") ?? CardIndexFromActionId(id);
        if (!uint.TryParse(cardIndex, out var parsedCardIndex))
        {
            throw new InvalidOperationException("Card live action requires numeric combat_card_index");
        }
        var card = NetCombatCardDb.Instance.GetCard(parsedCardIndex);
        var target = LiveTargetFromAction(state, action);
        if (!card.CanPlayTargeting(target))
        {
            throw new InvalidOperationException($"Card {card.Id} cannot be played with the requested target");
        }
        return new PlayCardAction(card, target);
    }

    private static async Task RestoreLiveCheckpointAsync(SerializableRun checkpoint)
    {
        RunManager.Instance.CleanUp(graceful: false);
        var runState = RunState.FromSerializable(checkpoint);
        await RunManager.Instance.SetUpSavedSingleplayer(runState, checkpoint);
        if (NGame.Instance == null)
        {
            throw new InvalidOperationException("NGame.Instance is not available");
        }
        NGame.Instance.ReactionContainer.InitializeNetworking(new NetSingleplayerGameService());
        await NGame.Instance.LoadRun(runState, checkpoint.PreFinishedRoom);
    }

    private static RunState? CurrentRunState()
    {
        return RunManager.Instance.GetType()
            .GetProperty("State", BindingFlags.NonPublic | BindingFlags.Instance)
            ?.GetValue(RunManager.Instance) as RunState;
    }

    private static Creature? LiveTargetFromAction(CombatState state, JsonElement action)
    {
        var combatId = StringProperty(action, "target_combat_id");
        if (uint.TryParse(combatId, out var parsedCombatId))
        {
            return state.GetCreature(parsedCombatId);
        }

        var targetIndex = NullableIntProperty(action, "target_index");
        if (targetIndex.HasValue)
        {
            return state.Enemies.ElementAtOrDefault(targetIndex.Value);
        }

        var id = StringProperty(action, "id");
        if (id != null)
        {
            var parts = id.Split(':');
            if (parts.Length >= 3 && parts[2].StartsWith("enemy_", StringComparison.Ordinal))
            {
                var rawIndex = parts[2]["enemy_".Length..];
                if (int.TryParse(rawIndex, out var parsedIndex))
                {
                    return state.Enemies.ElementAtOrDefault(parsedIndex);
                }
            }
        }
        return null;
    }

    private static string? CardIndexFromActionId(string? id)
    {
        if (string.IsNullOrWhiteSpace(id))
        {
            return null;
        }
        var parts = id.Split(':');
        return parts.Length >= 2 && parts[0] == "play" ? parts[1] : null;
    }

    private static void WaitForLiveAction(GameAction action, int timeoutMilliseconds)
    {
        var timeout = Math.Clamp(timeoutMilliseconds, 1_000, 120_000);
        var completed = Task.WhenAny(action.CompletionTask, Task.Delay(timeout)).GetAwaiter().GetResult();
        if (completed != action.CompletionTask)
        {
            throw new TimeoutException($"Timed out waiting for live action after {timeout}ms");
        }
        action.CompletionTask.GetAwaiter().GetResult();
        if (action.Exception != null)
        {
            throw new InvalidOperationException("Live action failed", action.Exception);
        }
    }

    private static void WaitForQuiescence(GameAction action, int timeoutMilliseconds)
    {
        WaitForLiveAction(action, timeoutMilliseconds);
        var timeout = Math.Clamp(timeoutMilliseconds, 1_000, 120_000);
        var deadline = DateTimeOffset.UtcNow.AddMilliseconds(timeout);
        while (DateTimeOffset.UtcNow < deadline)
        {
            if (RunManager.Instance.ActionQueueSet.IsEmpty
                && (RunManager.Instance.ActionQueueSynchronizer.CombatState.ToString() == "PlayPhase"
                    || TryIsTerminalCombat()))
            {
                return;
            }
            Thread.Sleep(10);
        }
        throw new TimeoutException(
            $"Timed out waiting for the game to reach a quiescent player decision after {timeout}ms"
        );
    }

    private static void WaitForDecisionBoundary(int timeoutMilliseconds)
    {
        var timeout = Math.Clamp(timeoutMilliseconds, 1_000, 120_000);
        var deadline = DateTimeOffset.UtcNow.AddMilliseconds(timeout);
        while (DateTimeOffset.UtcNow < deadline)
        {
            if (RunManager.Instance.ActionQueueSet != null
                && RunManager.Instance.ActionQueueSynchronizer != null
                && RunManager.Instance.ActionQueueSet.IsEmpty
                && RunManager.Instance.ActionQueueSynchronizer.CombatState.ToString() == "PlayPhase")
            {
                return;
            }
            Thread.Sleep(10);
        }
        throw new TimeoutException($"Timed out creating the debug Nibbit combat after {timeout}ms");
    }

    private static bool TryIsTerminalCombat()
    {
        try
        {
            var managerType = typeof(CombatManager);
            var manager = managerType.GetProperty("Instance", BindingFlags.Public | BindingFlags.Static)
                ?.GetValue(null);
            if (manager == null)
            {
                return false;
            }
            var state = CurrentCombatState(manager);
            var enemies = GetEnumerableProperty(state, "Enemies").Where(item => item != null).ToList();
            var player = GetEnumerableProperty(state, "Players").FirstOrDefault();
            return enemies.Count > 0 && enemies.All(IsCreatureDead)
                || player != null && IsCreatureDead(GetProperty(player, "Creature") ?? player);
        }
        catch
        {
            return false;
        }
    }

    private static void EnsureQuiescentDecisionBoundary()
    {
        if (RunManager.Instance.ActionQueueSet == null
            || RunManager.Instance.ActionQueueSynchronizer == null)
        {
            throw new OracleHttpException(409, "No active combat is available");
        }
        if (!RunManager.Instance.ActionQueueSet.IsEmpty
            || RunManager.Instance.ActionQueueSynchronizer.CombatState.ToString() != "PlayPhase")
        {
            throw new OracleHttpException(
                409,
                "Simulator snapshots are available only at a quiescent player decision boundary"
            );
        }
    }

    private object CurrentCombatManager()
    {
        if (_combatManagerType == null)
        {
            throw new InvalidOperationException("Could not find CombatManager type in sts2 assembly");
        }

        var instance = _combatManagerType.GetProperty("Instance", BindingFlags.Public | BindingFlags.Static)
            ?.GetValue(null);
        if (instance == null)
        {
            throw new InvalidOperationException("CombatManager.Instance is null");
        }
        return instance;
    }

    private static object CurrentCombatState(object manager)
    {
        var method = manager.GetType().GetMethod(
            "DebugOnlyGetState",
            BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance
        );
        if (method == null)
        {
            throw new InvalidOperationException("CombatManager.DebugOnlyGetState was not found");
        }
        var state = method.Invoke(manager, Array.Empty<object>());
        if (state == null)
        {
            throw new InvalidOperationException("No active combat state is available");
        }
        return state;
    }

    private Dictionary<string, object?> ExportStateObject(object state)
    {
        var players = GetEnumerableProperty(state, "Players").ToList();
        var player = players.FirstOrDefault();
        var enemies = GetEnumerableProperty(state, "Enemies").ToList();
        var round = GetProperty(state, "RoundNumber");
        var currentSide = GetProperty(state, "CurrentSide")?.ToString();

        var export = new Dictionary<string, object?>
        {
            ["version"] = 1,
            ["combat"] = new Dictionary<string, object?>
            {
                ["won"] = enemies.All(IsCreatureDead),
                ["lost"] = player != null && IsCreatureDead(GetProperty(player, "Creature") ?? player),
                ["turn"] = round,
                ["current_side"] = currentSide
            },
            ["player"] = player == null ? null : ExportPlayer(player),
            ["enemies"] = enemies.Where(enemy => enemy != null).Select(enemy => ExportCreatureLike(enemy!)).ToList()
        };

        if (player != null)
        {
            var combat = GetProperty(player, "PlayerCombatState");
            if (combat != null)
            {
                export["energy"] = GetProperty(combat, "Energy");
                export["stars"] = GetProperty(combat, "Stars");
                export["hand"] = ExportCards(GetProperty(combat, "Hand"));
                export["draw_pile"] = ExportCards(GetProperty(combat, "DrawPile"));
                export["discard_pile"] = ExportCards(GetProperty(combat, "DiscardPile"));
                export["exhaust_pile"] = ExportCards(GetProperty(combat, "ExhaustPile"));
                export["play_pile"] = ExportCards(GetProperty(combat, "PlayPile"));
            }
        }

        return export;
    }

    private Dictionary<string, object?> ExportSimulatorSnapshotObject()
    {
        var state = CurrentCombatState(CurrentCombatManager());
        var runState = CurrentRunState()
            ?? throw new InvalidOperationException("No active run state is available");
        var players = GetEnumerableProperty(state, "Players").ToList();
        var player = players.FirstOrDefault()
            ?? throw new InvalidOperationException("No player is available in the active combat");
        var playerCombat = GetProperty(player, "PlayerCombatState")
            ?? throw new InvalidOperationException("No player combat state is available");
        var creature = GetProperty(player, "Creature") ?? player;
        var enemies = GetEnumerableProperty(state, "Enemies").Where(item => item != null).ToList();
        var provenance = ReadGameProvenance();
        var won = enemies.All(IsCreatureDead);
        var lost = IsCreatureDead(creature);

        return new Dictionary<string, object?>
        {
            ["snapshot_schema"] = 2,
            ["provenance"] = provenance,
            ["rng"] = new Dictionary<string, object?>
            {
                ["algorithm"] = RngAlgorithm(),
                ["run_seed"] = runState.Rng.StringSeed,
                ["streams"] = ExportRngStreams()
            },
            ["combat"] = new Dictionary<string, object?>
            {
                ["won"] = won,
                ["lost"] = lost,
                ["turn"] = GetProperty(state, "RoundNumber"),
                ["current_side"] = GetProperty(state, "CurrentSide")?.ToString(),
                ["ascension_level"] = runState.AscensionLevel
            },
            ["decision"] = new Dictionary<string, object?>
            {
                ["kind"] = won || lost ? "terminal" : "player_action"
            },
            ["player"] = new Dictionary<string, object?>
            {
                ["combat_id"] = GetAnyProperty(creature, "CombatId")?.ToString() ?? "0",
                ["model_id"] = GetAnyProperty(creature, "ModelId", "Id")?.ToString(),
                ["hp"] = GetAnyProperty(creature, "CurrentHp", "Hp"),
                ["max_hp"] = GetAnyProperty(creature, "MaxHp"),
                ["block"] = GetAnyProperty(creature, "Block"),
                ["energy"] = GetProperty(playerCombat, "Energy"),
                ["max_energy"] = GetProperty(player, "MaxEnergy"),
                ["powers"] = ExportSnapshotModels(GetAnyProperty(creature, "Powers"), includeAmount: true),
                ["relics"] = ExportSnapshotModels(GetProperty(player, "Relics"), includeAmount: false),
                ["potions"] = ExportSnapshotModels(GetProperty(player, "Potions"), includeAmount: false)
            },
            ["enemies"] = enemies.Select(enemy => ExportSnapshotEnemy(enemy!, runState.AscensionLevel)).ToList(),
            ["hand"] = ExportSnapshotCards(GetProperty(playerCombat, "Hand")),
            ["draw_pile"] = ExportSnapshotCards(GetProperty(playerCombat, "DrawPile")),
            ["discard_pile"] = ExportSnapshotCards(GetProperty(playerCombat, "DiscardPile")),
            ["exhaust_pile"] = ExportSnapshotCards(GetProperty(playerCombat, "ExhaustPile")),
            ["play_pile"] = ExportSnapshotCards(GetProperty(playerCombat, "PlayPile")),
            ["metrics"] = new Dictionary<string, object?> { ["powers_played"] = 0 }
        };
    }

    private static string RngAlgorithm()
    {
        var implementation = typeof(MegaCrit.Sts2.Core.Random.Rng)
            .GetField("_random", BindingFlags.NonPublic | BindingFlags.Instance)
            ?.FieldType
            .Name ?? "unknown";
        var probe = new MegaCrit.Sts2.Core.Random.Rng(1u);
        var vector = Enumerable.Range(0, 4)
            .Select(_ => probe.NextInt(1_000))
            .ToArray();
        if (implementation == "MegaRandom" && vector.SequenceEqual(new[] { 702, 520, 574, 391 }))
        {
            return "xoshiro256_star_star_v1";
        }
        if (implementation == "Random" && vector.SequenceEqual(new[] { 248, 110, 467, 771 }))
        {
            return "dotnet_system_random_v1";
        }

        var fingerprintInput = $"{implementation}:{string.Join(",", vector)}";
        var fingerprint = Convert.ToHexString(
            SHA256.HashData(Encoding.UTF8.GetBytes(fingerprintInput))
        ).ToLowerInvariant()[..12];
        return $"unverified_rng_{fingerprint}";
    }

    private Dictionary<string, object?> ReadGameProvenance()
    {
        var assemblyPath = _gameAssembly.Location;
        var assemblyHash = Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(assemblyPath))).ToLowerInvariant();
        var releasePath = Path.GetFullPath(Path.Combine(Path.GetDirectoryName(assemblyPath)!, "..", "release_info.json"));
        string version = "unknown";
        string commit = "unknown";
        if (File.Exists(releasePath))
        {
            using var document = JsonDocument.Parse(File.ReadAllText(releasePath));
            if (document.RootElement.TryGetProperty("version", out var rawVersion))
            {
                version = rawVersion.GetString() ?? version;
            }
            if (document.RootElement.TryGetProperty("commit", out var rawCommit))
            {
                commit = rawCommit.GetString() ?? commit;
            }
        }
        return new Dictionary<string, object?>
        {
            ["game_version"] = version,
            ["game_commit"] = commit,
            ["assembly_sha256"] = assemblyHash,
            ["content_revision"] = "base",
            ["modded_gameplay"] = HasExternalGameplayMods(),
            ["bridge_version"] = ModEntry.BridgeVersion
        };
    }

    private static bool HasExternalGameplayMods()
    {
        foreach (var mod in ModManager.GetLoadedMods())
        {
            var manifest = GetProperty(mod, "manifest") ?? GetProperty(mod, "Manifest");
            var id = manifest == null ? null : GetAnyProperty(manifest, "id", "Id")?.ToString();
            if (!string.IsNullOrEmpty(id) && id != "sls2_combat_oracle")
            {
                return true;
            }
        }
        return false;
    }

    private Dictionary<string, object?> ExportRngStreams()
    {
        var runState = CurrentRunState()
            ?? throw new InvalidOperationException("No active run state is available");
        var rngs = runState.Rng.GetType()
            .GetField("_rngs", BindingFlags.NonPublic | BindingFlags.Instance)
            ?.GetValue(runState.Rng);
        var result = new Dictionary<string, object?>();
        foreach (var entry in EnumerateObject(rngs))
        {
            if (entry == null)
            {
                continue;
            }
            var key = GetProperty(entry, "Key")?.ToString();
            var value = GetProperty(entry, "Value");
            if (key == null || value == null)
            {
                continue;
            }
            result[SnakeCase(key)] = new Dictionary<string, object?>
            {
                ["seed"] = GetProperty(value, "Seed"),
                ["counter"] = GetProperty(value, "Counter")
            };
        }
        return result;
    }

    private Dictionary<string, object?> ExportSnapshotEnemy(object enemy, int ascensionLevel)
    {
        var monster = GetProperty(enemy, "Monster");
        var machine = monster == null ? null : GetProperty(monster, "MoveStateMachine");
        var nextMove = monster == null ? null : GetProperty(monster, "NextMove");
        var currentMove = nextMove == null ? null : GetProperty(nextMove, "Id")?.ToString();
        List<string?> history = machine == null
            ? new List<string?>()
            : GetEnumerableProperty(machine, "StateLog")
                .Where(item => item != null)
                .Select(item => GetProperty(item!, "Id")?.ToString())
                .Where(item => item != null)
                .ToList();
        return new Dictionary<string, object?>
        {
            ["combat_id"] = GetAnyProperty(enemy, "CombatId")?.ToString(),
            ["model_id"] = GetAnyProperty(enemy, "ModelId", "Id")?.ToString(),
            ["hp"] = GetAnyProperty(enemy, "CurrentHp", "Hp"),
            ["max_hp"] = GetAnyProperty(enemy, "MaxHp"),
            ["block"] = GetAnyProperty(enemy, "Block"),
            ["powers"] = ExportSnapshotModels(GetAnyProperty(enemy, "Powers"), includeAmount: true),
            ["ai"] = new Dictionary<string, object?>
            {
                ["current_move"] = currentMove,
                ["move_history"] = history,
                ["is_front"] = monster == null ? false : GetProperty(monster, "IsFront") ?? false,
                ["is_alone"] = monster == null ? false : GetProperty(monster, "IsAlone") ?? false,
                ["tough_enemies"] = ascensionLevel >= 8,
                ["deadly_enemies"] = ascensionLevel >= 9
            }
        };
    }

    private List<object?> ExportSnapshotCards(object? pile)
    {
        return EnumerateObject(pile).Select(card =>
        {
            if (card == null)
            {
                return null;
            }
            var modelId = GetAnyProperty(card, "ModelId", "Id")?.ToString();
            return (object?)new Dictionary<string, object?>
            {
                ["instance_id"] = TryGetCombatCardIndex(card)
                    ?? GetAnyProperty(card, "CombatCardIndex", "Index")?.ToString(),
                ["model_id"] = modelId,
                ["upgrade_level"] = GetAnyProperty(card, "CurrentUpgradeLevel", "UpgradeLevel") ?? 0,
                ["cost"] = GetEnergyCost(card) ?? GetAnyProperty(card, "Cost", "CurrentCost") ?? 0,
                ["cost_for_turn"] = GetEnergyCost(card),
                ["retained"] = GetAnyProperty(card, "IsRetained", "Retained") ?? false,
                ["exhausts"] = GetAnyProperty(card, "Exhausts", "ExhaustOnPlay") ?? false,
                ["ethereal"] = modelId == "CARD.ASCENDERS_BANE"
            };
        }).ToList();
    }

    private static List<object?> ExportSnapshotModels(object? value, bool includeAmount)
    {
        return EnumerateObject(value).Select(item =>
        {
            if (item == null)
            {
                return null;
            }
            var model = new Dictionary<string, object?>
            {
                ["model_id"] = GetAnyProperty(item, "ModelId", "Id")?.ToString()
            };
            if (includeAmount)
            {
                model["amount"] = GetAnyProperty(item, "Amount", "Stacks") ?? 0;
            }
            return (object?)model;
        }).ToList();
    }

    private static Dictionary<string, int> RngDeltas(
        Dictionary<string, object?> before,
        Dictionary<string, object?> after
    )
    {
        var result = new Dictionary<string, int>();
        foreach (var (key, afterValue) in after)
        {
            if (afterValue is not Dictionary<string, object?> afterState)
            {
                continue;
            }
            var afterCounter = Convert.ToInt32(afterState["counter"]);
            var beforeCounter = before.TryGetValue(key, out var beforeValue)
                && beforeValue is Dictionary<string, object?> beforeState
                ? Convert.ToInt32(beforeState["counter"])
                : 0;
            result[key] = afterCounter - beforeCounter;
        }
        return result;
    }

    private static uint CurrentGameChecksum()
    {
        var runState = CurrentRunState()
            ?? throw new InvalidOperationException("No active run state is available");
        var fullState = NetFullCombatState.FromRun(runState, null);
        return RunManager.Instance.ChecksumTracker.GenerateChecksum(fullState);
    }

    private static string SnakeCase(string value)
    {
        var builder = new StringBuilder();
        for (var index = 0; index < value.Length; index++)
        {
            var character = value[index];
            if (char.IsUpper(character) && index > 0)
            {
                builder.Append('_');
            }
            builder.Append(char.ToLowerInvariant(character));
        }
        return builder.ToString();
    }

    private static Dictionary<string, object?> ExportPlayer(object player)
    {
        var creature = GetProperty(player, "Creature") ?? player;
        var data = ExportCreatureLike(creature);
        data["net_id"] = GetProperty(player, "NetId");
        data["gold"] = GetProperty(player, "Gold");
        data["max_energy"] = GetProperty(player, "MaxEnergy");
        data["relics"] = ExportEnumerableModels(GetProperty(player, "Relics"));
        data["potions"] = ExportEnumerableModels(GetProperty(player, "Potions"));
        return data;
    }

    private static Dictionary<string, object?> ExportCreatureLike(object creature)
    {
        return new Dictionary<string, object?>
        {
            ["type"] = creature.GetType().FullName,
            ["id"] = GetAnyProperty(creature, "Id", "NetId", "ModelId")?.ToString(),
            ["model_id"] = GetAnyProperty(creature, "ModelId", "Id")?.ToString(),
            ["combat_id"] = GetAnyProperty(creature, "CombatId")?.ToString(),
            ["hp"] = GetAnyProperty(creature, "CurrentHp", "Hp", "Health"),
            ["max_hp"] = GetAnyProperty(creature, "MaxHp", "MaxHealth"),
            ["block"] = GetAnyProperty(creature, "Block", "CurrentBlock"),
            ["is_dead"] = IsCreatureDead(creature),
            ["powers"] = ExportEnumerableModels(GetAnyProperty(creature, "Powers"))
        };
    }

    private List<object?> ExportCards(object? pile)
    {
        if (pile == null)
        {
            return [];
        }
        return EnumerateObject(pile).Select(ExportCard).Cast<object?>().ToList();
    }

    private Dictionary<string, object?> ExportCard(object? card, int zoneIndex)
    {
        if (card == null)
        {
            return new Dictionary<string, object?>();
        }
        return new Dictionary<string, object?>
        {
            ["type"] = card.GetType().FullName,
            ["id"] = GetAnyProperty(card, "Id", "ModelId")?.ToString(),
            ["model_id"] = GetAnyProperty(card, "ModelId", "Id")?.ToString(),
            ["combat_card_index"] = TryGetCombatCardIndex(card)
                ?? GetAnyProperty(card, "CombatCardIndex", "Index")?.ToString(),
            ["zone_index"] = zoneIndex,
            ["upgrade_level"] = GetAnyProperty(card, "CurrentUpgradeLevel", "UpgradeLevel"),
            ["target_type"] = GetAnyProperty(card, "TargetType")?.ToString(),
            ["cost"] = GetEnergyCost(card) ?? GetAnyProperty(card, "Cost", "CurrentCost"),
            ["name"] = GetAnyProperty(card, "Name", "DisplayName")?.ToString()
        };
    }

    private string? TryGetCombatCardIndex(object card)
    {
        var dbType = _gameAssembly.GetType("MegaCrit.Sts2.Core.GameActions.Multiplayer.NetCombatCardDb");
        var db = dbType?.GetProperty("Instance", BindingFlags.Public | BindingFlags.Static)?.GetValue(null);
        var method = dbType?.GetMethod("TryGetCardId", BindingFlags.Public | BindingFlags.Instance);
        if (db == null || method == null)
        {
            return null;
        }

        object?[] parameters = [card, null];
        var success = method.Invoke(db, parameters);
        if (success is true && parameters[1] != null)
        {
            return parameters[1]?.ToString();
        }
        return null;
    }

    private static bool CanPlayTargeting(object card, object? target)
    {
        var method = card.GetType().GetMethod(
            "CanPlayTargeting",
            BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance
        );
        if (method == null)
        {
            return false;
        }
        return method.Invoke(card, [target]) is true;
    }

    private static int? GetEnergyCost(object card)
    {
        var energyCost = GetProperty(card, "EnergyCost");
        if (energyCost == null)
        {
            return null;
        }
        var method = energyCost.GetType().GetMethod(
            "GetWithModifiers",
            BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance
        );
        if (method == null)
        {
            return null;
        }
        var enumType = method.GetParameters().FirstOrDefault()?.ParameterType;
        if (enumType == null)
        {
            return null;
        }
        var all = Enum.Parse(enumType, "All");
        var value = method.Invoke(energyCost, [all]);
        return value == null ? null : Convert.ToInt32(value);
    }

    private static object CardAction(
        string cardIndex,
        int handIndex,
        string? cardId,
        string? targetType,
        int? energyCost,
        TargetRef? target
    )
    {
        return new
        {
            id = target == null ? $"play:{cardIndex}" : $"play:{cardIndex}:{target.ActionId}",
            type = "card",
            card_id = cardId,
            combat_card_index = cardIndex,
            hand_index = handIndex,
            target_type = targetType,
            target = target?.ActionId,
            target_id = target?.Id,
            target_index = target?.Index,
            target_combat_id = target?.CombatId,
            cost = energyCost
        };
    }

    private static string CardActionIndex(JsonElement card, int handIndex)
    {
        var combatIndex = StringProperty(card, "combat_card_index");
        return string.IsNullOrEmpty(combatIndex) ? $"hand_{handIndex}" : combatIndex;
    }

    private static IEnumerable<TargetRef> LiveEnemyTargets(JsonElement state)
    {
        if (!state.TryGetProperty("enemies", out var enemies) || enemies.ValueKind != JsonValueKind.Array)
        {
            yield break;
        }

        var index = 0;
        foreach (var enemy in enemies.EnumerateArray())
        {
            if (BoolProperty(enemy, "is_dead") || IntProperty(enemy, "hp") <= 0)
            {
                index++;
                continue;
            }
            var id = StringProperty(enemy, "id") ?? $"enemy_{index}";
            yield return new TargetRef($"enemy_{index}", id, index);
            index++;
        }
    }

    private static string? StringProperty(JsonElement element, string name)
    {
        if (!element.TryGetProperty(name, out var property) || property.ValueKind == JsonValueKind.Null)
        {
            return null;
        }
        return property.ValueKind == JsonValueKind.String ? property.GetString() : property.ToString();
    }

    private static int IntProperty(JsonElement element, string name)
    {
        return NullableIntProperty(element, name) ?? 0;
    }

    private static int? NullableIntProperty(JsonElement element, string name)
    {
        if (!element.TryGetProperty(name, out var property))
        {
            return null;
        }
        if (property.ValueKind == JsonValueKind.Number && property.TryGetInt32(out var value))
        {
            return value;
        }
        return int.TryParse(property.ToString(), out var parsed) ? parsed : null;
    }

    private static bool BoolProperty(JsonElement element, string name)
    {
        if (!element.TryGetProperty(name, out var property))
        {
            return false;
        }
        return property.ValueKind switch
        {
            JsonValueKind.True => true,
            JsonValueKind.False => false,
            JsonValueKind.String => bool.TryParse(property.GetString(), out var value) && value,
            _ => false
        };
    }

    private sealed record TargetRef(
        string ActionId,
        string Id,
        int Index,
        string? CombatId = null,
        object? LiveCreature = null
    );

    private static List<object?> ExportEnumerableModels(object? value)
    {
        if (value == null)
        {
            return [];
        }
        return EnumerateObject(value)
            .Select(item => item == null ? null : new Dictionary<string, object?>
            {
                ["type"] = item.GetType().FullName,
                ["id"] = GetAnyProperty(item, "Id", "ModelId")?.ToString(),
                ["model_id"] = GetAnyProperty(item, "ModelId", "Id")?.ToString(),
                ["amount"] = GetAnyProperty(item, "Amount", "Stacks")
            })
            .Cast<object?>()
            .ToList();
    }

    private static bool IsCreatureDead(object? creature)
    {
        if (creature == null)
        {
            return false;
        }
        var explicitDead = GetAnyProperty(creature, "IsDead", "Dead");
        if (explicitDead is bool dead)
        {
            return dead;
        }
        var hp = GetAnyProperty(creature, "CurrentHp", "Hp", "Health");
        return hp is int i && i <= 0;
    }

    private static IEnumerable<object?> GetEnumerableProperty(object obj, string name)
    {
        return EnumerateObject(GetProperty(obj, name));
    }

    private static object? GetAnyProperty(object obj, params string[] names)
    {
        foreach (var name in names)
        {
            var value = GetProperty(obj, name);
            if (value != null)
            {
                return value;
            }
        }
        return null;
    }

    private static object? GetProperty(object obj, string name)
    {
        var property = obj.GetType()
            .GetProperty(name, BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance)
            ?.GetValue(obj);
        if (property != null)
        {
            return property;
        }
        return obj.GetType()
            .GetField(name, BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance)
            ?.GetValue(obj);
    }

    private static IEnumerable<object?> EnumerateObject(object? value)
    {
        if (value == null)
        {
            yield break;
        }

        if (value is IEnumerable enumerable and not string)
        {
            foreach (var item in enumerable)
            {
                yield return item;
            }
            yield break;
        }

        var cardsProp = value.GetType()
            .GetProperty("Cards", BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance);
        if (cardsProp?.GetValue(value) is IEnumerable cards)
        {
            foreach (var item in cards)
            {
                yield return item;
            }
        }
    }
}
