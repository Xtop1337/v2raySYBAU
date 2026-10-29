//! Every kind of protection against real local servers, through the app's own config generation
//! and batch "real delay" test. Runs only when `XRAY_BIN` and `SINGBOX_BIN` are set.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use sybau_core::model::{Profile, TestMethod};
use sybau_core::runtime::{free_ports, wait_for_port, CoreKind, CorePaths, CoreProcess};
use sybau_core::settings::Settings;
use sybau_core::share::parse_link;
use sybau_core::testing::{run_batch, BatchOptions, DelayResult};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

const UUID: &str = "3f1c7a52-9d0e-4b8a-8c1f-2a7d6e5b4c3a";
const SNI: &str = "example.com";

fn bin(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.exists())
}

fn output(command: &mut Command) -> String {
    let out = command.output().expect("run tool");
    String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr)
}

/// Value after `label` on the line that contains it, e.g. `PrivateKey: abc` -> `abc`.
fn field(text: &str, label: &str) -> String {
    let line = text.lines().find(|l| l.contains(label)).unwrap_or_else(|| panic!("no {label} in {text}"));
    line.rsplit([':', ' ']).next().unwrap().trim().trim_matches('"').to_string()
}

async fn http_204() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf).await;
                let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
            });
        }
    });
    port
}

fn spawn(kind: CoreKind, exe: &Path, args: &[&str], dir: &Path) -> CoreProcess {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    CoreProcess::spawn(kind, exe, &args, dir, Arc::new(|_| {})).unwrap()
}

struct Case {
    name: &'static str,
    link: String,
    /// The connection must work (`true`) or must be refused by the protection (`false`).
    expect_ok: bool,
}

#[tokio::test]
async fn every_protection_against_real_servers() {
    let (Some(xray), Some(singbox)) = (bin("XRAY_BIN"), bin("SINGBOX_BIN")) else {
        eprintln!("XRAY_BIN / SINGBOX_BIN not set, skipping");
        return;
    };
    let work = tempfile::tempdir().unwrap();
    let dir = work.path();
    let cores = dir.join("cores");
    std::fs::create_dir_all(&cores).unwrap();
    std::fs::copy(&xray, cores.join(CoreKind::Xray.exe_name())).unwrap();
    std::fs::copy(&singbox, cores.join(CoreKind::SingBox.exe_name())).unwrap();

    // Self-signed certificate for example.com and its SHA-256 fingerprint (for pinning).
    let (cert, key) = (dir.join("cert.pem"), dir.join("key.pem"));
    output(
        Command::new("openssl")
            .args(["req", "-x509", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:prime256v1", "-nodes", "-days", "2", "-subj"])
            .arg(format!("/CN={SNI}"))
            .arg("-addext")
            .arg(format!("subjectAltName=DNS:{SNI}"))
            .arg("-keyout")
            .arg(&key)
            .arg("-out")
            .arg(&cert),
    );
    let fingerprint = output(Command::new("openssl").args(["x509", "-noout", "-fingerprint", "-sha256", "-in"]).arg(&cert));
    let fingerprint = fingerprint.trim().split_once('=').unwrap().1.replace(':', "").to_lowercase();

    let keys = output(Command::new(&xray).arg("x25519"));
    let (private_key, public_key) = (field(&keys, "PrivateKey"), field(&keys, "PublicKey"));
    let wrong_public_key = field(&output(Command::new(&xray).arg("x25519")), "PublicKey");
    let mldsa = output(Command::new(&xray).arg("mldsa65"));
    let (mldsa_seed, mldsa_verify) = (field(&mldsa, "Seed"), field(&mldsa, "Verify"));
    let wrong_mldsa_verify = field(&output(Command::new(&xray).arg("mldsa65")), "Verify");
    let enc = output(Command::new(&xray).arg("vlessenc"));
    let x25519_block: Vec<&str> = enc.split("Authentication: ML-KEM-768").next().unwrap().lines().collect();
    let decryption = field(&x25519_block.join("\n"), "\"decryption\"");
    let encryption = field(&x25519_block.join("\n"), "\"encryption\"");

    let p = free_ports(13).unwrap();
    // ML-DSA-65 signatures go into the Reality certificate, which needs a target whose
    // certificate chain is large enough: pad the chain with three RSA-4096 certificates.
    let big_chain = dir.join("big-chain.pem");
    let mut chain = std::fs::read_to_string(&cert).unwrap();
    for i in 0..3 {
        let (c, k) = (dir.join(format!("pad{i}.pem")), dir.join(format!("pad{i}.key")));
        output(
            Command::new("openssl")
                .args(["req", "-x509", "-newkey", "rsa:4096", "-nodes", "-days", "2", "-subj", "/CN=pad"])
                .arg("-keyout")
                .arg(&k)
                .arg("-out")
                .arg(&c),
        );
        chain.push_str(&std::fs::read_to_string(&c).unwrap());
    }
    std::fs::write(&big_chain, chain).unwrap();
    let tls = json!({"serverName": SNI, "alpn": ["h2", "http/1.1"], "certificates": [{"certificateFile": cert, "keyFile": key}]});
    let reality = |extra: Value| {
        let mut r =
            json!({"target": format!("127.0.0.1:{}", p[2]), "serverNames": [SNI], "privateKey": private_key, "shortIds": ["", "ab12"]});
        sybau_core::util::merge_json(&mut r, &extra);
        r
    };
    let vless = |flow: &str| json!({"clients": [{"id": UUID, "flow": flow}], "decryption": "none"});
    let server = json!({
        "log": {"loglevel": "warning"},
        "inbounds": [
            {"port": p[0], "listen": "127.0.0.1", "protocol": "vless", "settings": vless("xtls-rprx-vision"),
             "streamSettings": {"network": "raw", "security": "reality", "realitySettings": reality(json!({}))}},
            {"port": p[1], "listen": "127.0.0.1", "protocol": "vless", "settings": vless(""),
             "streamSettings": {"network": "xhttp", "xhttpSettings": {"path": "/x"}, "security": "reality", "realitySettings": reality(json!({"mldsa65Seed": mldsa_seed, "target": format!("127.0.0.1:{}", p[12])}))}},
            {"port": p[12], "listen": "127.0.0.1", "protocol": "vless", "settings": vless(""),
             "streamSettings": {"network": "raw", "security": "tls", "tlsSettings": {"serverName": SNI, "alpn": ["h2", "http/1.1"], "certificates": [{"certificateFile": big_chain, "keyFile": key}]}}},
            {"port": p[11], "listen": "127.0.0.1", "protocol": "vless", "settings": vless(""),
             "streamSettings": {"network": "xhttp", "xhttpSettings": {"path": "/x"}, "security": "reality", "realitySettings": reality(json!({}))}},
            {"port": p[2], "listen": "127.0.0.1", "protocol": "vless", "settings": vless("xtls-rprx-vision"),
             "streamSettings": {"network": "raw", "security": "tls", "tlsSettings": tls}},
            {"port": p[3], "listen": "127.0.0.1", "protocol": "vless", "settings": vless(""),
             "streamSettings": {"network": "ws", "wsSettings": {"path": "/ws"}, "security": "tls", "tlsSettings": tls}},
            {"port": p[4], "listen": "127.0.0.1", "protocol": "vless", "settings": vless(""),
             "streamSettings": {"network": "grpc", "grpcSettings": {"serviceName": "svc"}, "security": "tls", "tlsSettings": tls}},
            {"port": p[5], "listen": "127.0.0.1", "protocol": "trojan", "settings": {"clients": [{"password": "trojan-pass"}]},
             "streamSettings": {"network": "raw", "security": "tls", "tlsSettings": tls}},
            {"port": p[6], "listen": "127.0.0.1", "protocol": "vmess", "settings": {"clients": [{"id": UUID}]},
             "streamSettings": {"network": "ws", "wsSettings": {"path": "/vm"}, "security": "tls", "tlsSettings": tls}},
            {"port": p[7], "listen": "127.0.0.1", "protocol": "shadowsocks", "settings": {"method": "2022-blake3-aes-128-gcm", "password": "MTIzNDU2Nzg5MDEyMzQ1Ng==", "network": "tcp,udp"}},
            {"port": p[8], "listen": "127.0.0.1", "protocol": "vless", "settings": {"clients": [{"id": UUID}], "decryption": decryption}}
        ],
        "outbounds": [{"protocol": "freedom", "settings": {"finalRules": [{"action": "allow", "ip": ["127.0.0.0/8"]}]}}]
    });
    let server_file = dir.join("server.json");
    std::fs::write(&server_file, server.to_string()).unwrap();
    let _xray_server = spawn(CoreKind::Xray, &cores.join(CoreKind::Xray.exe_name()), &["run", "-c", server_file.to_str().unwrap()], &cores);

    let sb_tls = json!({"enabled": true, "server_name": SNI, "certificate_path": cert, "key_path": key, "alpn": ["h3"]});
    let sb_server = json!({
        "log": {"level": "warn"},
        "inbounds": [
            {"type": "hysteria2", "listen": "127.0.0.1", "listen_port": p[9], "users": [{"password": "hy-pass"}],
             "obfs": {"type": "salamander", "password": "obfs-pass"}, "tls": sb_tls},
            {"type": "tuic", "listen": "127.0.0.1", "listen_port": p[10], "users": [{"uuid": UUID, "password": "tuic-pass"}], "tls": sb_tls}
        ],
        "outbounds": [{"type": "direct"}]
    });
    let sb_file = dir.join("sb-server.json");
    std::fs::write(&sb_file, sb_server.to_string()).unwrap();
    let _sb_server = spawn(CoreKind::SingBox, &cores.join(CoreKind::SingBox.exe_name()), &["run", "-c", sb_file.to_str().unwrap()], &cores);
    assert!(wait_for_port(p[8], Duration::from_secs(5), || true).await, "xray server did not start");
    tokio::time::sleep(Duration::from_millis(800)).await;

    let r = |port: u16, extra: &str| {
        format!("vless://{UUID}@127.0.0.1:{port}?security=reality&sni={SNI}&fp=chrome&pbk={public_key}&sid=ab12{extra}")
    };
    let t =
        |port: u16, extra: &str| format!("security=tls&sni={SNI}&fp=chrome{extra}&pcs={fingerprint}").replace("PORT", &port.to_string());
    let cases = vec![
        Case { name: "VLESS + Reality + XTLS-Vision", link: format!("{}&type=tcp&flow=xtls-rprx-vision#a", r(p[0], "")), expect_ok: true },
        Case { name: "VLESS + Reality + xHTTP", link: format!("{}&type=xhttp&path=%2Fx#b0", r(p[11], "")), expect_ok: true },
        Case { name: "VLESS + Reality + xHTTP + ML-DSA-65", link: format!("{}&type=xhttp&path=%2Fx&pqv={mldsa_verify}#b", r(p[1], "")), expect_ok: true },
        Case { name: "Reality + ML-DSA-65, wrong pqv key (must fail)", link: format!("{}&type=xhttp&path=%2Fx&pqv={wrong_mldsa_verify}#b2", r(p[1], "")), expect_ok: false },
        Case { name: "Reality with a wrong short id (must fail)", link: format!("vless://{UUID}@127.0.0.1:{}?security=reality&sni={SNI}&fp=chrome&pbk={public_key}&sid=ffff&type=tcp&flow=xtls-rprx-vision#c2", p[0]), expect_ok: false },
        Case { name: "Reality with a wrong public key (must fail)", link: format!("vless://{UUID}@127.0.0.1:{}?security=reality&sni={SNI}&fp=chrome&pbk={wrong_public_key}&sid=ab12&type=tcp&flow=xtls-rprx-vision#c", p[0]), expect_ok: false },
        Case { name: "VLESS + TLS + XTLS-Vision (pinned cert)", link: format!("vless://{UUID}@127.0.0.1:{}?type=tcp&flow=xtls-rprx-vision&{}#d", p[2], t(0, "")), expect_ok: true },
        Case { name: "VLESS + TLS + WebSocket (pinned cert)", link: format!("vless://{UUID}@127.0.0.1:{}?type=ws&path=%2Fws&{}#e", p[3], t(0, "")), expect_ok: true },
        Case { name: "VLESS + TLS + gRPC (pinned cert)", link: format!("vless://{UUID}@127.0.0.1:{}?type=grpc&serviceName=svc&{}#f", p[4], t(0, "")), expect_ok: true },
        Case { name: "Trojan + TLS (pinned cert)", link: format!("trojan://trojan-pass@127.0.0.1:{}?type=tcp&{}#g", p[5], t(0, "")), expect_ok: true },
        Case { name: "Trojan + TLS, allowInsecure=1 (self-signed)", link: format!("trojan://trojan-pass@127.0.0.1:{}?type=tcp&security=tls&sni={SNI}&allowInsecure=1#h", p[5]), expect_ok: true },
        Case { name: "Trojan + TLS, wrong pinned hash (must fail)", link: format!("trojan://trojan-pass@127.0.0.1:{}?type=tcp&security=tls&sni={SNI}&pcs={}#h2", p[5], "0".repeat(64)), expect_ok: false },
        Case { name: "Trojan + TLS, untrusted cert (must fail)", link: format!("trojan://trojan-pass@127.0.0.1:{}?type=tcp&security=tls&sni={SNI}#i", p[5]), expect_ok: false },
        Case { name: "VMess + TLS + WebSocket (pinned cert)", link: format!("vless://{UUID}@127.0.0.1:{}?type=ws&path=%2Fvm&{}#j", p[6], t(0, "")).replacen("vless://", "vmess://", 1), expect_ok: true },
        Case { name: "Shadowsocks 2022 (blake3-aes-128-gcm)", link: format!("ss://{}@127.0.0.1:{}#k", sybau_core::util::b64_encode_url(b"2022-blake3-aes-128-gcm:MTIzNDU2Nzg5MDEyMzQ1Ng=="), p[7]), expect_ok: true },
        Case { name: "VLESS Encryption (mlkem768x25519plus)", link: format!("vless://{UUID}@127.0.0.1:{}?type=tcp&encryption={}#l", p[8], encryption), expect_ok: true },
        Case { name: "Hysteria2 + TLS + Salamander obfs", link: format!("hy2://hy-pass@127.0.0.1:{}/?sni={SNI}&insecure=1&obfs=salamander&obfs-password=obfs-pass#m", p[9]), expect_ok: true },
        Case { name: "TUIC v5 + TLS", link: format!("tuic://{UUID}:tuic-pass@127.0.0.1:{}?sni={SNI}&alpn=h3&allow_insecure=1&congestion_control=bbr#n", p[10]), expect_ok: true },
    ];

    let target = http_204().await;
    let mut settings = Settings::default();
    settings.test.url = format!("http://127.0.0.1:{target}/generate_204");
    settings.test.timeout_ms = 4000;
    let profiles: Vec<Profile> = cases.iter().map(|c| parse_link(&c.link).unwrap_or_else(|e| panic!("{}: {e}", c.name))).collect();
    let paths = CorePaths { dir: cores.clone(), config_dir: dir.to_path_buf() };
    let results: Arc<Mutex<Vec<DelayResult>>> = Arc::default();
    let sink = results.clone();
    run_batch(
        profiles.clone(),
        &settings,
        &paths,
        &BatchOptions::from_settings(&settings, TestMethod::Real),
        CancellationToken::new(),
        Arc::new(move |r| sink.lock().unwrap().push(r)),
    )
    .await
    .unwrap();
    let results = results.lock().unwrap().clone();

    let mut failures = Vec::new();
    for (case, profile) in cases.iter().zip(&profiles) {
        let ms = results.iter().find(|r| r.profile_id == profile.id).map_or(-1, |r| r.ms);
        let ok = ms >= 0;
        let verdict = if ok == case.expect_ok { "OK  " } else { "FAIL" };
        println!("{verdict} {:<48} {}", case.name, if ok { format!("{ms} ms") } else { "no connection".into() });
        if ok != case.expect_ok {
            failures.push(case.name);
        }
    }
    assert!(failures.is_empty(), "unexpected results: {failures:?}");
}
