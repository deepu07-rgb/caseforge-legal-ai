//! Phone Capture Companion (B-S1-11) — LAN-only HTTP + QR pairing.
//! AC-6.7: zero non-LAN traffic; bind only to private / loopback addresses.

use crate::vault::{DocumentIngestRequest, VaultManager};
use serde::Serialize;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use uuid::Uuid;

/// Private / loopback ranges only (AC-6.7).
pub fn is_lan_or_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                // APIPA
                || (v4.octets()[0] == 169 && v4.octets()[1] == 254)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.is_unique_local() || {
                // fe80::/10 link-local
                let segs = v6.segments();
                (segs[0] & 0xffc0) == 0xfe80
            }
        }
    }
}

pub fn assert_bind_allowed(ip: IpAddr) -> Result<(), String> {
    if is_lan_or_loopback(ip) {
        Ok(())
    } else {
        Err(format!(
            "CF_COMPANION_NON_LAN: refusing bind to non-LAN address {ip} (AC-6.7)"
        ))
    }
}

/// Prefer a private IPv4 for QR codes so phones on the same Wi-Fi can connect.
pub fn detect_lan_ipv4() -> Option<Ipv4Addr> {
    // Best-effort: UDP connect trick without sending packets to public internet.
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    // 192.0.2.1 is TEST-NET-1 (documentation); OS picks outbound interface.
    // We never send data — just peek local_addr after connect.
    socket.connect("192.0.2.1:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(v4) if is_lan_or_loopback(IpAddr::V4(v4)) => Some(v4),
        _ => None,
    }
}

#[derive(Clone, Serialize)]
pub struct CompanionSession {
    pub session_id: String,
    pub case_id: String,
    pub token: String,
    pub bind_ip: String,
    pub port: u16,
    pub pair_url: String,
    pub qr_text: String,
    pub status: String,
    pub uploads_accepted: u32,
    pub lan_only: bool,
    pub batch: &'static str,
}

struct LiveSession {
    session: CompanionSession,
    stop: Arc<AtomicBool>,
}

pub struct CompanionState {
    inner: Mutex<Option<LiveSession>>,
}

impl CompanionState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub fn status(&self) -> Option<CompanionSession> {
        self.inner
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|l| l.session.clone()))
    }
}

#[derive(Clone, Serialize)]
pub struct CompanionStartResult {
    pub session: CompanionSession,
    pub accepted: bool,
}

/// Start LAN-only companion HTTP server for a case.
pub fn start_companion(
    state: Arc<CompanionState>,
    vault: Arc<VaultManager>,
    case_id: String,
    preferred_port: Option<u16>,
) -> Result<CompanionStartResult, String> {
    // Ensure case is open
    let _ = vault.doc_list(&case_id).map_err(|e| e.to_string())?;

    // Stop any existing session
    stop_companion(state.clone())?;

    let lan = detect_lan_ipv4().unwrap_or(Ipv4Addr::LOCALHOST);
    assert_bind_allowed(IpAddr::V4(lan))?;

    let port = preferred_port.unwrap_or(17890);
    let bind_addr = SocketAddr::new(IpAddr::V4(lan), port);
    // Also try localhost if bind fails on detected LAN
    let listener = TcpListener::bind(bind_addr)
        .or_else(|_| {
            let loopback = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
            assert_bind_allowed(loopback.ip())?;
            TcpListener::bind(loopback).map_err(|e| e.to_string())
        })
        .map_err(|e| format!("CF_COMPANION_BIND: {e}"))?;

    let local = listener.local_addr().map_err(|e| e.to_string())?;
    assert_bind_allowed(local.ip())?;

    let session_id = Uuid::new_v4().to_string();
    let token_full = Uuid::new_v4().to_string().replace('-', "");
    let token = token_full.chars().take(16).collect::<String>();
    let pair_url = format!("http://{}:{}/c/{}", local.ip(), local.port(), token);
    let mut sess = CompanionSession {
        session_id: session_id.clone(),
        case_id: case_id.clone(),
        token: token.clone(),
        bind_ip: local.ip().to_string(),
        port: local.port(),
        pair_url: pair_url.clone(),
        qr_text: pair_url.clone(),
        status: "listening".into(),
        uploads_accepted: 0,
        lan_only: true,
        batch: "B-S1-11",
    };

    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut g = state.inner.lock().map_err(|e| e.to_string())?;
        *g = Some(LiveSession {
            session: sess.clone(),
            stop: stop.clone(),
        });
    }

    let state_t = state.clone();
    let vault_t = vault.clone();
    let case_t = case_id.clone();
    let token_t = token.clone();
    let stop_t = stop.clone();

    listener.set_nonblocking(false).map_err(|e| e.to_string())?;

    thread::spawn(move || {
        // Accept loop with short timeouts so stop can land
        let _ = listener.set_nonblocking(true);
        while !stop_t.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, peer)) => {
                    // Reject non-LAN peers (AC-6.7)
                    if !is_lan_or_loopback(peer.ip()) {
                        let _ = stream.write_all(
                            b"HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nNon-LAN peer rejected (AC-6.7)\n",
                        );
                        continue;
                    }
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
                    let _ = handle_client(&mut stream, &state_t, &vault_t, &case_t, &token_t);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(_) => {
                    thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
        if let Ok(mut g) = state_t.inner.lock() {
            if let Some(live) = g.as_mut() {
                live.session.status = "stopped".into();
            }
        }
    });

    sess.status = "listening".into();
    Ok(CompanionStartResult {
        session: sess,
        accepted: true,
    })
}

pub fn stop_companion(state: Arc<CompanionState>) -> Result<Option<CompanionSession>, String> {
    let mut g = state.inner.lock().map_err(|e| e.to_string())?;
    if let Some(live) = g.take() {
        live.stop.store(true, Ordering::SeqCst);
        let mut s = live.session;
        s.status = "stopped".into();
        Ok(Some(s))
    } else {
        Ok(None)
    }
}

fn bump_uploads(state: &CompanionState) {
    if let Ok(mut g) = state.inner.lock() {
        if let Some(live) = g.as_mut() {
            live.session.uploads_accepted = live.session.uploads_accepted.saturating_add(1);
        }
    }
}

fn handle_client(
    stream: &mut TcpStream,
    state: &CompanionState,
    vault: &VaultManager,
    case_id: &str,
    token: &str,
) -> Result<(), String> {
    let mut buf = vec![0u8; 65536];
    let mut req = Vec::new();
    loop {
        let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        req.extend_from_slice(&buf[..n]);
        if req.windows(4).any(|w| w == b"\r\n\r\n") {
            // may need more for body
            if let Some(header_end) = find_header_end(&req) {
                let headers = std::str::from_utf8(&req[..header_end]).unwrap_or("");
                let content_len = parse_content_length(headers).unwrap_or(0);
                let body_so_far = req.len().saturating_sub(header_end);
                if body_so_far >= content_len {
                    break;
                }
                // continue reading until content_len
                while req.len().saturating_sub(header_end) < content_len {
                    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
                    if n == 0 {
                        break;
                    }
                    req.extend_from_slice(&buf[..n]);
                    if req.len() > 12 * 1024 * 1024 {
                        return write_response(stream, 413, "text/plain", b"Payload too large");
                    }
                }
                break;
            }
        }
        if req.len() > 12 * 1024 * 1024 {
            return write_response(stream, 413, "text/plain", b"Payload too large");
        }
    }

    let header_end = find_header_end(&req).unwrap_or(req.len());
    let head = std::str::from_utf8(&req[..header_end]).unwrap_or("");
    let body = &req[header_end..];
    let first = head.lines().next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let path = parts.next().unwrap_or("/");

    if method == "GET" && (path == "/" || path.starts_with("/c/")) {
        let path_token = path.strip_prefix("/c/").unwrap_or("");
        if path != "/" && path_token != token {
            return write_response(stream, 403, "text/plain", b"Invalid pairing token");
        }
        let html = capture_page_html(token);
        return write_response(stream, 200, "text/html; charset=utf-8", html.as_bytes());
    }

    if method == "POST" && path == "/upload" {
        let json_body = std::str::from_utf8(body).unwrap_or("{}");
        let v: serde_json::Value = serde_json::from_str(json_body).unwrap_or(serde_json::json!({}));
        let req_token = v.get("token").and_then(|t| t.as_str()).unwrap_or("");
        if req_token != token {
            return write_response(
                stream,
                403,
                "application/json",
                br#"{"ok":false,"error":"invalid token"}"#,
            );
        }
        let filename = v
            .get("filename")
            .and_then(|t| t.as_str())
            .unwrap_or("capture.jpg");
        let content_b64 = v
            .get("content_base64")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        if content_b64.is_empty() {
            return write_response(
                stream,
                400,
                "application/json",
                br#"{"ok":false,"error":"content_base64 required"}"#,
            );
        }
        // Normalize extension
        let name = if filename.to_ascii_lowercase().ends_with(".jpg")
            || filename.to_ascii_lowercase().ends_with(".jpeg")
            || filename.to_ascii_lowercase().ends_with(".png")
            || filename.to_ascii_lowercase().ends_with(".pdf")
        {
            filename.to_string()
        } else {
            format!("{filename}.jpg")
        };
        let type_tag = if name.to_ascii_lowercase().ends_with(".png") {
            "PNG"
        } else if name.to_ascii_lowercase().ends_with(".pdf") {
            "PDF"
        } else {
            "JPG"
        };
        let ingest = DocumentIngestRequest {
            original_filename: name,
            content_base64: content_b64.to_string(),
            type_tag: Some(type_tag.into()),
            language_tag: Some("en".into()),
        };
        match vault.doc_ingest(case_id, ingest) {
            Ok(meta) => {
                bump_uploads(state);
                let resp = serde_json::json!({
                    "ok": true,
                    "doc_id": meta.doc_id,
                    "filename": meta.original_filename,
                    "lan_only": true,
                    "batch": "B-S1-11"
                });
                return write_response(
                    stream,
                    200,
                    "application/json",
                    resp.to_string().as_bytes(),
                );
            }
            Err(e) => {
                let resp = serde_json::json!({"ok": false, "error": e.to_string()});
                return write_response(
                    stream,
                    500,
                    "application/json",
                    resp.to_string().as_bytes(),
                );
            }
        }
    }

    if method == "GET" && path == "/health" {
        return write_response(
            stream,
            200,
            "application/json",
            br#"{"ok":true,"lan_only":true,"batch":"B-S1-11"}"#,
        );
    }

    write_response(stream, 404, "text/plain", b"Not found")
}

fn find_header_end(req: &[u8]) -> Option<usize> {
    req.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

fn parse_content_length(headers: &str) -> Option<usize> {
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            return rest.trim().parse().ok();
        }
    }
    None
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .map_err(|e| e.to_string())?;
    stream.write_all(body).map_err(|e| e.to_string())?;
    Ok(())
}

fn capture_page_html(token: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>CaseForge Capture</title>
<style>
  body {{ font-family: system-ui, sans-serif; margin: 1.25rem; background: #f4efe6; color: #1a1a1a; }}
  h1 {{ font-size: 1.2rem; }}
  .note {{ font-size: 0.85rem; color: #555; margin: 0.5rem 0 1rem; }}
  button, label.btn {{ display: block; width: 100%; padding: 0.85rem; margin: 0.4rem 0;
    font-size: 1rem; border: 1px solid #222; background: #1a1a1a; color: #f4efe6; border-radius: 6px; text-align: center; }}
  label.btn input {{ display: none; }}
  #status {{ margin-top: 1rem; font-family: ui-monospace, monospace; font-size: 0.8rem; white-space: pre-wrap; }}
  .ok {{ color: #2a7a4b; }}
  .err {{ color: #b42828; }}
</style>
</head>
<body>
  <h1>CaseForge · Phone Capture</h1>
  <p class="note">LAN only · token session · photos stay on your Wi‑Fi (AC-6.7). No cloud upload.</p>
  <label class="btn">Take / choose photo
    <input id="file" type="file" accept="image/*,application/pdf" capture="environment" multiple />
  </label>
  <button id="send" type="button">Upload to open case</button>
  <div id="status">Ready.</div>
<script>
const TOKEN = {token_json};
const statusEl = document.getElementById('status');
function log(msg, cls) {{
  statusEl.className = cls || '';
  statusEl.textContent = msg;
}}
document.getElementById('send').onclick = async () => {{
  const input = document.getElementById('file');
  if (!input.files || !input.files.length) {{ log('Choose at least one image.', 'err'); return; }}
  log('Uploading ' + input.files.length + ' file(s)…');
  let ok = 0, fail = 0;
  for (const file of input.files) {{
    try {{
      const buf = await file.arrayBuffer();
      const bytes = new Uint8Array(buf);
      let binary = '';
      for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
      const b64 = btoa(binary);
      const res = await fetch('/upload', {{
        method: 'POST',
        headers: {{ 'Content-Type': 'application/json' }},
        body: JSON.stringify({{ token: TOKEN, filename: file.name || 'capture.jpg', content_base64: b64 }})
      }});
      const j = await res.json();
      if (j.ok) ok++; else fail++;
    }} catch (e) {{ fail++; }}
  }}
  log('Done. accepted=' + ok + ' failed=' + fail + (fail ? '' : ' · LAN-only path'), fail ? 'err' : 'ok');
}};
</script>
</body>
</html>
"#,
        token_json = serde_json::to_string(token).unwrap_or_else(|_| "\"\"".into())
    )
}

/// Static policy facts for UI / packet-capture docs.
#[derive(Serialize)]
pub struct CompanionPolicy {
    pub batch: &'static str,
    pub lan_only: bool,
    pub allowed_bind: &'static [&'static str],
    pub forbidden: &'static [&'static str],
    pub ac: &'static str,
    pub packet_capture_doc: &'static str,
}

pub fn companion_policy() -> CompanionPolicy {
    CompanionPolicy {
        batch: "B-S1-11",
        lan_only: true,
        allowed_bind: &[
            "127.0.0.0/8",
            "10.0.0.0/8",
            "172.16.0.0/12",
            "192.168.0.0/16",
            "169.254.0.0/16",
            "fc00::/7",
            "fe80::/10",
        ],
        forbidden: &[
            "public unicast",
            "0.0.0.0 as advertised pair URL to internet",
        ],
        ac: "AC-6.7",
        packet_capture_doc: "docs/B-S1-11_phone_capture.md",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_ranges_ok() {
        assert!(is_lan_or_loopback(IpAddr::V4(Ipv4Addr::new(
            192, 168, 1, 10
        ))));
        assert!(is_lan_or_loopback(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5))));
        assert!(is_lan_or_loopback(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(is_lan_or_loopback(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
    }

    #[test]
    fn public_rejected() {
        assert!(!is_lan_or_loopback(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(assert_bind_allowed(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))).is_err());
    }
}
