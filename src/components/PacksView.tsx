/**
 * B-S1-12 Knowledge packs UI — official (Ed25519 + AC-7.1) vs custom isolation.
 */

import { useState } from "react";
import type { HeadnoteScanResult, InstalledPack, PackSecurityReport } from "../lib/ipc";

export default function PacksView({
  packs,
  busy,
  lastScan,
  securityReport,
  onRefresh,
  onInstallPath,
  onUninstall,
  onHeadnoteScan,
  policyNote,
}: {
  packs: InstalledPack[];
  busy: boolean;
  lastScan: HeadnoteScanResult | null;
  securityReport: PackSecurityReport | null;
  onRefresh: () => Promise<void>;
  onInstallPath: (path: string) => Promise<void>;
  onUninstall: (packId: string) => Promise<void>;
  onHeadnoteScan: (packId: string) => Promise<void>;
  policyNote: string;
}) {
  const [path, setPath] = useState("");
  const official = packs.filter((p) => p.pack_kind === "official");
  const custom = packs.filter((p) => p.pack_kind === "custom");

  return (
    <div className="packs-layout" data-testid="packs-panel">
      <div className="row" style={{ justifyContent: "space-between", marginTop: 0 }}>
        <span className="eyebrow">library · knowledge packs</span>
        <span className="chip muted">{packs.length} installed</span>
      </div>
      <h3>Knowledge packs</h3>
      <p className="muted" style={{ fontSize: "0.85rem" }}>
        Official packs are Ed25519-signed and must pass AC-7.1 headnote scan (raw judgment + neutral
        citation only). Custom packs install into an isolated root. Features stay identical —
        jurisdiction switches KB only.
      </p>
      <p className="mono muted" style={{ fontSize: "0.72rem" }}>
        {policyNote}
      </p>
      {securityReport && (
        <div className="notice ok" data-testid="pack-security-report" style={{ marginBottom: "0.8rem" }}>
          <strong>Security report · {securityReport.batch}</strong>
          <div className="mono" style={{ fontSize: "0.72rem", marginTop: "0.35rem" }}>
            {securityReport.pack_id} · schema={securityReport.schema_version} · trust={securityReport.trust_domain} · files=
            {securityReport.files_validated} · bytes={securityReport.bytes_validated} · transactional=
            {String(securityReport.transactional_upgrade)} · protected={String(securityReport.protection_verified)}
          </div>
          <div className="muted" style={{ fontSize: "0.78rem", marginTop: "0.25rem" }}>
            {securityReport.checks.join(" · ")}
          </div>
        </div>
      )}

      <div className="packs-install">
        <label>
          Install from folder (USB / extracted .akpkg)
          <input
            value={path}
            onChange={(e) => setPath(e.target.value)}
            placeholder="C:\\packs\\caseforge.core.india.v0"
            data-testid="pack-source-path"
          />
        </label>
        <div className="row" style={{ marginTop: "0.55rem", gap: "0.4rem" }}>
          <button
            type="button"
            className="primary"
            disabled={busy || !path.trim()}
            onClick={() => void onInstallPath(path.trim())}
            data-testid="pack-install"
          >
            Install pack
          </button>
          <button type="button" disabled={busy} onClick={() => void onRefresh()}>
            Refresh
          </button>
        </div>
      </div>

      <div className="stat-label" style={{ marginTop: "1.15rem" }}>
        Official
      </div>
      {official.length === 0 ? (
        <p className="muted">No official packs installed.</p>
      ) : (
        <ul className="packs-list">
          {official.map((p) => (
            <li key={p.pack_id} className="packs-item" data-testid="pack-official">
              <div>
                <strong>{p.display_name}</strong>
                <div className="mono muted" style={{ fontSize: "0.72rem" }}>
                  {p.pack_id} · v{p.version} · {p.jurisdiction ?? "—"} · sig=
                  {String(p.signature_ok)} · schema={p.schema_version} · trust={p.trust_level} · beta=
                  {String(p.controlled_beta_eligible)} · headnotes_stripped={String(p.headnotes_stripped)}
                </div>
                <div className="muted" style={{ fontSize: "0.76rem", marginTop: "0.3rem" }}>
                  Publisher: {p.publisher ?? "not recorded"} · updated: {p.updated_at ?? "unknown"} · source: {p.source_status ?? "UNKNOWN"}
                </div>
                <div className="mono muted" style={{ fontSize: "0.7rem", marginTop: "0.2rem" }}>
                  signer={p.signature_ok ? "verified" : "unverified"} · editorial={p.editorial_review_status ?? "not recorded"} · rights={p.rights_basis ?? "not recorded"}
                </div>
                {(p.source_status === "SUPERSEDED" || p.source_status === "PARTIALLY_SUPERSEDED" || p.superseded_by_version) && (
                  <div className="notice warn" data-testid="pack-supersession-warning" style={{ marginTop: "0.45rem" }}>
                    Supersession warning: use {p.superseded_by_version ? `version ${p.superseded_by_version}` : "the current official edition"}.
                  </div>
                )}
                {!p.controlled_beta_eligible && (
                  <div className="notice warn" data-testid="controlled-beta-warning" style={{ marginTop: "0.45rem" }}>
                    Blocked for controlled legal testing: this pack is placeholder, legacy, custom, or otherwise unverified.
                  </div>
                )}
              </div>
              <div className="row" style={{ gap: "0.35rem" }}>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => void onHeadnoteScan(p.pack_id)}
                  data-testid="pack-headnote-scan"
                >
                  AC-7.1 scan
                </button>
                <button type="button" disabled={busy} onClick={() => void onUninstall(p.pack_id)}>
                  Remove
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      <div className="stat-label" style={{ marginTop: "1rem" }}>
        Custom (isolated)
      </div>
      {custom.length === 0 ? (
        <p className="muted">No custom packs installed.</p>
      ) : (
        <ul className="packs-list">
          {custom.map((p) => (
            <li key={p.pack_id} className="packs-item" data-testid="pack-custom">
              <div>
                <strong>{p.display_name}</strong>
                <div className="mono muted" style={{ fontSize: "0.72rem" }}>
                  {p.pack_id} · schema={p.schema_version} · trust={p.trust_level} · beta=
                  {String(p.controlled_beta_eligible)} · isolated={String(p.isolated)}
                </div>
              </div>
              <button type="button" disabled={busy} onClick={() => void onUninstall(p.pack_id)}>
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}

      {lastScan && (
        <div
          className={`notice ${lastScan.zero_matches ? "ok" : "warn"}`}
          style={{ marginTop: "1rem", fontSize: "0.85rem" }}
          data-testid="pack-scan-result"
        >
          {lastScan.ac} {lastScan.pack_id}:{" "}
          {lastScan.zero_matches
            ? "zero headnote matches"
            : `HITS: ${lastScan.hits.join("; ")}`}
        </div>
      )}
    </div>
  );
}
