/**
 * B-S1-18 Licensing, air-gap, AC-13.1 network audit.
 * EXPIRED = read-only vault; data never ransomed.
 */

import { useCallback, useEffect, useState } from "react";
import EditionPanel from "./EditionPanel";
import type {
  FingerprintComponents,
  LicenceStatus,
  NetworkAuditReport,
} from "../lib/ipc";
import {
  airGapSet,
  isTauriRuntime,
  licenceActivate,
  licenceFingerprint,
  licenceMarkExpired,
  licencePolicy,
  licenceStatus,
  networkAudit,
} from "../lib/ipc";

export default function LicenceView({
  vaultUnlocked,
  onError,
  onNotice,
}: {
  vaultUnlocked: boolean;
  onError: (msg: string) => void;
  onNotice: (msg: string) => void;
}) {
  const tauri = isTauriRuntime();
  const [status, setStatus] = useState<LicenceStatus | null>(null);
  const [fp, setFp] = useState<FingerprintComponents | null>(null);
  const [audit, setAudit] = useState<NetworkAuditReport | null>(null);
  const [payloadJson, setPayloadJson] = useState("");
  const [policyNote, setPolicyNote] = useState("");
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    if (!tauri) return;
    try {
      const pol = await licencePolicy();
      setPolicyNote(
        `states=${JSON.stringify(pol.states)} · fingerprint 3-of-4 · Ed25519 offline`,
      );
      setFp(await licenceFingerprint());
      if (vaultUnlocked) {
        setStatus(await licenceStatus());
        setAudit(await networkAudit());
      } else {
        setStatus(null);
        setAudit(null);
      }
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    }
  }, [tauri, vaultUnlocked, onError]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const activate = async () => {
    if (!vaultUnlocked) {
      onError("Unlock vault before activation.");
      return;
    }
    setBusy(true);
    try {
      const r = await licenceActivate(payloadJson);
      setStatus(r.status);
      onNotice(
        `Licence ${r.status.state} · fingerprint match ${r.fingerprint_match_count}/4`,
      );
      await refresh();
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const toggleAirGap = async (enabled: boolean) => {
    if (!vaultUnlocked) {
      onError("Unlock vault first.");
      return;
    }
    setBusy(true);
    try {
      const s = await airGapSet(enabled);
      setStatus(s);
      onNotice(enabled ? "Air-gap ON — networking killed" : "Air-gap OFF");
      setAudit(await networkAudit());
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const markExpired = async () => {
    if (!vaultUnlocked) return;
    setBusy(true);
    try {
      const s = await licenceMarkExpired();
      setStatus(s);
      onNotice("Marked EXPIRED — vault read-only; data not ransomed");
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const stateClass =
    status?.state === "ACTIVE"
      ? "ok"
      : status?.state === "EXPIRED"
        ? "warn"
        : "muted";

  return (
    <div className="page-grid licence-root" data-testid="licence-panel">
      <div className="matter-chips">
        <span className="chip matter">Licence · air-gap</span>
        <span className="chip muted">B-S1-18</span>
        {status && (
          <>
            <span className={`chip ${stateClass}`} data-testid="licence-state">
              {status.state}
            </span>
            <span className={`chip ${status.vault_writable ? "ok" : "warn"}`}>
              vault {status.vault_writable ? "writable" : "read-only"}
            </span>
            <span className={`chip ${status.air_gap_enabled ? "warn" : "muted"}`}>
              air-gap={String(status.air_gap_enabled)}
            </span>
          </>
        )}
        {!vaultUnlocked && <span className="chip warn">Vault locked</span>}
      </div>

      <p className="muted g6-banner" data-testid="licence-g6">
        Offline Ed25519 licence · machine fingerprint 3-of-4.{" "}
        <strong>EXPIRED = read-only vault</strong> — user data is never ransomed or deleted.
        Case data has zero network path (AC-13.1). Air-gap kills all networking.
      </p>

      <div className="digest-layout">
        <div className="card">
          <span className="eyebrow">Status</span>
          <h3 className="digest-section-title">Licence state machine</h3>
          <p className="mono muted" style={{ fontSize: "0.75rem" }}>
            {policyNote}
          </p>
          {status ? (
            <ul className="gap-list" data-testid="licence-status">
              <li>
                state: <strong>{status.state}</strong>
              </li>
              <li>licence_id: {status.licence_id ?? "—"}</li>
              <li>tier: {status.tier ?? "—"}</li>
              <li>
                fingerprint: {status.fingerprint_match_count}/
                {status.fingerprint_total} (need {status.fingerprint_required})
              </li>
              <li>expires_at: {status.expires_at_unix_ms ?? "perpetual / none"}</li>
              <li>detail: {status.detail}</li>
              <li>
                ransom_allowed: <strong>{String(status.user_data_may_be_ransomed)}</strong>
              </li>
              <li>
                case_data_network:{" "}
                <strong>{String(status.case_data_network_path_allowed)}</strong>
              </li>
            </ul>
          ) : (
            <p className="muted">Unlock vault to load licence state.</p>
          )}
          <div className="row" style={{ gap: "0.4rem", flexWrap: "wrap" }}>
            <button type="button" disabled={busy} onClick={() => void refresh()}>
              Refresh
            </button>
            <button
              type="button"
              data-testid="licence-mark-expired"
              disabled={busy || !vaultUnlocked}
              onClick={() => void markExpired()}
            >
              Mark EXPIRED (test)
            </button>
          </div>
        </div>

        <div className="card">
          <span className="eyebrow">Fingerprint · 3-of-4</span>
          <h3 className="digest-section-title">This machine</h3>
          {fp ? (
            <ul className="gap-list mono" style={{ fontSize: "0.72rem" }} data-testid="licence-fp">
              <li>hostname: {fp.hostname.slice(0, 24)}…</li>
              <li>username: {fp.username.slice(0, 24)}…</li>
              <li>machine_guid: {fp.machine_guid.slice(0, 24)}…</li>
              <li>disk_root: {fp.disk_root.slice(0, 24)}…</li>
            </ul>
          ) : (
            <p className="muted">Fingerprint unavailable outside shell.</p>
          )}
          <p className="muted" style={{ fontSize: "0.85rem" }}>
            Bind a licence with <code>python eval/tools/make_licence.py --bind-live</code>
          </p>
        </div>
      </div>

      <EditionPanel vaultUnlocked={vaultUnlocked} onError={onError} onNotice={onNotice} onChanged={() => void refresh()} />

      <div className="card">
        <span className="eyebrow">Activation · offline</span>
        <h3 className="digest-section-title">Install signed licence JSON</h3>
        <textarea
          className="draft-editor"
          rows={10}
          data-testid="licence-payload"
          placeholder='Paste licence JSON from make_licence.py …'
          value={payloadJson}
          onChange={(e) => setPayloadJson(e.target.value)}
        />
        <button
          type="button"
          className="primary"
          data-testid="licence-activate"
          style={{ marginTop: "0.5rem" }}
          disabled={busy || !vaultUnlocked || !payloadJson.trim()}
          onClick={() => void activate()}
        >
          Activate offline
        </button>
      </div>

      <div className="card">
        <span className="eyebrow">Air-gap · AC-13.1</span>
        <h3 className="digest-section-title">Network policy</h3>
        <div className="row" style={{ gap: "0.4rem", marginBottom: "0.75rem" }}>
          <button
            type="button"
            className="primary"
            data-testid="airgap-on"
            disabled={busy || !vaultUnlocked}
            onClick={() => void toggleAirGap(true)}
          >
            Enable air-gap
          </button>
          <button
            type="button"
            data-testid="airgap-off"
            disabled={busy || !vaultUnlocked}
            onClick={() => void toggleAirGap(false)}
          >
            Disable air-gap
          </button>
        </div>
        {audit && (
          <div data-testid="network-audit">
            <p>
              AC-13.1:{" "}
              <strong className={audit.pass ? "filled" : ""}>
                {audit.pass ? "PASS" : "FAIL"}
              </strong>
            </p>
            <p className="muted" style={{ fontSize: "0.88rem" }}>
              {audit.notes}
            </p>
            <ul className="gap-list">
              {audit.findings.map((f) => (
                <li key={f}>{f}</li>
              ))}
            </ul>
            <p className="mono muted" style={{ fontSize: "0.7rem" }}>
              case_data_network_path_allowed={String(audit.case_data_network_path_allowed)} ·
              air_gap={String(audit.air_gap_enabled)}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
