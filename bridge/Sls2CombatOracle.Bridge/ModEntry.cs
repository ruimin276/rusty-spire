using System.Reflection;
using MegaCrit.Sts2.Core.Modding;

namespace Sls2CombatOracle.Bridge;

[ModInitializer("Initialize")]
public static class ModEntry
{
    public const string BridgeId = "sls2-combat-oracle";
    public const string BridgeVersion = "0.1.3";

    private static OracleServer? _server;

    public static void Initialize()
    {
        BridgeLog.Info($"initializer called for {BridgeId} {BridgeVersion}");
        if (_server != null)
        {
            BridgeLog.Info("server already initialized");
            return;
        }

        var port = ReadPort();
        try
        {
            _server = new OracleServer(
                port,
                new ReflectionCombatBridge(Assembly.Load("sts2"))
            );
            _server.Start();
            BridgeLog.Info($"HTTP oracle listening on http://127.0.0.1:{port}");
        }
        catch (Exception error)
        {
            BridgeLog.Error("failed to start HTTP oracle", error);
            throw;
        }
    }

    private static int ReadPort()
    {
        var raw = Environment.GetEnvironmentVariable("SLS2_ORACLE_PORT");
        return int.TryParse(raw, out var port) && port > 0 ? port : 17351;
    }
}
