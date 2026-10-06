using System.Text.Json;
using System.Text.Json.Nodes;
using V2RaySybau.Models;

namespace V2RaySybau.Core;

/// <summary>
/// Converts a connection profile into a V2Ray/Xray client configuration that can be used by the core executable.
/// </summary>
public sealed class XrayConfigAdapter
{
    public string Build(ConnectionProfile profile, int localPort = 10808)
    {
        ArgumentNullException.ThrowIfNull(profile);

        var userId = string.IsNullOrWhiteSpace(profile.UserId) ? Guid.Empty.ToString() : profile.UserId;
        var protocol = string.IsNullOrWhiteSpace(profile.Protocol) ? "vless" : profile.Protocol.Trim().ToLowerInvariant();
        var transport = string.IsNullOrWhiteSpace(profile.Transport) ? "tcp" : profile.Transport.Trim().ToLowerInvariant();
        var security = string.IsNullOrWhiteSpace(profile.Security) ? "none" : profile.Security.Trim().ToLowerInvariant();

        var outboundSettings = new JsonObject
        {
            ["vnext"] = new JsonArray
            {
                new JsonObject
                {
                    ["address"] = profile.Host,
                    ["port"] = profile.Port,
                    ["users"] = new JsonArray
                    {
                        new JsonObject
                        {
                            ["id"] = userId,
                            ["encryption"] = protocol == "vless" ? "none" : (security == "none" ? "auto" : security)
                        }
                    }
                }
            }
        };

        var streamSettings = new JsonObject
        {
            ["network"] = transport,
            ["security"] = security == "tls" ? "tls" : "none"
        };

        if (profile.Parameters.TryGetValue("path", out var path) && !string.IsNullOrWhiteSpace(path))
            streamSettings["path"] = path;

        if (profile.Parameters.TryGetValue("host", out var hostHeader) && !string.IsNullOrWhiteSpace(hostHeader))
            streamSettings["headers"] = new JsonObject { ["Host"] = hostHeader };

        if (profile.Parameters.TryGetValue("sni", out var sni) && !string.IsNullOrWhiteSpace(sni))
            streamSettings["sni"] = sni;

        if (profile.Parameters.TryGetValue("serviceName", out var serviceName) && !string.IsNullOrWhiteSpace(serviceName))
            streamSettings["serviceName"] = serviceName;

        if (transport == "ws" && profile.Parameters.TryGetValue("path", out var wsPath) && !string.IsNullOrWhiteSpace(wsPath))
            streamSettings["path"] = wsPath;

        if (transport == "httpupgrade" && profile.Parameters.TryGetValue("host", out var upgradeHost) && !string.IsNullOrWhiteSpace(upgradeHost))
            streamSettings["host"] = upgradeHost;

        var outbound = new JsonObject
        {
            ["protocol"] = protocol,
            ["settings"] = outboundSettings,
            ["streamSettings"] = streamSettings,
            ["mux"] = new JsonObject
            {
                ["enabled"] = true,
                ["concurrency"] = 8
            }
        };

        var config = new JsonObject
        {
            ["log"] = new JsonObject
            {
                ["loglevel"] = "warning"
            },
            ["inbounds"] = new JsonArray
            {
                new JsonObject
                {
                    ["listen"] = "127.0.0.1",
                    ["port"] = localPort,
                    ["protocol"] = "socks",
                    ["settings"] = new JsonObject
                    {
                        ["auth"] = "noauth",
                        ["udp"] = true,
                        ["ip"] = "127.0.0.1"
                    },
                    ["sniffing"] = new JsonObject
                    {
                        ["enabled"] = true,
                        ["destOverride"] = new JsonArray { "http", "tls" }
                    }
                }
            },
            ["outbounds"] = new JsonArray { outbound },
            ["routing"] = new JsonObject
            {
                ["domainStrategy"] = "AsIs",
                ["rules"] = new JsonArray
                {
                    new JsonObject
                    {
                        ["type"] = "field",
                        ["network"] = "tcp,udp",
                        ["outboundTag"] = "direct"
                    }
                }
            }
        };

        return config.ToJsonString(new JsonSerializerOptions { WriteIndented = true });
    }
}
