namespace Sls2CombatOracle.Bridge;

internal static class BridgeLog
{
    private static readonly object Lock = new();

    private static readonly string LogPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
        "Library",
        "Logs",
        "sls2-combat-oracle.log"
    );

    public static void Info(string message)
    {
        Write("INFO", message);
    }

    public static void Error(string message, Exception error)
    {
        Write("ERROR", $"{message}: {error}");
    }

    private static void Write(string level, string message)
    {
        var line = $"{DateTimeOffset.Now:O} [{level}] {message}";
        Console.WriteLine($"[Sls2CombatOracle] {message}");

        lock (Lock)
        {
            try
            {
                Directory.CreateDirectory(Path.GetDirectoryName(LogPath)!);
                File.AppendAllText(LogPath, line + Environment.NewLine);
            }
            catch
            {
                // Console logging is still useful if the user log path is unavailable.
            }
        }
    }
}
