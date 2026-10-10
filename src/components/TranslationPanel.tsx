import { useCallback, useEffect, useRef, useState } from "react";
import type { DocumentMeta, TranslationEvent, TranslationJob, TranslationRecord } from "../lib/ipc";
import {
  docList,
  translationCancel,
  translationConfirmField,
  translationEvents,
  translationGraph,
  translationGraphGet,
  translationList,
  translationStart,
  translationStatus,
  translationUnconfirmField,
} from "../lib/ipc";

/**
 * B-S4-15: Hindi to English, side by side. The original is always shown beside the English; the English is a machine translation, its
 * wording is not verified, and it is never offered as a quotation. Dates, amounts, section numbers, case numbers and names are set
 * aside before the model sees the text and written back by fixed rules, and each one stays "not confirmed" until the lawyer types the
 * value as the original gives it.
 */
const KIND_LABEL: Record<string, string> = { DATE: "Date", AMOUNT: "Amount", NAME: "Name", SECTION_NUMBER: "Section", CASE_NUMBER: "Case number" };
const LABEL = "machine translation - wording not verified";

function plain(e: unknown): string {
  return (e instanceof Error ? e.message : String(e)).replace(/^CF_[A-Z_]+:\s*/, "");
}

export default function TranslationPanel({ caseId }: { caseId: string }) {
  const [docs, setDocs] = useState<DocumentMeta[]>([]);
  const [docId, setDocId] = useState<string>("");
  const [records, setRecords] = useState<TranslationRecord[]>([]);
  const [events, setEvents] = useState<TranslationEvent[]>([]);
  const [job, setJob] = useState<TranslationJob | null>(null);
  const [typed, setTyped] = useState<Record<string, string>>({});
  const [facts, setFacts] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const timer = useRef<number | null>(null);

  const refresh = useCallback(async () => {
    try {
      setRecords(await translationList(caseId, docId || undefined));
      setEvents(await translationEvents(caseId));
    } catch (e) {
      setError(plain(e));
    }
  }, [caseId, docId]);

  useEffect(() => {
    void (async () => {
      try {
        setDocs(await docList(caseId));
      } catch (e) {
        setError(plain(e));
      }
    })();
  }, [caseId]);
  useEffect(() => {
    void refresh();
  }, [refresh]);
  useEffect(
    () => () => {
      if (timer.current !== null) window.clearInterval(timer.current);
    },
    [],
  );

  const watch = (first: TranslationJob) => {
    setJob(first);
    if (timer.current !== null) window.clearInterval(timer.current);
    timer.current = window.setInterval(async () => {
      try {
        const s = await translationStatus(first.job_id);
        setJob(s);
        if (s.state !== "running") {
          if (timer.current !== null) window.clearInterval(timer.current);
          timer.current = null;
        }
        await refresh();
      } catch (e) {
        setError(plain(e));
        if (timer.current !== null) window.clearInterval(timer.current);
      }
    }, 1500);
  };

  const start = async () => {
    setError(null);
    try {
      watch(await translationStart(caseId, docId || undefined));
    } catch (e) {
      setError(plain(e));
    }
  };
  const stop = async () => {
    if (job) await translationCancel(job.job_id).catch((e) => setError(plain(e)));
  };
  const confirm = async (segmentId: string, fieldId: string) => {
    const key = `${segmentId}/${fieldId}`;
    try {
      await translationConfirmField(caseId, segmentId, fieldId, typed[key] ?? "");
      setTyped((t) => ({ ...t, [key]: "" }));
      await refresh();
    } catch (e) {
      setError(plain(e));
    }
  };
  const withdraw = async (segmentId: string, fieldId: string) => {
    try {
      await translationUnconfirmField(caseId, segmentId, fieldId);
      await refresh();
    } catch (e) {
      setError(plain(e));
    }
  };
  const readFacts = async () => {
    if (!docId) return;
    try {
      setFacts((await translationGraphGet(caseId, docId)) ?? (await translationGraph(caseId, docId)));
    } catch (e) {
      setError(plain(e));
    }
  };

  const running = job?.state === "running";
  const unconfirmedCount = records.reduce((n, r) => n + r.segment.critical_fields.filter((f) => !f.counsel_confirmed).length, 0);
  const flaggedCount = records.filter((r) => r.review.segment_flagged).length;
  const factNodes = ((facts?.nodes as { node_id: string; node_type: string; label: string }[] | undefined) ?? []).slice(0, 40);

  return (
    <div className="card" data-testid="translation-panel">
      <h3>Hindi to English</h3>
      <p className="muted">
        Read the Hindi original and the English side by side. The English is a machine translation: its wording is not verified, and it is never a
        quotation. To quote, quote the Hindi. Dates, amounts, section numbers, case numbers and names stay &quot;not confirmed&quot; until you confirm them against the
        original. Everything runs on this computer.
      </p>

      <div className="row" style={{ alignItems: "center", gap: "0.5rem", flexWrap: "wrap" }}>
        <label>
          Document{" "}
          <select value={docId} onChange={(e) => setDocId(e.target.value)} disabled={running}>
            <option value="">All documents with Hindi text</option>
            {docs.map((d) => (
              <option key={d.doc_id} value={d.doc_id}>
                {d.original_filename}
              </option>
            ))}
          </select>
        </label>
        {!running ? (
          <button type="button" onClick={() => void start()}>
            Translate
          </button>
        ) : (
          <button type="button" onClick={() => void stop()}>
            Stop (progress is kept)
          </button>
        )}
        {docId && records.length > 0 && (
          <button type="button" onClick={() => void readFacts()}>
            Show facts read from this translation
          </button>
        )}
      </div>

      {job && (
        <p className="muted" data-testid="translation-progress">
          {job.state === "running" ? `${job.progress_percent}% — ` : ""}
          {job.message}
          {job.summary ? ` (${job.summary.translated} translated, ${job.summary.resumed} kept from before, ${job.summary.flagged} to check)` : ""}
        </p>
      )}
      {error && <p className="chip warn">{error}</p>}

      {records.length > 0 && (
        <p className="muted">
          {records.length} segments. {flaggedCount > 0 ? `${flaggedCount} need a second look (a number or name was not carried through). ` : ""}
          {unconfirmedCount} field(s) not confirmed yet.
        </p>
      )}

      <div data-testid="translation-segments">
        {records.map(({ segment, review }) => (
          <section key={segment.segment_id} className="card" style={{ marginTop: "0.5rem" }} data-testid="translation-segment">
            <div className="row" style={{ justifyContent: "space-between", gap: "0.5rem" }}>
              <span className="chip muted mono">
                page {segment.page_index + 1} · {segment.segment_id.split(":").pop()}
              </span>
              <span className="chip muted">
                OCR confidence {segment.ocr_confidence.calibrated == null ? "not scored" : segment.ocr_confidence.calibrated.toFixed(2)}
              </span>
            </div>
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "1rem" }}>
              <div>
                <div className="eyebrow">Original</div>
                <p lang="hi" style={{ whiteSpace: "pre-wrap" }} data-testid="translation-original">
                  {segment.source_text}
                </p>
              </div>
              <div>
                <div className="eyebrow">
                  English <span className="chip warn">{LABEL}</span>
                </div>
                <p style={{ whiteSpace: "pre-wrap" }} data-testid="translation-english">
                  {segment.target_text}
                </p>
              </div>
            </div>
            {review.segment_flagged && (
              <p className="chip warn">Check this segment against the original: the translation did not carry a number, date or name through as it should.</p>
            )}
            {segment.critical_fields.length > 0 && (
              <table className="table" style={{ width: "100%" }}>
                <thead>
                  <tr>
                    <th>Kind</th>
                    <th>Original</th>
                    <th>English form</th>
                    <th>Status</th>
                  </tr>
                </thead>
                <tbody>
                  {segment.critical_fields.map((f) => {
                    const key = `${segment.segment_id}/${f.field_id}`;
                    const why = review.field_flags[f.field_id] ?? [];
                    return (
                      <tr key={f.field_id}>
                        <td>{KIND_LABEL[f.field_kind] ?? f.field_kind}</td>
                        <td lang="hi">{f.source_value}</td>
                        <td>
                          {f.translated_value || "—"}
                          {why.map((w) => (
                            <div key={w} className="muted" style={{ fontSize: "0.8rem" }}>
                              {w}
                            </div>
                          ))}
                        </td>
                        <td>
                          {f.counsel_confirmed ? (
                            <>
                              <span className="chip ok">confirmed by you as {f.confirmed_value}</span>{" "}
                              <button type="button" onClick={() => void withdraw(segment.segment_id, f.field_id)}>
                                Withdraw
                              </button>
                            </>
                          ) : (
                            <>
                              <span className="chip warn">not confirmed</span>{" "}
                              <input
                                aria-label={`Value of ${KIND_LABEL[f.field_kind] ?? f.field_kind} as the original gives it`}
                                placeholder="as the original gives it"
                                value={typed[key] ?? ""}
                                onChange={(e) => setTyped((t) => ({ ...t, [key]: e.target.value }))}
                              />{" "}
                              <button type="button" disabled={!(typed[key] ?? "").trim()} onClick={() => void confirm(segment.segment_id, f.field_id)}>
                                Confirm
                              </button>
                            </>
                          )}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
          </section>
        ))}
        {records.length === 0 && !running && <p className="muted">Nothing translated yet. Recognize the scanned pages first, then choose Translate.</p>}
      </div>

      {facts && (
        <section className="card" style={{ marginTop: "0.75rem" }} data-testid="translation-facts">
          <h4>Facts read from this translation</h4>
          <p className="chip warn">from a translation — not confirmed, and not part of your case understanding; check each one against the Hindi original</p>
          <ul>
            {factNodes.map((n) => (
              <li key={n.node_id}>
                <span className="chip muted">{n.node_type}</span> {n.label}
              </li>
            ))}
            {factNodes.length === 0 && <li className="muted">None found.</li>}
          </ul>
        </section>
      )}

      {events.length > 0 && (
        <details style={{ marginTop: "0.75rem" }}>
          <summary>What you confirmed or withdrew ({events.length})</summary>
          <ul>
            {events.map((e) => (
              <li key={`${e.at_unix_ms}-${e.segment_id}-${e.field_id}`}>
                {new Date(e.at_unix_ms).toLocaleString()} — {e.event === "CONFIRM" ? "confirmed" : "withdrew"} {e.source_value}
                {e.event === "CONFIRM" ? ` (machine said ${e.machine_value})` : ""}
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}
