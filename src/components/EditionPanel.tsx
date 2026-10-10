import { useCallback, useEffect, useState } from "react";
import type { EditionStatus, SeatInfo } from "../lib/ipc";
import { editionStatus, isTauriRuntime, licenceFingerprint, seatDeactivate, seatStatus } from "../lib/ipc";

/**
 * B-S4-22: what this licence includes, the pack-subscription notice, and moving the licence to another computer.
 * Nothing here can lock a user out of their own work: an expired licence makes the vault read-only (open and export still
 * work) and an expired pack subscription only stops new law from being installed.
 */
const EDITION_NAME: Record<string, string> = { ADVOCATE: "Advocate", PRO: "Pro", CHAMBERS_FIRM: "Chambers (firm)", JUDICIARY: "Judiciary (Chambers Edition)" };

function saveJson(name: string, value: unknown) {
  const url = URL.createObjectURL(new Blob([JSON.stringify(value, null, 2) + "\n"], { type: "application/json" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}
const when = (ms: number | null | undefined) => (ms ? new Date(ms).toISOString().slice(0, 10) : "");

export default function EditionPanel({ vaultUnlocked, onError, onNotice, onChanged }: { vaultUnlocked: boolean; onError: (m: string) => void; onNotice: (m: string) => void; onChanged: () => void }) {
  const [ed, setEd] = useState<(EditionStatus & { feature_labels?: Record<string, string> }) | null>(null);
  const [seat, setSeat] = useState<SeatInfo | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    if (!isTauriRuntime() || !vaultUnlocked) {
      setEd(null);
      setSeat(null);
      return;
    }
    try {
      setEd(await editionStatus());
      setSeat(await seatStatus());
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    }
  }, [vaultUnlocked, onError]);
  useEffect(() => {
    void load();
  }, [load]);

  if (!ed) return null;
  const labels = ed.feature_labels ?? {};
  const licensed = ed.source !== "NONE";
  const sub = ed.pack_subscription;
  const st = seat?.status ?? ed.seat;
  const released = seat?.released ?? null;

  const giveUp = async () => {
    if (!window.confirm("This computer will give up its licence so it can be moved to another computer. Your cases, documents and exports stay here and stay readable. You will need the issuer to re-issue the licence for the new computer. Continue?")) return;
    setBusy(true);
    try {
      const receipt = await seatDeactivate();
      saveJson("caseforge-seat-release-receipt.json", receipt);
      onNotice("This computer has given up its licence. The receipt was saved; send it to whoever issued your licence.");
      await load();
      onChanged();
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  const saveFingerprint = async () => {
    try {
      saveJson("caseforge-this-computer.json", await licenceFingerprint());
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="card" data-testid="edition-panel">
      <span className="eyebrow">Edition</span>
      <h3 className="digest-section-title">{licensed ? EDITION_NAME[ed.edition ?? ""] ?? ed.edition : "No edition licence activated"}</h3>
      {!licensed && <p className="muted" style={{ fontSize: "0.85rem" }}>{ed.problem ? `The licence on this computer could not be used: ${ed.problem}` : "Activate a version 2 licence below to use an edition. Without one, this install runs as an unlicensed development build."}</p>}
      {licensed && (
        <>
          <p style={{ fontSize: "0.85rem" }}>This licence includes: {ed.features.map((f) => labels[f] ?? f).join(", ") || "nothing"}.</p>
          {ed.ignored_features.length > 0 && <p className="muted" style={{ fontSize: "0.8rem" }}>The licence also names {ed.ignored_features.map((f) => labels[f] ?? f).join(", ")}, which this edition does not include.</p>}
          {ed.licence_state === "EXPIRED" && <div className="notice warn">Your licence has expired. Your work is read-only, and you can still open and export everything you made.{ed.licence_expires_at_unix_ms ? ` It ended on ${when(ed.licence_expires_at_unix_ms)}.` : ""}</div>}
        </>
      )}
      {ed.entitlement_map === "DEVELOPMENT_ONLY" && <p className="muted" style={{ fontSize: "0.78rem" }}>What each edition includes is a development placeholder until editions and prices are decided; it is not a statement of what any edition will contain.</p>}

      {sub.state === "UPDATES_EXPIRED" && <div className="notice warn" data-testid="updates-expired">{sub.notice}</div>}
      {sub.state === "SUBSCRIBED" && sub.until_date && <p className="muted" style={{ fontSize: "0.82rem" }}>Law pack updates are included until {sub.until_date}. After that, installed packs stay available and show the date they were last checked.</p>}

      {licensed && (
        <div style={{ marginTop: 8 }} data-testid="seat-transfer">
          <h4>Move this licence to another computer</h4>
          {!st || !st.enabled ? (
            <p className="muted" style={{ fontSize: "0.82rem" }}>Moving the licence is not enabled for this licence. Ask whoever issued it.</p>
          ) : (
            <p style={{ fontSize: "0.85rem" }}>
              You have used {st.used_in_window} of {st.limit_per_year} moves in the past {st.window_days} days; {st.remaining} left.
            </p>
          )}
          <p className="muted" style={{ fontSize: "0.78rem" }}>
            1. On the new computer, save its fingerprint (button below). 2. Here, give up this computer’s licence; a signed receipt is saved. 3. Send the receipt and the new computer’s fingerprint to whoever issued the licence. 4. Activate the licence they send back on the new computer. Nothing needs a network.
          </p>
          <div className="row" style={{ gap: 6, flexWrap: "wrap" }}>
            <button type="button" disabled={busy || !vaultUnlocked || !st?.can_deactivate} onClick={() => void giveUp()}>
              Give up this computer’s licence
            </button>
            <button type="button" onClick={() => void saveFingerprint()}>Save this computer’s fingerprint</button>
          </div>
          {st && !st.can_deactivate && st.reason && <p className="muted" style={{ fontSize: "0.78rem" }}>{st.reason}</p>}
        </div>
      )}
      {!licensed && released && <ReleasedNote released={released} />}
    </div>
  );
}

function ReleasedNote({ released }: { released: NonNullable<SeatInfo["released"]> }) {
  return (
    <div className="notice" data-testid="seat-released">
      This computer gave up licence {released.licence_id} on {when(released.released_at_unix_ms)}. Send the receipt to whoever issued it, and activate the re-issued licence on the new computer.{" "}
      <button type="button" onClick={() => saveJson("caseforge-seat-release-receipt.json", released.receipt)}>Save the receipt again</button>
    </div>
  );
}
