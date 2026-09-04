using System.Text.Json.Nodes;
using V2RaySybau.Models;
namespace V2RaySybau.Core;
/// <summary>Converts app profiles to an Xray/V2Ray compatible client configuration.</summary>
public sealed class XrayConfigAdapter
{
    public string Build(ConnectionProfile profile, int localPort = 10808)
    {
        var users = new JsonArray { new JsonObject { ["id"] = profile.UserId, ["encryption"] = profile.Protocol == "vless" ? "none" : profile.Security } };
        var outbound = new JsonObject { ["protocol"] = profile.Protocol, ["settings"] = new JsonObject { ["vnext"] = new JsonArray { new JsonObject { ["address"] = profile.Host, ["port"] = profile.Port, ["users"] = users } } }, ["streamSettings"] = new JsonObject { ["network"] = profile.Transport, ["security"] = profile.Parameters.GetValueOrDefault("security", "none") } };
        var config = new JsonObject { ["log"] = new JsonObject { ["loglevel"] = "warning" }, ["inbounds"] = new JsonArray { new JsonObject { ["listen"] = "127.0.0.1", ["port"] = localPort, ["protocol"] = "socks", ["settings"] = new JsonObject { ["udp"] = true } } }, ["outbounds"] = new JsonArray { outbound, new JsonObject { ["protocol"] = "freedom", ["tag"] = "direct" } } };
        return config.ToJsonString(new System.Text.Json.JsonSerializerOptions { WriteIndented = true });
    }
}
