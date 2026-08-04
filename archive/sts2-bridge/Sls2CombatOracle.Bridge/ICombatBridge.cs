using System.Text.Json;

namespace Sls2CombatOracle.Bridge;

internal interface ICombatBridge
{
    object ExportState();
    object ExportSimulatorSnapshot();
    object RngVector(uint seed, int count, int maxExclusive);
    object StartDebugNibbit(bool allowLiveMutation, int timeoutMilliseconds);
    IReadOnlyList<object> LegalActions(JsonElement state);
    object Step(JsonElement state, JsonElement action);
    object LiveStep(JsonElement action, bool allowLiveMutation, int timeoutMilliseconds);
    object LiveTraceStep(JsonElement action, bool allowLiveMutation, int timeoutMilliseconds);
    object SaveLiveCheckpoint(bool allowLiveMutation);
    object RestoreLiveCheckpoint(bool allowLiveMutation, int timeoutMilliseconds);
    string StateHash(JsonElement state);
}
