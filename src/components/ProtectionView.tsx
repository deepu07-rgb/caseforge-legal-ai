/**
 * B-S1-19 Asset protection — packs/models at rest; never ransom user data.
 */

import { useCallback, useEffect, useState } from "react";
import type {
  CopiedFolderAudit,
  IntegrityResult,
  NoRansomAudit,
  ProtectionStatus,
} from "../lib/ipc";
import {
  isTauriRuntime,
  protectionCopiedFolderAudit,
  protectionNoRansomAudit,
  protectionPolicy,
  protectionProtectModels,
  protectionStatus,
  protectionVerifyAll,
} from "../lib/ipc";

export default function ProtectionView({
  vaultUnlocked,
  onError,
  onNotice,
}: {
  vaultUnlocked: boolean;
  onError: (msg: string) => void;
  onNotice: (msg: string) => void;
}) {
  const tauri = isTauriRuntime();
  const [status, setStatus] = useState<ProtectionStatus | null>(null);
  const [results, setResults] = useState<IntegrityResult[]>([]);
  const [copied, setCopied] = useState<CopiedFolderAudit | null>(null);
  const [ransom, setRansom] = useState<NoRansomAudit | null>(null);
  const [policyNote, setPolicyNote] = useState("");
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    if (!tauri) return;
    try {
      const pol = await protectionPolicy();
      setPolicyNote(
        `${pol.pipeline_id} · ${pol.cipher} · budget ≤${pol.effort_budget_percent}% · never ransom user data`,
      );
      setRansom(await protectionNoRansomAudit());
      if (vaultUnlocked) {
        setStatus(await protectionStatus());
        setResults(await protectionVerifyAll());
        setCopied(await protectionCopiedFolderAudit());
      }
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    }
  }, [tauri, vaultUnlocked, onError]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const protectModels = async () => {
    if (!vaultUnlocked) {
      onError("Unlock vault first.");
      return;
    }
    setBusy(true);
    try {
      const r = await protectionProtectModels();
      onNotice(`Models protected: ${r.assets_protected} asset(s)`);
      await refresh();
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const verify = async () => {
    if (!vaultUnlocked) return;
    setBusy(true);
    try {
      const r = await protectionVerifyAll();
      setResults(r);
      onNotice(`Verified ${r.length} scope(s)`);
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="page-grid protection-root" data-testid="protection-panel">
      <div className="matter-chips">
        <span className="chip matter">Protection</span>
        <span className="chip muted">B-S1-19</span>
        {status && (
          <>
            <span className="chip ok">{status.cipher}</span>
            <span className={`chip ${status.user_data_may_be_ransomed ? "warn" : "ok"}`}>
              ransom={String(status.user_data_may_be_ransomed)}
            </span>
            <span className="chip muted">
              protected={status.packs_protected} fail={status.packs_failed_integrity}
            </span>
          </>
        )}
      </div>

      <p className="muted g6-banner" data-testid="protection-g6">
        Economic protection for <strong>packs & models</strong> (AES-256-GCM, machine-bound).
        Tamper → graceful degrade (pack unusable).{" "}
        <strong>User case vaults are never ransomed</strong> (AC-14.2). Effort budget ≤ 8%.
      </p>

      <div className="digest-layout">
        <div className="card">
          <span className="eyebrow">Status</span>
          <h3 className="digest-section-title">Protection layer</h3>
          <p className="mono muted" style={{ fontSize: "0.75rem" }}>
            {policyNote}
          </p>
          {status ? (
            <ul className="gap-list" data-testid="protection-status">
              <li>pipeline: {status.pipeline_id}</li>
              <li>key fingerprint: {status.key_fingerprint}…</li>
              <li>models_protected: {String(status.models_protected)}</li>
              <li>foreign_machine_sim: {String(status.foreign_machine_sim)}</li>
              <li>{status.detail}</li>
            </ul>
          ) : (
            <p className="muted">Unlock vault to load protection status.</p>
          )}
          <div className="row" style={{ gap: "0.4rem", flexWrap: "wrap" }}>
            <button type="button" disabled={busy} onClick={() => void refresh()}>
              Refresh
            </button>
            <button
              type="button"
              className="primary"
              data-testid="protection-verify"
              disabled={busy || !vaultUnlocked}
              onClick={() => void verify()}
            >
              Verify integrity
            </button>
            <button
              type="button"
              data-testid="protection-models"
              disabled={busy || !vaultUnlocked}
              onClick={() => void protectModels()}
            >
              Protect models dir
            </button>
          </div>
        </div>

        <div className="card">
          <span className="eyebrow">AC-14.2 · no ransom</span>
          <h3 className="digest-section-title">User data invariants</h3>
          {ransom && (
            <ul className="gap-list" data-testid="no-ransom-audit">
              <li>
                pass: <strong>{String(ransom.pass)}</strong>
              </li>
              <li>user_data_may_be_ransomed: {String(ransom.user_data_may_be_ransomed)}</li>
              <li>
                encrypts_user_case_db: {String(ransom.protection_encrypts_user_case_db)}
              </li>
              <li>
                expired_deletes_data: {String(ransom.expired_licence_deletes_data)}
              </li>
              <li>
                online_kill_switch: {String(ransom.online_kill_switch_for_user_files)}
              </li>
              {ransom.findings.map((f) => (
                <li key={f} className="muted">
                  {f}
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>

      <div className="card">
        <span className="eyebrow">AC-14.1 · copied folder</span>
        <h3 className="digest-section-title">Machine-bound packs/models</h3>
        {copied ? (
          <div data-testid="copied-folder-audit">
            <p>
              pass: <strong>{String(copied.pass)}</strong> · user vault with passphrase:{" "}
              <strong>{String(copied.user_vault_opens_with_passphrase)}</strong>
            </p>
            <p className="muted" style={{ fontSize: "0.88rem" }}>
              {copied.notes}
            </p>
            <ul className="gap-list">
              {copied.findings.map((f) => (
                <li key={f}>{f}</li>
              ))}
            </ul>
          </div>
        ) : (
          <p className="muted">Unlock vault to run copied-folder audit.</p>
        )}
      </div>

      <div className="card">
        <h4>Integrity results</h4>
        <ul className="gap-list" data-testid="integrity-results">
          {results.map((r) => (
            <li key={r.scope_id}>
              <span className={`chip ${r.ok ? "ok" : "warn"}`}>
                {r.ok ? "ok" : "degraded"}
              </span>{" "}
              {r.scope_id} · checked={r.assets_checked} · user_data_touched=
              {String(r.user_data_touched)}
              {!r.ok && (
                <div className="mono muted" style={{ fontSize: "0.7rem" }}>
                  {r.failures.join("; ")}
                </div>
              )}
            </li>
          ))}
          {results.length === 0 && (
            <li className="muted">No protected scopes yet — install official packs or protect models.</li>
          )}
        </ul>
      </div>
    </div>
  );
}
