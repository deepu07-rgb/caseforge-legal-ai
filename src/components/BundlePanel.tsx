import { useCallback, useEffect, useState } from "react";
import type { BundleExport, BundleInspect, BundlePeer, BundleReceived } from "../lib/ipc";
import {
  bundleExport,
  bundleIdentity,
  bundleImport,
  bundleInspect,
  bundleLanFetch,
  bundleLanShare,
  bundleLanStop,
  bundlePeerAdd,
  bundlePeerList,
  bundlePeerRemove,
  bundlePeerVerify,
  bundleReceivedGet,
  bundleReceivedList,
} from "../lib/ipc";

/**
 * B-S4-16: Encrypted Bundle Exchange. A junior hands a senior a case bundle on a USB stick (or over the local network): encrypted to
 * the senior's key, signed by the junior, with redactions that cannot be undone. Nothing goes to a cloud or a relay. Keys are exchanged
 * as small files and checked by reading a code aloud.
 */
const NAME_KEY = "caseforge.bundle.display_name";
const KIND_LABEL: Record<string, string> = { GRAPH_SUBSET: "Case understanding (parties, dates, facts)", ARGUMENT_TREE: "Argument tree", DOCUMENT: "Documents (the text read from them)", COUNSEL_CORRECTIONS: "My corrections", NOTES: "Notes" };

function plain(e: unknown): string {
  return (e instanceof Error ? e.message : String(e)).replace(/^CF_[A-Z_]+:\s*/, "");
}
function bytesToB64(buf: ArrayBuffer): string {
  let s = "";
  const a = new Uint8Array(buf);
  for (let i = 0; i < a.length; i += 0x8000) s += String.fromCharCode(...a.subarray(i, i + 0x8000));
  return btoa(s);
}
function save(name: string, data: BlobPart, type: string) {
  const url = URL.createObjectURL(new Blob([data], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}
function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

export default function BundlePanel({ caseId, caseTitle }: { caseId: string; caseTitle: string }) {
  const [tab, setTab] = useState<"send" | "receive" | "people">("send");
  const [name, setName] = useState(() => {
    try {
      return localStorage.getItem(NAME_KEY) ?? "";
    } catch {
      return "";
    }
  });
  const [mine, setMine] = useState<{ card: Record<string, unknown>; key_id: string; verification_code: string } | null>(null);
  const [peers, setPeers] = useState<BundlePeer[]>([]);
  const [codes, setCodes] = useState<Record<string, string>>({});
  const [chosen, setChosen] = useState<string[]>([]);
  const [kinds, setKinds] = useState<string[]>(["GRAPH_SUBSET", "ARGUMENT_TREE", "DOCUMENT"]);
  const [notes, setNotes] = useState("");
  const [terms, setTerms] = useState("");
  const [ignoreCase, setIgnoreCase] = useState(true);
  const [built, setBuilt] = useState<BundleExport | null>(null);
  const [lan, setLan] = useState<string | null>(null);
  const [lanUrl, setLanUrl] = useState("");
  const [incoming, setIncoming] = useState<{ b64: string; info: BundleInspect } | null>(null);
  const [acceptUnknown, setAcceptUnknown] = useState(false);
  const [received, setReceived] = useState<BundleReceived["summary"][]>([]);
  const [open, setOpen] = useState<BundleReceived | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setPeers(await bundlePeerList());
      setReceived(await bundleReceivedList(caseId));
    } catch (e) {
      setError(plain(e));
    }
  }, [caseId]);
  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = async (fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await fn();
    } catch (e) {
      setError(plain(e));
    } finally {
      setBusy(false);
    }
  };

  const makeCard = () =>
    run(async () => {
      try {
        localStorage.setItem(NAME_KEY, name);
      } catch {
        /* the name is only a convenience */
      }
      setMine(await bundleIdentity(name));
    });

  const readFile = (f: File, as: "text" | "b64"): Promise<string> =>
    new Promise((res, rej) => {
      const r = new FileReader();
      r.onerror = () => rej(new Error("The file could not be read."));
      if (as === "text") {
        r.onload = () => res(String(r.result));
        r.readAsText(f);
      } else {
        r.onload = () => res(bytesToB64(r.result as ArrayBuffer));
        r.readAsArrayBuffer(f);
      }
    });

  const addCard = (f: File | undefined) =>
    f &&
    run(async () => {
      const p = await bundlePeerAdd(JSON.parse(await readFile(f, "text")));
      setNotice(`Added ${p.card.display_name}. Read the code below to them and type the code they read to you before you send them anything.`);
      await refresh();
    });

  const build = () =>
    run(async () => {
      const redactions = terms.split("\n").map((t) => t.trim()).filter(Boolean).map((text) => ({ text, mode: ignoreCase ? "CASE_INSENSITIVE" : "EXACT" }));
      const extra = notes.trim() ? [{ kind: "NOTES", payload: { text: notes.trim() } }] : [];
      const r = await bundleExport({ caseId, recipientKeyIds: chosen, kinds, redactions, extraItems: extra, displayName: name });
      setBuilt(r);
      setNotice(`Bundle built: ${r.items.length} item(s), ${(r.bytes / 1024).toFixed(0)} KB, encrypted for ${r.recipients.map((x) => x.display_name).join(", ")}.`);
    });

  const inspect = (b64: string) =>
    run(async () => {
      setIncoming({ b64, info: await bundleInspect(b64) });
      setAcceptUnknown(false);
    });

  const importIt = () =>
    incoming &&
    run(async () => {
      const s = await bundleImport(caseId, incoming.b64, acceptUnknown);
      setNotice(`Kept in this matter: ${s.attribution}.`);
      setIncoming(null);
      await refresh();
      setOpen(await bundleReceivedGet(caseId, s.bundle_id));
    });

  const checked = (id: string) => chosen.includes(id);
  const verifiedPeers = peers.filter((p) => p.verified);

  return (
    <div className="card" data-testid="bundle-panel">
      <div className="row" style={{ justifyContent: "space-between", alignItems: "baseline", flexWrap: "wrap", gap: 8 }}>
        <h4>Share this matter with a colleague</h4>
        <div className="tabs">
          {(["send", "receive", "people"] as const).map((t) => (
            <button key={t} type="button" className={`tab ${tab === t ? "active" : ""}`} onClick={() => setTab(t)}>
              {t === "send" ? "Send a bundle" : t === "receive" ? "Receive a bundle" : "People and keys"}
            </button>
          ))}
        </div>
      </div>
      <p className="muted" style={{ fontSize: "0.8rem" }}>
        A bundle is one file, encrypted so that only the person you choose can open it, and signed so they can see it came from you and was not changed. It travels on a USB stick or across your local network. It never goes to a server or the cloud.
      </p>
      {error && <div className="notice warn">{error}</div>}
      {notice && <div className="notice ok">{notice}</div>}

      {tab === "people" && (
        <div>
          <h5>Your card</h5>
          <div className="row" style={{ gap: 6, flexWrap: "wrap" }}>
            <input value={name} onChange={(e) => setName(e.target.value)} placeholder="Your name, as colleagues should see it" style={{ minWidth: 260 }} />
            <button type="button" disabled={busy || !name.trim()} onClick={() => void makeCard()}>Make my card</button>
          </div>
          {mine && (
            <div style={{ marginTop: 6 }}>
              <p>
                Your code: <strong className="mono" data-testid="my-code">{mine.verification_code}</strong>
              </p>
              <p className="muted" style={{ fontSize: "0.8rem" }}>Give the card file to people who will send you bundles, and read them this code (in person or by phone). The card is public; it holds no secret.</p>
              <button type="button" onClick={() => save(`caseforge-card-${mine.key_id}.json`, JSON.stringify(mine.card, null, 2), "application/json")}>Save my card</button>
            </div>
          )}
          <h5 style={{ marginTop: 14 }}>People you exchange bundles with</h5>
          <label className="btn">
            Add someone’s card <input type="file" accept=".json,application/json" style={{ display: "none" }} onChange={(e) => void addCard(e.target.files?.[0])} />
          </label>
          <ul style={{ paddingLeft: 0, listStyle: "none" }}>
            {peers.map((p) => (
              <li key={p.card.key_id} style={{ marginTop: 8 }} data-testid="peer">
                <strong>{p.label}</strong> <span className={`chip ${p.verified ? "ok" : "warn"}`}>{p.verified ? "code checked" : "code not checked"}</span>
                <div className="muted mono" style={{ fontSize: "0.75rem" }}>Their code: {p.verification_code}</div>
                {!p.verified && (
                  <div className="row" style={{ gap: 6, marginTop: 4 }}>
                    <input value={codes[p.card.key_id] ?? ""} placeholder="The code they read to you" onChange={(e) => setCodes({ ...codes, [p.card.key_id]: e.target.value })} style={{ minWidth: 240 }} />
                    <button type="button" disabled={busy} onClick={() => void run(async () => { await bundlePeerVerify(p.card.key_id, codes[p.card.key_id] ?? ""); await refresh(); })}>Confirm</button>
                  </div>
                )}
                <button type="button" style={{ marginTop: 4 }} onClick={() => void run(async () => { await bundlePeerRemove(p.card.key_id); await refresh(); })}>Remove</button>
              </li>
            ))}
            {peers.length === 0 && <li className="muted">No one yet.</li>}
          </ul>
        </div>
      )}

      {tab === "send" && (
        <div>
          <h5>To</h5>
          {verifiedPeers.length === 0 ? (
            <p className="muted">Add a colleague’s card and confirm their code under “People and keys” first. A bundle is only made for people whose code you have checked.</p>
          ) : (
            verifiedPeers.map((p) => (
              <label key={p.card.key_id} style={{ display: "block" }}>
                <input type="checkbox" checked={checked(p.card.key_id)} onChange={(e) => setChosen(e.target.checked ? [...chosen, p.card.key_id] : chosen.filter((x) => x !== p.card.key_id))} /> {p.label}
              </label>
            ))
          )}
          <h5 style={{ marginTop: 10 }}>What to include from “{caseTitle}”</h5>
          {Object.entries(KIND_LABEL).filter(([k]) => k !== "NOTES").map(([k, label]) => (
            <label key={k} style={{ display: "block" }}>
              <input type="checkbox" checked={kinds.includes(k)} onChange={(e) => setKinds(e.target.checked ? [...kinds, k] : kinds.filter((x) => x !== k))} /> {label}
            </label>
          ))}
          <p className="muted" style={{ fontSize: "0.78rem" }}>Documents go as the text CaseForge read from them, not as the original files. Your digest, impeachment kits and audit reports can be added from their own screens in a later version.</p>
          <textarea rows={2} style={{ width: "100%" }} placeholder="A note for the recipient (optional)" value={notes} onChange={(e) => setNotes(e.target.value)} />
          <h5 style={{ marginTop: 10 }}>Hide before sending (cannot be undone in the bundle)</h5>
          <textarea rows={3} style={{ width: "100%" }} placeholder={"One name, number or phrase per line, e.g.\nRamesh Kumar\n98765 43210"} value={terms} onChange={(e) => setTerms(e.target.value)} data-testid="redaction-terms" />
          <label style={{ display: "block", fontSize: "0.82rem" }}>
            <input type="checkbox" checked={ignoreCase} onChange={(e) => setIgnoreCase(e.target.checked)} /> Ignore capital letters
          </label>
          <p className="muted" style={{ fontSize: "0.78rem" }}>
            Each phrase is replaced by [REDACTED] wherever it appears, even split across a line break or with the spaces left out, before anything is encrypted; only a count is kept. If any of it would survive, the bundle is not made. Your own copy of the matter is not changed. Think of other spellings, short forms and numbers too: the check finds the phrases you give it, not people you meant.
          </p>
          <button type="button" className="primary" disabled={busy || chosen.length === 0 || (kinds.length === 0 && !notes.trim()) || !name.trim()} onClick={() => void build()}>Build the bundle</button>
          {!name.trim() && <span className="muted"> (add your name under “People and keys” first)</span>}
          {built && (
            <div style={{ marginTop: 8 }} data-testid="bundle-built">
              <ul style={{ paddingLeft: "1.1rem", fontSize: "0.82rem" }}>
                {built.items.map((i) => (
                  <li key={i.item_id}>{KIND_LABEL[i.kind] ?? i.kind}{i.redacted ? ` — ${built.redaction_log.find((r) => r.item_id === i.item_id)?.redaction_count ?? 0} hidden` : ""}</li>
                ))}
                {built.skipped.map((s) => <li key={s} className="muted">Left out — {s}</li>)}
              </ul>
              <div className="row" style={{ gap: 6, flexWrap: "wrap" }}>
                <button type="button" onClick={() => save(`caseforge-${built.bundle_id}.cfbundle`, b64ToBytes(built.bundle_b64) as BlobPart, "application/octet-stream")}>Save to a USB stick or folder (.cfbundle)</button>
                <button type="button" disabled={busy} onClick={() => void run(async () => { const s = await bundleLanShare(built.bundle_b64); setLan(`Offered once on your local network at ${s.url} — tell your colleague this address. It stops after one download or ${Math.round(s.expires_in_s / 60)} minutes.`); })}>Offer on this network</button>
                {lan && <button type="button" onClick={() => void run(async () => { await bundleLanStop(); setLan(null); })}>Stop offering</button>}
              </div>
              {lan && <div className="notice">{lan}</div>}
            </div>
          )}
        </div>
      )}

      {tab === "receive" && (
        <div>
          <div className="row" style={{ gap: 8, flexWrap: "wrap", alignItems: "center" }}>
            <label className="btn">
              Open a bundle file <input type="file" accept=".cfbundle" style={{ display: "none" }} onChange={(e) => { const f = e.target.files?.[0]; if (f) void readFile(f, "b64").then(inspect).catch((er) => setError(plain(er))); }} />
            </label>
            <input value={lanUrl} onChange={(e) => setLanUrl(e.target.value)} placeholder="or the address shown on your colleague’s computer" style={{ minWidth: 320 }} />
            <button type="button" disabled={busy || !lanUrl.trim()} onClick={() => void run(async () => { await inspect(await bundleLanFetch(lanUrl.trim())); })}>Fetch</button>
          </div>
          {incoming && (
            <div className="notice" data-testid="bundle-incoming" style={{ marginTop: 8 }}>
              <div>
                From <strong>{incoming.info.sender.display_name}</strong> · {incoming.info.created_at.slice(0, 10)} · signature checks out
              </div>
              {incoming.info.sender.verified ? <div className="chip ok">You have checked {incoming.info.sender.known_as}’s code</div> : <div className="chip warn">You have not checked this sender’s code. Anyone can make a bundle for your key; check the code with the person before opening it.</div>}
              {!incoming.info.addressed_to_you && <div className="chip warn">This bundle was not made for your key; you cannot open it.</div>}
              <ul style={{ paddingLeft: "1.1rem", fontSize: "0.82rem" }}>
                {incoming.info.items?.map((i) => <li key={i.item_id}>{KIND_LABEL[i.kind] ?? i.kind}{i.redacted ? " (parts hidden by the sender)" : ""}</li>)}
              </ul>
              {!incoming.info.sender.verified && (
                <label style={{ display: "block" }}>
                  <input type="checkbox" checked={acceptUnknown} onChange={(e) => setAcceptUnknown(e.target.checked)} /> I have checked this sender’s code and want to open it
                </label>
              )}
              <button type="button" className="primary" disabled={busy || !incoming.info.addressed_to_you || (!incoming.info.sender.verified && !acceptUnknown)} onClick={() => void importIt()}>Open and keep in this matter</button>
            </div>
          )}
          <h5 style={{ marginTop: 12 }}>Received in this matter</h5>
          <ul style={{ paddingLeft: "1.1rem" }}>
            {received.map((r) => (
              <li key={r.bundle_id}>
                <button type="button" onClick={() => void run(async () => setOpen(await bundleReceivedGet(caseId, r.bundle_id)))}>{r.attribution}</button> <span className="muted">{r.items.length} item(s)</span>
              </li>
            ))}
            {received.length === 0 && <li className="muted">Nothing received yet.</li>}
          </ul>
          {open && <ReceivedView data={open} />}
        </div>
      )}
    </div>
  );
}

function ReceivedView({ data }: { data: BundleReceived }) {
  return (
    <div data-testid="bundle-received" style={{ marginTop: 8 }}>
      <div className="stat-label">{data.summary.attribution}</div>
      <p className="muted" style={{ fontSize: "0.78rem" }}>Kept beside the matter as received. Nothing here has changed your own documents, graph, tree or corrections.</p>
      {data.items.map((it) => (
        <details key={it.item_id} open={it.kind === "COUNSEL_CORRECTIONS"} style={{ marginBottom: 6 }}>
          <summary>
            {KIND_LABEL[it.kind] ?? it.kind} <span className="muted" style={{ fontSize: "0.75rem" }}>({it.attribution})</span>
          </summary>
          {it.kind === "COUNSEL_CORRECTIONS" && it.plan ? (
            <table style={{ fontSize: "0.8rem", width: "100%" }}>
              <thead><tr><th align="left">What</th><th align="left">Their value</th><th align="left">Yours</th><th align="left">Status</th></tr></thead>
              <tbody>
                {it.plan.map((p, i) => (
                  <tr key={i}>
                    <td className="mono">{p.target_key.replace("cfat:v1:", "")}</td>
                    <td>{p.sender_value ?? "—"}</td>
                    <td>{p.recipient_value ?? "—"}</td>
                    <td>{p.status === "CONFLICTS_WITH_YOUR_CORRECTION" ? <strong>differs from your correction — yours stays</strong> : p.status === "SAME_AS_YOURS" ? "same as yours" : "suggested by them"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <pre style={{ whiteSpace: "pre-wrap", fontSize: "0.78rem", maxHeight: 240, overflow: "auto" }}>{JSON.stringify((it.envelope as { payload: unknown }).payload, null, 2)}</pre>
          )}
        </details>
      ))}
    </div>
  );
}
