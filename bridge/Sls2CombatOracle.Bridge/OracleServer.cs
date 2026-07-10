using System.Net;
using System.Text;
using System.Text.Json;

namespace Sls2CombatOracle.Bridge;

internal sealed class OracleServer
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        WriteIndented = true
    };

    private readonly HttpListener _listener;
    private readonly ICombatBridge _bridge;
    private readonly CancellationTokenSource _cts = new();

    public OracleServer(int port, ICombatBridge bridge)
    {
        _bridge = bridge;
        _listener = new HttpListener();
        _listener.Prefixes.Add($"http://127.0.0.1:{port}/");
    }

    public void Start()
    {
        _listener.Start();
        _ = Task.Run(ListenLoop);
    }

    private async Task ListenLoop()
    {
        while (!_cts.IsCancellationRequested)
        {
            try
            {
                var context = await _listener.GetContextAsync();
                _ = Task.Run(() => Handle(context));
            }
            catch (ObjectDisposedException)
            {
                return;
            }
            catch (HttpListenerException)
            {
                return;
            }
            catch (Exception error)
            {
                Console.WriteLine($"[Sls2CombatOracle] listener error: {error}");
            }
        }
    }

    private async Task Handle(HttpListenerContext context)
    {
        try
        {
            var path = context.Request.Url?.AbsolutePath ?? "/";
            var request = await ReadRequest(context.Request);
            object response = path switch
            {
                "/health" => new
                {
                    ok = true,
                    bridge = ModEntry.BridgeId,
                    version = ModEntry.BridgeVersion,
                    capabilities = new
                    {
                        export_state = true,
                        legal_actions = true,
                        state_hash = true,
                        live_step = true,
                        live_checkpoint = true,
                        live_restore_checkpoint = true,
                        branchable_step = false
                    }
                },
                "/export_state" => _bridge.ExportState(),
                "/legal_actions" => new { actions = _bridge.LegalActions(GetState(request)) },
                "/step" => new { state = _bridge.Step(GetState(request), GetAction(request)) },
                "/live_step" => new
                {
                    state = _bridge.LiveStep(
                        GetAction(request),
                        BoolProperty(request, "allow_live_mutation"),
                        IntProperty(request, "timeout_milliseconds") ?? 30_000
                    )
                },
                "/live_checkpoint" => _bridge.SaveLiveCheckpoint(BoolProperty(request, "allow_live_mutation")),
                "/live_restore_checkpoint" => new
                {
                    state = _bridge.RestoreLiveCheckpoint(
                        BoolProperty(request, "allow_live_mutation"),
                        IntProperty(request, "timeout_milliseconds") ?? 30_000
                    )
                },
                "/state_hash" => new { state_hash = _bridge.StateHash(GetState(request)) },
                _ => throw new OracleHttpException(404, $"Unknown endpoint: {path}")
            };
            await WriteJson(context.Response, 200, response);
        }
        catch (OracleHttpException error)
        {
            await WriteJson(context.Response, error.StatusCode, new { error = error.Message });
        }
        catch (NotSupportedException error)
        {
            await WriteJson(context.Response, 501, new { error = error.Message });
        }
        catch (Exception error)
        {
            BridgeLog.Error("request failed", error);
            await WriteJson(context.Response, 500, new { error = error.Message });
        }
    }

    private static async Task<JsonElement> ReadRequest(HttpListenerRequest request)
    {
        if (!request.HasEntityBody)
        {
            return JsonDocument.Parse("{}").RootElement.Clone();
        }

        using var reader = new StreamReader(request.InputStream, request.ContentEncoding);
        var body = await reader.ReadToEndAsync();
        if (string.IsNullOrWhiteSpace(body))
        {
            return JsonDocument.Parse("{}").RootElement.Clone();
        }

        return JsonDocument.Parse(body).RootElement.Clone();
    }

    private static JsonElement GetState(JsonElement request)
    {
        if (!request.TryGetProperty("state", out var state))
        {
            throw new OracleHttpException(400, "Request must include state");
        }
        return state.Clone();
    }

    private static JsonElement GetAction(JsonElement request)
    {
        if (!request.TryGetProperty("action", out var action))
        {
            throw new OracleHttpException(400, "Request must include action");
        }
        return action.Clone();
    }

    private static bool BoolProperty(JsonElement request, string name)
    {
        if (!request.TryGetProperty(name, out var value))
        {
            return false;
        }
        return value.ValueKind switch
        {
            JsonValueKind.True => true,
            JsonValueKind.False => false,
            JsonValueKind.String => bool.TryParse(value.GetString(), out var parsed) && parsed,
            _ => false
        };
    }

    private static int? IntProperty(JsonElement request, string name)
    {
        if (!request.TryGetProperty(name, out var value))
        {
            return null;
        }
        if (value.ValueKind == JsonValueKind.Number && value.TryGetInt32(out var parsed))
        {
            return parsed;
        }
        return int.TryParse(value.ToString(), out parsed) ? parsed : null;
    }

    private static async Task WriteJson(HttpListenerResponse response, int status, object payload)
    {
        var json = JsonSerializer.Serialize(payload, JsonOptions);
        var bytes = Encoding.UTF8.GetBytes(json);
        response.StatusCode = status;
        response.ContentType = "application/json; charset=utf-8";
        response.ContentLength64 = bytes.Length;
        await response.OutputStream.WriteAsync(bytes);
        response.Close();
    }
}

internal sealed class OracleHttpException : Exception
{
    public int StatusCode { get; }

    public OracleHttpException(int statusCode, string message) : base(message)
    {
        StatusCode = statusCode;
    }
}
