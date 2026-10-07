/**
 * B-S1-11 Phone Capture Companion — LAN-only QR pairing UI.
 * AC-6.7: zero non-LAN traffic; pair URL uses private/loopback bind only.
 */

import type { CompanionPolicy, CompanionSession } from "../lib/ipc";

export default function CompanionView({
  session,
  policy,
  busy,
  onStart,
  onStop,
  onRefresh,
}: {
  session: CompanionSession | null;
  policy: CompanionPolicy | null;
  busy: boolean;
  onStart: () => Promise<void>;
  onStop: () => Promise<void>;
  onRefresh: () => Promise<void>;
}) {
  const listening = session?.status === "listening";

  return (
    <div className="companion-layout" data-testid="companion-panel">
      <div className="row" style={{ justifyContent: "space-between", marginTop: 0 }}>
        <span className="eyebrow">phone capture · LAN only</span>
        <span className={`chip ${listening ? "ok" : "muted"}`}>
          {session?.status ?? "stopped"}
        </span>
      </div>
      <h3>Phone Capture Companion</h3>
      <p className="muted" style={{ fontSize: "0.85rem" }}>
        Pair a phone on the same Wi‑Fi via QR / URL. Uploads go only to this machine’s private
        address (AC-6.7). No cloud, no cellular path.
      </p>

      <div className="notice warn" style={{ fontSize: "0.82rem" }}>
        Packet-capture procedure: see{" "}
        <span className="mono">{policy?.packet_capture_doc ?? "docs/B-S1-11_phone_capture.md"}</span>
        . Confirm zero non-LAN destinations for companion traffic.
      </div>

      <div className="row" style={{ marginTop: "0.85rem", flexWrap: "wrap", gap: "0.4rem" }}>
        <button
          type="button"
          className="primary"
          disabled={busy || listening}
          onClick={() => void onStart()}
          data-testid="companion-start"
        >
          Start LAN pairing
        </button>
        <button
          type="button"
          disabled={busy || !listening}
          onClick={() => void onStop()}
          data-testid="companion-stop"
        >
          Stop server
        </button>
        <button type="button" disabled={busy} onClick={() => void onRefresh()}>
          Refresh status
        </button>
      </div>

      {session && (
        <div className="companion-card" data-testid="companion-session">
          <div className="stat-label">Pair URL (scan or open on phone)</div>
          <p className="mono companion-url" data-testid="companion-pair-url">
            {session.pair_url}
          </p>
          <p className="mono muted" style={{ fontSize: "0.75rem" }}>
            bind {session.bind_ip}:{session.port} · token {session.token.slice(0, 8)}… · uploads{" "}
            {session.uploads_accepted} · lan_only={String(session.lan_only)}
          </p>
          {/* Lightweight QR: use external chart API is FORBIDDEN for AC-6.7.
              Render text QR via CSS block pattern is complex; show scannable URL + note. */}
          <div className="companion-qr-fallback" aria-label="QR placeholder">
            <p style={{ margin: 0, fontSize: "0.85rem" }}>
              Point the phone camera at a QR generator <strong>on the phone itself</strong> (offline
              app) using the URL above — or type the URL in the phone browser on the same Wi‑Fi.
              Desktop cloud QR APIs are intentionally not used (AC-6.7).
            </p>
          </div>
        </div>
      )}

      {policy && (
        <div style={{ marginTop: "1rem" }}>
          <div className="stat-label">Bind policy ({policy.ac})</div>
          <ul className="companion-policy">
            {policy.allowed_bind.map((a) => (
              <li key={a} className="mono">
                allow {a}
              </li>
            ))}
            {policy.forbidden.map((f) => (
              <li key={f} className="mono muted">
                deny {f}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
