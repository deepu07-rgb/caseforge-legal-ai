//! LAN transfer of an encrypted bundle (B-S4-16). One computer offers the file on the local network for one download; the other
//! fetches it. No cloud, no relay, no name lookup: both ends use the Phone Companion's LAN rules (companion/mod.rs, AC-6.7):
//! the offer binds only a private, link-local or loopback address, the fetch connects only to such an address given as a number, and
//! a peer that is not on such a network is dropped before a byte is read. The offer serves exactly one successful download at a
//! secret path and then stops, or stops after its time limit. The bundle is already encrypted to its recipient, so what crosses the
//! wire is ciphertext either way.
//!
//! It is refused under air-gap by the command (lib.rs), like the Phone Companion. A USB file needs no network at all.

use crate::companion::{assert_bind_allowed, detect_lan_ipv4, is_lan_or_loopback};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const MAX_BYTES: usize = 512 * 1024 * 1024;
pub const E_NON_LAN: &str = "CF_BUNDLE_NON_LAN";

#[derive(Debug, Clone, Serialize)]
pub struct ShareInfo {
    pub url: String,
    pub bind_ip: String,
    pub port: u16,
    pub expires_in_s: u64,
    pub bytes: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Audit {
    pub bound: Vec<String>,
    pub peers_seen: Vec<(String, bool)>,
    pub connected_to: Vec<String>,
}

struct Live {
    stop: Arc<AtomicBool>,
}

static ACTIVE: Lazy<Mutex<Option<Live>>> = Lazy::new(|| Mutex::new(None));
pub static AUDIT: Lazy<Mutex<Audit>> = Lazy::new(|| Mutex::new(Audit::default()));

fn note<F: FnOnce(&mut Audit)>(f: F) {
    if let Ok(mut a) = AUDIT.lock() {
        f(&mut a);
    }
}

pub fn stop() {
    if let Ok(mut g) = ACTIVE.lock() {
        if let Some(l) = g.take() {
            l.stop.store(true, Ordering::SeqCst);
        }
    }
}

fn token() -> String {
    use rand::RngCore;
    let mut b = [0u8; 18];
    rand::rngs::OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

/// Offers `bytes` once, on this computer's local-network address (or `bind` if given and allowed).
pub fn share(bytes: Vec<u8>, port: Option<u16>, ttl: Duration, bind: Option<IpAddr>) -> Result<ShareInfo, String> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err("CF_BUNDLE_BAD_FILE: nothing to send, or the bundle is too large for a local transfer".into());
    }
    let ip = bind.unwrap_or_else(|| IpAddr::V4(detect_lan_ipv4().unwrap_or(Ipv4Addr::LOCALHOST)));
    assert_bind_allowed(ip).map_err(|e| e.replace("CF_COMPANION_NON_LAN", E_NON_LAN))?;
    stop();
    let listener = TcpListener::bind(SocketAddr::new(ip, port.unwrap_or(0))).map_err(|e| format!("CF_BUNDLE_LAN: cannot listen on {ip}: {e}"))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let local = listener.local_addr().map_err(|e| e.to_string())?;
    note(|a| a.bound.push(local.to_string()));
    let tok = token();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let flag = stop_flag.clone();
    let path = format!("/b/{tok}");
    let n = bytes.len();
    std::thread::spawn(move || {
        let deadline = Instant::now() + ttl;
        while !flag.load(Ordering::SeqCst) && Instant::now() < deadline {
            match listener.accept() {
                Ok((mut s, peer)) => {
                    let ok = is_lan_or_loopback(peer.ip());
                    note(|a| a.peers_seen.push((peer.ip().to_string(), ok)));
                    if !ok {
                        continue; // dropped before reading anything
                    }
                    let _ = s.set_nonblocking(false);
                    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
                    let _ = s.set_write_timeout(Some(Duration::from_secs(60)));
                    let mut head = [0u8; 2048];
                    let got = s.read(&mut head).unwrap_or(0);
                    let req = String::from_utf8_lossy(&head[..got]);
                    let line = req.lines().next().unwrap_or("");
                    if line.starts_with("GET ") && line.split_whitespace().nth(1) == Some(path.as_str()) {
                        let hdr = format!("HTTP/1.0 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {n}\r\nConnection: close\r\n\r\n");
                        if s.write_all(hdr.as_bytes()).is_ok() && s.write_all(&bytes).is_ok() && s.flush().is_ok() {
                            flag.store(true, Ordering::SeqCst); // one download, then done
                        }
                    } else {
                        let _ = s.write_all(b"HTTP/1.0 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(20)),
                Err(_) => break,
            }
        }
    });
    if let Ok(mut g) = ACTIVE.lock() {
        *g = Some(Live { stop: stop_flag });
    }
    Ok(ShareInfo { url: format!("http://{}:{}/b/{tok}", local.ip(), local.port()), bind_ip: local.ip().to_string(), port: local.port(), expires_in_s: ttl.as_secs(), bytes: n })
}

/// Fetches an offered bundle from `http://<ip>:<port>/b/<token>`; the address must be a local-network number.
pub fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let rest = url.trim().strip_prefix("http://").ok_or("CF_BUNDLE_LAN: the address must start with http://")?;
    let (hostport, path) = rest.split_once('/').ok_or("CF_BUNDLE_LAN: the address is incomplete")?;
    let addr: SocketAddr = hostport.parse().map_err(|_| "CF_BUNDLE_NON_LAN: use the numeric local-network address shown on the other computer, not a name".to_string())?;
    if !is_lan_or_loopback(addr.ip()) {
        return Err(format!("{E_NON_LAN}: {} is not on a local network; bundles are exchanged on the local network or by USB only", addr.ip()));
    }
    note(|a| a.connected_to.push(addr.to_string()));
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(5)).map_err(|e| format!("CF_BUNDLE_LAN: cannot reach {addr}: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(60))).map_err(|e| e.to_string())?;
    s.write_all(format!("GET /{path} HTTP/1.0\r\nHost: {hostport}\r\nConnection: close\r\n\r\n").as_bytes()).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 65536];
    loop {
        let n = s.read(&mut chunk).map_err(|e| format!("CF_BUNDLE_LAN: the transfer broke off: {e}"))?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() > MAX_BYTES + 4096 {
            return Err("CF_BUNDLE_LAN: the file is larger than a bundle can be".into());
        }
    }
    let split = buf.windows(4).position(|w| w == b"\r\n\r\n").ok_or("CF_BUNDLE_LAN: the other computer sent something that is not a bundle")?;
    let head = String::from_utf8_lossy(&buf[..split]).to_string();
    if !head.starts_with("HTTP/1.0 200") && !head.starts_with("HTTP/1.1 200") {
        return Err("CF_BUNDLE_LAN: the other computer refused the request (wrong code, or the offer has ended)".into());
    }
    let body = buf[split + 4..].to_vec();
    let want: usize = head.lines().find_map(|l| l.strip_prefix("Content-Length: ")).and_then(|v| v.trim().parse().ok()).ok_or("CF_BUNDLE_LAN: no length given")?;
    if body.len() != want {
        return Err("CF_BUNDLE_LAN: the transfer was cut short".into());
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// The tests share the one offer slot, so they take turns.
    static TURN: Mutex<()> = Mutex::new(());

    const FIXTURE: &str = include_str!("../../golden/G-BUNDLE/fixtures/bundle_core.json");

    fn lan() -> Value {
        serde_json::from_str::<Value>(FIXTURE).unwrap()["lan"].clone()
    }
    fn ips(v: &Value) -> Vec<IpAddr> {
        v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().parse().unwrap()).collect()
    }

    #[test]
    fn golden_only_local_network_addresses_are_allowed() {
        // AC-S4-10.3
        let _t = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let l = lan();
        for ip in ips(&l["allowed"]) {
            assert!(is_lan_or_loopback(ip), "{ip}");
        }
        for ip in ips(&l["refused"]) {
            assert!(!is_lan_or_loopback(ip), "{ip}");
            assert!(share(vec![1], None, Duration::from_secs(1), Some(ip)).unwrap_err().starts_with(E_NON_LAN), "bind {ip}");
            let url = match ip {
                IpAddr::V4(v) => format!("http://{v}:9/b/x"),
                IpAddr::V6(v) => format!("http://[{v}]:9/b/x"),
            };
            assert!(fetch(&url).unwrap_err().starts_with(E_NON_LAN), "fetch {ip}");
        }
        assert!(fetch("http://example.com:80/b/x").unwrap_err().starts_with(E_NON_LAN), "a name is never looked up");
        assert!(fetch("https://127.0.0.1:1/b/x").is_err());
    }

    #[test]
    fn a_bundle_crosses_the_local_network_once_and_only_with_the_secret_path() {
        let _t = TURN.lock().unwrap_or_else(|e| e.into_inner());
        stop();
        let data: Vec<u8> = (0..200_000u32).map(|i| (i * 31 % 251) as u8).collect();
        let info = share(data.clone(), None, Duration::from_secs(20), Some(IpAddr::V4(Ipv4Addr::LOCALHOST))).unwrap();
        assert_eq!(info.bind_ip, "127.0.0.1");
        // the wrong path is refused and does not use up the offer
        let wrong = info.url.rsplit_once('/').unwrap().0.to_string() + "/nope";
        assert!(fetch(&wrong).unwrap_err().contains("refused"));
        assert_eq!(fetch(&info.url).unwrap(), data, "the file arrives intact");
        // one download, then the offer is over
        std::thread::sleep(Duration::from_millis(100));
        assert!(fetch(&info.url).is_err(), "a second download is not possible");
        let a = AUDIT.lock().unwrap().clone();
        assert!(a.bound.iter().all(|b| b.starts_with("127.")), "{:?}", a.bound);
        assert!(a.peers_seen.iter().all(|(_, ok)| *ok) && !a.peers_seen.is_empty());
        assert!(a.connected_to.iter().all(|c| c.starts_with("127.")));
        if let Some(dir) = std::env::var_os("CASEFORGE_EVIDENCE_DIR") {
            std::fs::write(std::path::Path::new(&dir).join("bundle_lan.json"), serde_json::to_string_pretty(&a).unwrap()).unwrap();
        }
    }

    #[test]
    fn an_offer_ends_when_stopped_and_empty_or_oversize_offers_are_refused() {
        let _t = TURN.lock().unwrap_or_else(|e| e.into_inner());
        stop();
        let info = share(vec![7; 10], None, Duration::from_secs(20), Some(IpAddr::V4(Ipv4Addr::LOCALHOST))).unwrap();
        stop();
        std::thread::sleep(Duration::from_millis(150));
        assert!(fetch(&info.url).is_err());
        assert!(share(vec![], None, Duration::from_secs(1), None).is_err());
    }

    #[test]
    fn the_transfer_module_never_names_a_remote_service() {
        let src = include_str!("bundle_lan.rs");
        let code = src.split("#[cfg(test)]").next().unwrap();
        for tok in ["reqwest", "https://", "ToSocketAddrs", "lookup_host", "infer_llm", "ollama"] {
            assert!(!code.lines().filter(|l| !l.trim_start().starts_with("//")).any(|l| l.contains(tok)), "{tok}");
        }
    }
}
