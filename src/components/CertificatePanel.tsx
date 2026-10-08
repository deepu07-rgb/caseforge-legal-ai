import { useState } from "react";
import { downloadBytes, renderLinesPdf } from "../lib/exportDraft";
import { auditCertificateIssue, auditCertificateVerify, type AuditReport, type CertificateVerifyOutcome } from "../lib/ipc";

/**
 * B-S4-05: sign an Audit Certificate for the audit just run, or verify one received from someone else.
 * The certificate records the procedure (draft, packs, date, results) — never that a filing is legally correct.
 */
function certificateLines(cert: Record<string, any>): string[] {
  const r = cert.audit_report;
  const lines = [
    "CASEFORGE AUDIT CERTIFICATE",
    "",
    `Certificate: ${cert.certificate_id}`,
    `Issued: ${cert.issued_at}   App version: ${cert.app_version}`,
    `Document: ${r.subject.filename}   SHA-256: ${cert.draft_sha256}`,
    `Signing key: ${cert.signing.key_id} (Ed25519, per-install)`,
    "",
    "Packs checked against:",
    ...cert.pack_snapshot.map((p: any) => `  - ${p.pack_id}  version ${p.version}${p.currency_date ? `  current to ${p.currency_date}` : ""}`),
    "",
    `Items audited: ${cert.summary.total_items}`,
    ...Object.entries(cert.summary.counts as Record<string, number>).filter(([, n]) => n > 0).map(([k, n]) => `  ${k}: ${n}`),
    "",
    "Results:",
    ...r.items.map((i: any) => `  [${i.result}] ${i.kind === "QUOTATION" ? `"${i.draft_text}"` : i.draft_text}${i.pinpoint?.claimed ? `  (pinpoint ${i.pinpoint.claimed}: ${i.pinpoint.status})` : ""}`),
    "",
    "Scope:",
    cert.scope_statement,
  ];
  if (cert.disclosure?.included) lines.push("", "Disclosure (counsel's own words):", cert.disclosure.text);
  lines.push(
    "",
    "Verify offline: python verify_certificate.py <certificate.json> --draft <document.txt>",
    "This printout is a summary; the signed JSON file is the certificate.",
  );
  return lines;
}

export function CertificatePanel({ report, text }: { report: AuditReport; text: string }) {
  const [disclosure, setDisclosure] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const [issued, setIssued] = useState<Record<string, any> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function issue() {
    setBusy(true);
    setError(null);
    try {
      const cert = await auditCertificateIssue({ report, draftText: text, disclosureText: disclosure.trim() || null, disclosureConfirmed: confirmed });
      setIssued(cert);
      const base = `caseforge-certificate-${cert.certificate_id}`;
      downloadBytes(`${base}.json`, new TextEncoder().encode(JSON.stringify(cert, null, 2)), "application/json");
      downloadBytes(`${base}.pdf`, renderLinesPdf(certificateLines(cert)), "application/pdf");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div style={{ marginTop: 10, paddingTop: 8, borderTop: "1px solid var(--line, #d6d9de)" }}>
      <h4 style={{ marginBottom: 4 }}>Audit certificate</h4>
      <p className="muted" style={{ fontSize: "0.8rem" }}>
        A signed record that this audit was run on this exact draft against the listed packs today. It does not certify that the draft is legally correct or compliant with any court's rules.
      </p>
      <label className="muted" style={{ fontSize: "0.8rem" }} htmlFor="cert-disclosure">
        Optional AI-use disclosure (your own words — required by some courts)
      </label>
      <textarea id="cert-disclosure" rows={2} value={disclosure} onChange={(e) => { setDisclosure(e.target.value); setConfirmed(false); }} style={{ width: "100%" }} />
      {disclosure.trim() && (
        <label style={{ fontSize: "0.8rem", display: "block" }}>
          <input type="checkbox" checked={confirmed} onChange={(e) => setConfirmed(e.target.checked)} /> I have reviewed this disclosure and want it on the certificate
        </label>
      )}
      <div className="row" style={{ marginTop: 6 }}>
        <button type="button" onClick={() => void issue()} disabled={busy || (disclosure.trim() !== "" && !confirmed)}>
          {busy ? "Signing…" : "Create certificate (JSON + PDF)"}
        </button>
      </div>
      {error && <div className="notice warn" style={{ marginTop: 6 }}>{error}</div>}
      {issued && (
        <div className="muted mono" style={{ fontSize: "0.72rem", marginTop: 6 }}>
          Saved {issued.certificate_id} · signing key {issued.signing.key_id} — share this key id so recipients can confirm the certificate came from you.
        </div>
      )}
    </div>
  );
}

export function VerifyCertificatePanel({ currentDraft }: { currentDraft: string }) {
  const [outcome, setOutcome] = useState<CertificateVerifyOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [useDraft, setUseDraft] = useState(false);

  async function onFile(file: File | undefined) {
    if (!file) return;
    setError(null);
    try {
      setOutcome(await auditCertificateVerify(await file.text(), useDraft ? currentDraft : null));
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div style={{ marginTop: 10, paddingTop: 8, borderTop: "1px solid var(--line, #d6d9de)" }}>
      <h4 style={{ marginBottom: 4 }}>Verify a certificate</h4>
      <label style={{ fontSize: "0.8rem", display: "block" }}>
        <input type="checkbox" checked={useDraft} onChange={(e) => setUseDraft(e.target.checked)} /> Also check it against the draft currently in the editor
      </label>
      <input type="file" accept=".json,application/json" onChange={(e) => void onFile(e.target.files?.[0])} />
      {error && <div className="notice warn">{error}</div>}
      {outcome && (
        <div style={{ marginTop: 6, fontSize: "0.82rem" }}>
          <div style={{ fontWeight: 600, color: outcome.valid ? "var(--ok, #2f7d4f)" : "var(--danger, #a33a2b)" }}>
            {outcome.valid ? "Valid — unaltered since signing" : "Invalid — this certificate or document has been changed"} · key {outcome.key_id ?? "?"}
          </div>
          <ul style={{ margin: "4px 0 0 1rem", padding: 0 }}>
            {outcome.checks.map((c) => (
              <li key={c.name} style={{ color: c.passed ? undefined : "var(--danger, #a33a2b)" }}>
                {c.passed ? "✓" : "✗"} {c.name}: {c.detail}
              </li>
            ))}
          </ul>
          <div className="muted" style={{ fontSize: "0.72rem", marginTop: 4 }}>{outcome.note}</div>
        </div>
      )}
    </div>
  );
}
