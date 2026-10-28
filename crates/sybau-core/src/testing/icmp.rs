use std::net::IpAddr;
use std::time::Duration;

async fn resolve_ip(host: &str) -> Option<IpAddr> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Some(ip);
    }
    let addrs: Vec<_> = tokio::net::lookup_host((host, 0)).await.ok()?.collect();
    addrs.iter().find(|a| a.is_ipv4()).or(addrs.first()).map(|a| a.ip())
}

/// ICMP echo round-trip time. Windows uses `IcmpSendEcho` (no administrator rights needed).
pub async fn icmp_ping(host: &str, timeout: Duration) -> Option<Duration> {
    let ip = tokio::time::timeout(timeout, resolve_ip(host)).await.ok()??;
    platform::ping(ip, timeout).await
}

#[cfg(windows)]
mod platform {
    use std::net::IpAddr;
    use std::time::Duration;

    use windows::Win32::NetworkManagement::IpHelper::{
        Icmp6CreateFile, Icmp6SendEcho2, IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMPV6_ECHO_REPLY_LH, ICMP_ECHO_REPLY,
    };
    use windows::Win32::Networking::WinSock::{AF_INET6, SOCKADDR_IN6};

    pub async fn ping(ip: IpAddr, timeout: Duration) -> Option<Duration> {
        tokio::task::spawn_blocking(move || ping_blocking(ip, timeout)).await.ok()?
    }

    fn ping_blocking(ip: IpAddr, timeout: Duration) -> Option<Duration> {
        let payload = [0x61u8; 32];
        let timeout_ms = timeout.as_millis().min(u32::MAX as u128) as u32;
        unsafe {
            match ip {
                IpAddr::V4(v4) => {
                    let handle = IcmpCreateFile().ok()?;
                    let mut reply = vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + payload.len() + 8];
                    let count = IcmpSendEcho(
                        handle,
                        u32::from_ne_bytes(v4.octets()),
                        payload.as_ptr() as *const _,
                        payload.len() as u16,
                        None,
                        reply.as_mut_ptr() as *mut _,
                        reply.len() as u32,
                        timeout_ms,
                    );
                    let _ = IcmpCloseHandle(handle);
                    if count == 0 {
                        return None;
                    }
                    let echo = &*(reply.as_ptr() as *const ICMP_ECHO_REPLY);
                    (echo.Status == 0).then(|| Duration::from_millis(echo.RoundTripTime.max(1) as u64))
                }
                IpAddr::V6(v6) => {
                    let handle = Icmp6CreateFile().ok()?;
                    let mut source: SOCKADDR_IN6 = std::mem::zeroed();
                    source.sin6_family = AF_INET6;
                    let mut dest: SOCKADDR_IN6 = std::mem::zeroed();
                    dest.sin6_family = AF_INET6;
                    dest.sin6_addr.u.Byte = v6.octets();
                    let mut reply = vec![0u8; std::mem::size_of::<ICMPV6_ECHO_REPLY_LH>() + payload.len() + 8];
                    let count = Icmp6SendEcho2(
                        handle,
                        None,
                        None,
                        None,
                        &source,
                        &dest,
                        payload.as_ptr() as *const _,
                        payload.len() as u16,
                        None,
                        reply.as_mut_ptr() as *mut _,
                        reply.len() as u32,
                        timeout_ms,
                    );
                    let _ = IcmpCloseHandle(handle);
                    if count == 0 {
                        return None;
                    }
                    let echo = &*(reply.as_ptr() as *const ICMPV6_ECHO_REPLY_LH);
                    (echo.Status == 0).then(|| Duration::from_millis(echo.RoundTripTime.max(1) as u64))
                }
            }
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::net::IpAddr;
    use std::time::Duration;

    use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};

    pub async fn ping(ip: IpAddr, timeout: Duration) -> Option<Duration> {
        let kind = if ip.is_ipv4() { ICMP::V4 } else { ICMP::V6 };
        // Unprivileged datagram ICMP sockets (Linux `ping_group_range`, macOS).
        let client = Client::new(&Config::builder().kind(kind).build()).ok()?;
        let mut pinger = client.pinger(ip, PingIdentifier(rand_id())).await;
        pinger.timeout(timeout);
        pinger.ping(PingSequence(0), &[0u8; 32]).await.ok().map(|(_, rtt)| rtt)
    }

    fn rand_id() -> u16 {
        (uuid::Uuid::new_v4().as_u128() & 0xffff) as u16
    }
}
