import { useEffect, useState } from "react";
import { packCurrency, treatmentLookup, type PackStamp, type TreatmentEdge, type TreatmentLookup } from "../lib/ipc";

/**
 * B-S4-13: the shared parts of every citation surface.
 *  - CurrencyStamp: "verified against <pack> as of <currency date>" (or what the pack does not state).
 *  - TreatmentBlock: what judgments in the installed library say about a cited judgment, each with the exact citing text.
 *    "No treatment found in installed corpus" is a statement about the library only. Nothing here says a case is still
 *    reliable, and nothing here changes a citation's standing.
 */
let cache: { at: number; promise: Promise<PackStamp[]> } | null = null;
export function invalidatePackStamps() {
  cache = null;
}
function loadStamps(): Promise<PackStamp[]> {
  if (!cache || Date.now() - cache.at > 30_000) cache = { at: Date.now(), promise: packCurrency().catch(() => []) };
  return cache.promise;
}
export function usePackStamps(): PackStamp[] | null {
  const [stamps, setStamps] = useState<PackStamp[] | null>(null);
  useEffect(() => {
    let live = true;
    void loadStamps().then((s) => live && setStamps(s));
    return () => {
      live = false;
    };
  }, []);
  return stamps;
}

/** The wording, kept in one place. Same as treatment::stamp_text on the host. */
export function stampText(name: string, currencyDate: string | null | undefined, updatesExpiredOn?: string | null): string {
  const base = currencyDate ? `verified against ${name} as of ${currencyDate}` : `verified against ${name}; this pack does not state a currency date`;
  return updatesExpiredOn ? base + updatesSuffix(updatesExpiredOn) : base;
}
/** Same as subscription::stamp_suffix on the host: once the pack subscription has ended, the older date stays and says why. */
export function updatesSuffix(endedOn: string): string {
  return ` \u2014 updates expired: the pack subscription ended on ${endedOn}, so newer law may not be reflected`;
}

export function CurrencyStamp({ packId, currencyDate, name }: { packId: string | null | undefined; currencyDate?: string | null; name?: string }) {
  const stamps = usePackStamps();
  const pack = packId ? stamps?.find((p) => p.pack_id === packId) : undefined;
  let text: string;
  if (currencyDate !== undefined || name) text = stampText(name ?? packId ?? "its pack", currencyDate, pack?.updates_expired_on);
  else if (!packId) text = "pack not stated for this citation";
  else if (!stamps) text = "checking the pack’s currency date…";
  else if (!pack) text = `pack ${packId} is no longer installed`;
  else text = stampText(pack.display_name || pack.pack_id, pack.currency_date, pack.updates_expired_on);
  return (
    <div className="muted" style={{ fontSize: "0.72rem" }} data-testid="currency-stamp">
      {text}
    </div>
  );
}

/** Every installed pack and its currency date, for surfaces that cite law without naming one pack. */
export function PackCurrencyList() {
  const stamps = usePackStamps();
  if (!stamps) return null;
  if (stamps.length === 0) return <div className="muted" style={{ fontSize: "0.72rem" }} data-testid="currency-stamp">No legal packs are installed.</div>;
  return (
    <div className="muted" style={{ fontSize: "0.72rem" }} data-testid="currency-stamp">
      Law available here: {stamps.map((p) => `${p.display_name || p.pack_id} ${p.currency_date ? `as of ${p.currency_date}` : "(no currency date stated)"}${p.updates_expired_on ? updatesSuffix(p.updates_expired_on) : ""}`).join("; ")}
    </div>
  );
}

const LABEL: Record<TreatmentEdge["treatment"], string> = {
  OVERRULED: "Overruled",
  REVERSED: "Reversed",
  PER_INCURIAM: "Per incuriam",
  REFERRED_TO_LARGER_BENCH: "Referred to a larger Bench",
  DOUBTED: "Doubted",
  DISTINGUISHED: "Distinguished",
  FOLLOWED: "Followed",
};
const TONE: Record<TreatmentEdge["treatment"], string> = {
  OVERRULED: "var(--danger, #a33a2b)",
  REVERSED: "var(--danger, #a33a2b)",
  PER_INCURIAM: "var(--danger, #a33a2b)",
  REFERRED_TO_LARGER_BENCH: "var(--warn, #9a6a00)",
  DOUBTED: "var(--warn, #9a6a00)",
  DISTINGUISHED: "var(--warn, #9a6a00)",
  FOLLOWED: "var(--muted, #5b6470)",
};

export function TreatmentList({ lookup }: { lookup: TreatmentLookup }) {
  if (lookup.state === "NOT_APPLICABLE") return null;
  if (lookup.state === "NO_DATA") return <div className="muted" style={{ fontSize: "0.78rem" }}>No installed pack carries treatment data, so how later judgments treated this one could not be checked.</div>;
  if (lookup.state === "NONE_FOUND") {
    return (
      <div style={{ fontSize: "0.78rem" }}>
        <span style={{ fontWeight: 600 }}>No treatment found in installed corpus.</span>
        <span className="muted"> This reflects the installed library only; later decisions outside it are not covered, so check them yourself.</span>
      </div>
    );
  }
  return (
    <div style={{ fontSize: "0.78rem" }}>
      <div className="muted">{lookup.edges.length} later passage{lookup.edges.length === 1 ? "" : "s"} in the installed corpus treat{lookup.edges.length === 1 ? "s" : ""} this judgment. The wording is quoted exactly; read each passage before relying on it.</div>
      <ul style={{ paddingLeft: "1.1rem", margin: "4px 0" }}>
        {lookup.edges.map((e) => (
          <li key={e.edge_id} style={{ marginBottom: 6 }}>
            <span style={{ color: TONE[e.treatment], fontWeight: 600 }}>{LABEL[e.treatment]}</span> — {e.citing_title} ({e.citing_citation}){e.paragraph ? `, ${e.paragraph}` : ""}
            <div style={{ whiteSpace: "pre-wrap" }}>“{e.excerpt}”</div>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** Lazy: the lookup runs when the reader opens it. */
export function TreatmentBlock({ sourceId, chunkId }: { sourceId: string; chunkId: string }) {
  const [lookup, setLookup] = useState<TreatmentLookup | null>(null);
  const [open, setOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!open || lookup) return;
    treatmentLookup([{ source_id: sourceId, chunk_id: chunkId }])
      .then((r) => setLookup(r[0] ?? null))
      .catch((e) => setError(String(e)));
  }, [open, lookup, sourceId, chunkId]);
  return (
    <details onToggle={(e) => setOpen((e.target as HTMLDetailsElement).open)} style={{ marginTop: 2 }}>
      <summary className="muted" style={{ fontSize: "0.75rem", cursor: "pointer" }}>How later judgments treated this one</summary>
      {error && <div className="notice warn">{error}</div>}
      {open && !lookup && !error && <div className="muted" style={{ fontSize: "0.78rem" }}>Checking the installed corpus…</div>}
      {lookup && (lookup.state === "NOT_APPLICABLE" ? <div className="muted" style={{ fontSize: "0.78rem" }}>Treatment applies to judgments; this is not one.</div> : <TreatmentList lookup={lookup} />)}
    </details>
  );
}
