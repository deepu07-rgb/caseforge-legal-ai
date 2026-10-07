# B-S1-11 — Phone Capture Companion (LAN only)

**Status:** DONE (2026-08-07)  
**AC:** **AC-6.7** — Packet capture confirms zero non-LAN traffic.

## Goal

QR / URL pairing over **local Wi‑Fi only**. Phone uploads page images into the open case vault. **No cloud, no cellular relay.**

## Architecture

| Piece | Detail |
|--------|--------|
| Bind | Private IPv4 (or loopback fallback) only |
| Auth | Short session token in URL path `/c/{token}` |
| Upload | `POST /upload` JSON `{ token, filename, content_base64 }` |
| Ingest | Existing vault `doc_ingest` (JPG/PNG/PDF) |
| Host IPC | `companion_start` / `companion_stop` / `companion_status` / `companion_policy` |

### LAN policy (enforced in code)

**Allowed bind / peer:**

- `127.0.0.0/8`, `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `169.254.0.0/16`
- IPv6 ULA `fc00::/7`, link-local `fe80::/10`

**Rejected:** public unicast peers and non-LAN bind addresses (`CF_COMPANION_NON_LAN`).

## Dogfood

1. Open a case in CaseForge (desktop, vault unlocked).  
2. Case file → **Phone** → **Start LAN pairing**.  
3. On phone (same Wi‑Fi): open the pair URL shown in the app (or generate QR offline on the phone).  
4. Capture / choose photos → **Upload to open case**.  
5. Desktop documents list should show new JPGs.  
6. **Stop server** when finished.

## AC-6.7 packet-capture procedure (evidence)

Run this once per release candidate on a reference machine.

### Setup

1. Desktop and phone on the **same Wi‑Fi SSID**.  
2. Start companion pairing; note bind IP (must be RFC1918 / loopback).  
3. Start a packet capture on the desktop:

**Windows (example with Packet Monitor / Wireshark):**

```text
Filter (Wireshark display):
  !(ip.addr == 10.0.0.0/8 or ip.addr == 172.16.0.0/12 or ip.addr == 192.168.0.0/16
    or ip.addr == 127.0.0.0/8 or ipv6.addr == fe80::/10 or ipv6.addr == fc00::/7)
  and (tcp.port == <companion_port>)
```

Or capture all traffic during upload and filter by companion TCP port.

### Actions while capturing

1. Open pair URL on phone.  
2. Upload at least one JPG.  
3. Confirm ingest in desktop UI.  
4. Stop companion.

### Pass criteria

| Check | Pass |
|-------|------|
| Companion TCP peers | Only LAN / loopback addresses |
| Destination outside private ranges | **Zero** companion-related packets |
| Cloud hosts / public IPs | **None** for companion upload stream |
| Cellular path | Not used (same Wi‑Fi only) |

Attach a short note or screenshot of the capture filter + “0 matching non-LAN packets” to release evidence.

### Fail / halt

If the OS cannot bind a private interface, raise a `QUESTION` (batch halt condition). Do not advertise a public IP pair URL.

## Out of scope

- Cloud sync  
- Cellular upload gateways  
- Desktop cloud QR-code APIs (would violate AC-6.7)

## Eval

```powershell
npm run test:companion
python eval/run_eval.py --ac AC-6.7
```
