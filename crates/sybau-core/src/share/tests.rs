use super::*;
use crate::model::{Network, Protocol, Security};
use crate::util::b64_encode;

#[test]
fn parses_vless_reality_vision() {
    let link = "vless://3f1c7a52-9d0e-4b8a-8c1f-2a7d6e5b4c3a@203.0.113.10:443?encryption=none&flow=xtls-rprx-vision&security=reality&sni=www.microsoft.com&fp=chrome&pbk=Z84J2IelR9ch3k8VtlVhhs5ycBUlXA7wHBWcBrjqnAw&sid=6ba85179e30d4fc2&spx=%2F&type=tcp&headerType=none#%F0%9F%87%B3%F0%9F%87%B1%20NL%20Reality";
    let p = parse_link(link).unwrap();
    assert_eq!(p.protocol, Protocol::Vless);
    assert_eq!(p.uuid, "3f1c7a52-9d0e-4b8a-8c1f-2a7d6e5b4c3a");
    assert_eq!(p.address, "203.0.113.10");
    assert_eq!(p.port, 443);
    assert_eq!(p.flow, "xtls-rprx-vision");
    assert_eq!(p.tls.security, Security::Reality);
    assert_eq!(p.tls.sni, "www.microsoft.com");
    assert_eq!(p.tls.public_key, "Z84J2IelR9ch3k8VtlVhhs5ycBUlXA7wHBWcBrjqnAw");
    assert_eq!(p.tls.short_id, "6ba85179e30d4fc2");
    assert_eq!(p.tls.spider_x, "/");
    assert_eq!(p.name, "🇳🇱 NL Reality");
}

#[test]
fn parses_vless_ws_tls_with_unescaped_name_and_ipv6() {
    let link = "vless://id@[2001:db8::1]:8443?type=ws&security=tls&path=%2Fws%3Fed%3D2048&host=cdn.example.com&sni=cdn.example.com&alpn=h2,http/1.1#My server | 1";
    let p = parse_link(link).unwrap();
    assert_eq!(p.address, "2001:db8::1");
    assert_eq!(p.port, 8443);
    assert_eq!(p.transport.network, Network::Ws);
    assert_eq!(p.transport.path, "/ws?ed=2048");
    assert_eq!(p.transport.host, "cdn.example.com");
    assert_eq!(p.tls.alpn, vec!["h2", "http/1.1"]);
    assert_eq!(p.name, "My server | 1");
}

#[test]
fn parses_vless_grpc_and_xhttp() {
    let grpc = parse_link("vless://id@h.com:443?type=grpc&serviceName=gsvc&mode=multi&security=tls#g").unwrap();
    assert_eq!(grpc.transport.network, Network::Grpc);
    assert_eq!(grpc.transport.service_name, "gsvc");
    assert_eq!(grpc.transport.mode, "multi");
    let xhttp = parse_link("vless://id@h.com:443?type=xhttp&path=%2Fx&mode=packet-up&security=reality&pbk=k#x").unwrap();
    assert_eq!(xhttp.transport.network, Network::Xhttp);
    assert_eq!(xhttp.transport.mode, "packet-up");
}

#[test]
fn parses_vmess_json_with_numeric_fields() {
    let json = r#"{"v":"2","ps":"VMess WS","add":"vm.example.com","port":443,"id":"b831381d-6324-4d53-ad4f-8cda48b30811","aid":0,"scy":"auto","net":"ws","type":"none","host":"vm.example.com","path":"/vm","tls":"tls","sni":"vm.example.com","fp":"chrome"}"#;
    let p = parse_link(&format!("vmess://{}", b64_encode(json.as_bytes()))).unwrap();
    assert_eq!(p.protocol, Protocol::Vmess);
    assert_eq!(p.name, "VMess WS");
    assert_eq!(p.port, 443);
    assert_eq!(p.transport.network, Network::Ws);
    assert_eq!(p.transport.header_type, "");
    assert_eq!(p.tls.security, Security::Tls);
    assert_eq!(p.tls.fingerprint, "chrome");
}

#[test]
fn parses_trojan_defaults_to_tls() {
    let p = parse_link("trojan://p%40ss/word@tr.example.com:443?sni=tr.example.com#Trojan").unwrap();
    assert_eq!(p.protocol, Protocol::Trojan);
    assert_eq!(p.password, "p@ss/word");
    assert_eq!(p.tls.security, Security::Tls);
    assert_eq!(p.transport.network, Network::Tcp);
}

#[test]
fn parses_shadowsocks_variants() {
    let sip002 = parse_link("ss://YWVzLTI1Ni1nY206cGFzcw@ss.example.com:8388#SS").unwrap();
    assert_eq!(sip002.method, "aes-256-gcm");
    assert_eq!(sip002.password, "pass");
    assert_eq!(sip002.port, 8388);

    let plain = parse_link("ss://2022-blake3-aes-128-gcm:a2V5%3D@1.2.3.4:443#ss2022").unwrap();
    assert_eq!(plain.method, "2022-blake3-aes-128-gcm");
    assert_eq!(plain.password, "a2V5=");

    let legacy = format!("ss://{}#old", b64_encode(b"chacha20-ietf-poly1305:pw@5.6.7.8:1234"));
    let legacy = parse_link(&legacy).unwrap();
    assert_eq!(legacy.address, "5.6.7.8");
    assert_eq!(legacy.port, 1234);
    assert_eq!(legacy.password, "pw");
}

#[test]
fn parses_hysteria2_and_tuic() {
    let hy =
        parse_link("hy2://secret@hy.example.com:443/?sni=hy.example.com&obfs=salamander&obfs-password=ob&insecure=1&mport=20000-30000#HY2")
            .unwrap();
    assert_eq!(hy.protocol, Protocol::Hysteria2);
    assert_eq!(hy.password, "secret");
    assert_eq!(hy.quic.obfs, "salamander");
    assert_eq!(hy.quic.hop_ports, "20000-30000");
    assert!(hy.tls.allow_insecure);

    let tuic = parse_link("tuic://2dd9ab4f-0b34-4b84-a4a0-1df2f5e5f4c6:pw@tu.example.com:443?congestion_control=bbr&alpn=h3&sni=tu.example.com&udp_relay_mode=native#TUIC").unwrap();
    assert_eq!(tuic.protocol, Protocol::Tuic);
    assert_eq!(tuic.uuid, "2dd9ab4f-0b34-4b84-a4a0-1df2f5e5f4c6");
    assert_eq!(tuic.password, "pw");
    assert_eq!(tuic.quic.congestion_control, "bbr");
    assert_eq!(tuic.tls.alpn, vec!["h3"]);
}

#[test]
fn rejects_garbage() {
    assert_eq!(parse_link("http://x"), Err(ParseError::UnsupportedScheme));
    assert!(parse_link("vless://@:443").is_err());
    assert!(parse_link("vmess://!!!").is_err());
}

#[test]
fn links_round_trip() {
    let links = [
        "vless://3f1c7a52-9d0e-4b8a-8c1f-2a7d6e5b4c3a@203.0.113.10:443?encryption=none&flow=xtls-rprx-vision&security=reality&sni=www.microsoft.com&fp=chrome&pbk=KEY&sid=6b&spx=%2F&type=tcp#NL",
        "vless://id@[2001:db8::1]:8443?type=ws&security=tls&path=%2Fws&host=a.com#v6",
        "trojan://pw@tr.example.com:443?type=grpc&serviceName=svc&security=tls#T",
        "ss://YWVzLTI1Ni1nY206cGFzcw@ss.example.com:8388#SS",
        "hy2://secret@hy.example.com:443/?sni=hy.example.com&obfs=salamander&obfs-password=ob#HY2",
        "tuic://u:pw@tu.example.com:443?congestion_control=bbr&alpn=h3#TUIC",
    ];
    for link in links {
        let parsed = parse_link(link).unwrap();
        let rebuilt = parse_link(&build_link(&parsed)).unwrap();
        assert_eq!(
            (parsed.protocol, &parsed.address, parsed.port, &parsed.uuid, &parsed.password, &parsed.transport, &parsed.tls, &parsed.name),
            (
                rebuilt.protocol,
                &rebuilt.address,
                rebuilt.port,
                &rebuilt.uuid,
                &rebuilt.password,
                &rebuilt.transport,
                &rebuilt.tls,
                &rebuilt.name
            ),
            "round trip of {link}"
        );
    }
    let vmess_json = r#"{"ps":"VM","add":"vm.com","port":"80","id":"u","net":"grpc","path":"svc","type":"multi","tls":"tls"}"#;
    let vmess = parse_link(&format!("vmess://{}", b64_encode(vmess_json.as_bytes()))).unwrap();
    let again = parse_link(&build_link(&vmess)).unwrap();
    assert_eq!(vmess.transport, again.transport);
    assert_eq!(again.transport.service_name, "svc");
    assert_eq!(again.transport.mode, "multi");
}
