/**
 * B-S1-17 Experience Store — local encrypted patterns + top-K injection (PRD §11).
 * Fleet contribution off by default (AC-11.2).
 */

import { useCallback, useEffect, useState } from "react";
import type { ExperienceHit, ExperiencePolicy, ExperienceRecord } from "../lib/ipc";
import {
  experienceDelete,
  experienceList,
  experiencePolicy,
  experienceRequestFleetReview,
  experienceTopK,
  experienceUpsert,
  isTauriRuntime,
} from "../lib/ipc";

const CATEGORIES = [
  "correction",
  "lexicon_entry",
  "draft_edit",
  "rated_exchange",
  "limb_outcome",
  "bench_setting",
] as const;

const STAGE2_CATS = new Set(["limb_outcome", "bench_setting"]);

export default function ExperienceView({
  vaultUnlocked,
  onError,
  onNotice,
}: {
  vaultUnlocked: boolean;
  onError: (msg: string) => void;
  onNotice: (msg: string) => void;
}) {
  const tauri = isTauriRuntime();
  const [policy, setPolicy] = useState<ExperiencePolicy | null>(null);
  const [records, setRecords] = useState<ExperienceRecord[]>([]);
  const [hits, setHits] = useState<ExperienceHit[]>([]);
  const [query, setQuery] = useState("Section 138 correction affidavit statute");
  const [filter, setFilter] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [summary, setSummary] = useState("");
  const [category, setCategory] = useState<(typeof CATEGORIES)[number]>("draft_edit");
  const [tags, setTags] = useState("preparation, draft");

  const refresh = useCallback(async () => {
    if (!tauri) return;
    try {
      setPolicy(await experiencePolicy());
      if (vaultUnlocked) {
        setRecords(await experienceList(filter || null, 100));
      } else {
        setRecords([]);
      }
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    }
  }, [tauri, vaultUnlocked, filter, onError]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const runTopK = async () => {
    if (!vaultUnlocked) {
      onError("Unlock vault to query Experience Store.");
      return;
    }
    setBusy(true);
    try {
      const h = await experienceTopK({ query, topK: policy?.top_k ?? 5 });
      setHits(h);
      onNotice(`Top-K injection candidates: ${h.length} (AC-11.1)`);
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const addRecord = async () => {
    if (!vaultUnlocked) {
      onError("Unlock vault first.");
      return;
    }
    if (!summary.trim()) {
      onError("Summary required.");
      return;
    }
    setBusy(true);
    try {
      const stage = STAGE2_CATS.has(category) ? "S2" : "S1";
      await experienceUpsert({
        record_category: category,
        summary: summary.trim(),
        tags: tags
          .split(",")
          .map((t) => t.trim())
          .filter(Boolean),
        source: "user",
        fleet_status: "local_only",
        stage,
        payload: STAGE2_CATS.has(category)
          ? {
              category,
              note: "Stage 2 Experience Store pattern (B-S2-06)",
            }
          : {},
      });
      setSummary("");
      await refresh();
      onNotice(
        `Experience record saved (${stage} · ${category} · local_only · encrypted)`,
      );
    } catch (e) {
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const requestFleet = async (id: string) => {
    setBusy(true);
    try {
      await experienceRequestFleetReview(id);
      onNotice("Fleet review requested");
      await refresh();
    } catch (e) {
      // Expected when fleet disabled (AC-11.2 default)
      onError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="page-grid experience-root" data-testid="experience-panel">
      <div className="matter-chips">
        <span className="chip matter">Experience Store</span>
        <span className="chip muted">B-S2-06</span>
        {policy && (
          <>
            <span className="chip ok">top-K={policy.top_k}</span>
            <span className={`chip ${policy.fleet_enabled ? "warn" : "ok"}`}>
              fleet={policy.fleet_enabled ? "enabled" : "off"}
            </span>
            <span className="chip muted">
              network_upload={String(policy.fleet_network_upload)}
            </span>
            {(policy.stage2_writable?.length ?? 0) > 0 && (
              <span className="chip ok" data-testid="experience-s2-writable">
                S2: {(policy.stage2_writable || []).join(", ")}
              </span>
            )}
          </>
        )}
        {!vaultUnlocked && <span className="chip warn">Vault locked</span>}
      </div>

      <p className="muted g6-banner" data-testid="experience-g6">
        Local encrypted accumulation only. Retrieval injects top-K past patterns (AC-11.1). Fleet
        contribution is <strong>off by default</strong> and never uploads without explicit review
        (AC-11.2). Stage 2 categories unlocked: <strong>limb_outcome</strong>,{" "}
        <strong>bench_setting</strong>, plus rated exchanges (B-S2-06).
      </p>

      <div className="digest-layout">
        <div className="card">
          <span className="eyebrow">Policy · AC-11.x</span>
          <h3 className="digest-section-title">Store policy</h3>
          {policy ? (
            <ul className="gap-list" data-testid="experience-policy">
              <li>
                pipeline: <code>{policy.pipeline_id}</code>
              </li>
              <li>
                storage: <code>{policy.storage}</code>
              </li>
              <li>top_k injection: {policy.top_k}</li>
              <li>
                fleet_enabled: <strong>{String(policy.fleet_enabled)}</strong> · network_upload:{" "}
                <strong>{String(policy.fleet_network_upload)}</strong>
              </li>
              <li>requires_explicit_review: {String(policy.fleet_requires_explicit_review)}</li>
              <li>stage1 writable: {policy.stage1_writable.join(", ")}</li>
              <li>ACs: {policy.acs.join(", ")}</li>
            </ul>
          ) : (
            <p className="muted">Load policy via shell…</p>
          )}
          <button type="button" onClick={() => void refresh()} disabled={busy}>
            Refresh
          </button>
        </div>

        <div className="card">
          <span className="eyebrow">Top-K · retrieval injection</span>
          <h3 className="digest-section-title">Query past patterns</h3>
          <p className="muted" style={{ fontSize: "0.88rem" }}>
            Token-overlap relevance on <code>relevance_key</code>. Hits are injected into the
            retrieval corpus as <code>experience_store/*</code> chunks.
          </p>
          <textarea
            className="draft-editor"
            rows={3}
            data-testid="experience-query"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <button
            type="button"
            className="primary"
            data-testid="experience-topk"
            style={{ marginTop: "0.65rem" }}
            disabled={busy || !vaultUnlocked}
            onClick={() => void runTopK()}
          >
            Run top-K
          </button>
          <ul className="gap-list" data-testid="experience-hits" style={{ marginTop: "0.75rem" }}>
            {hits.map((h) => (
              <li key={h.record_id}>
                <span className="chip ok">{h.record_category}</span> score=
                {h.score.toFixed(2)} · {h.summary}
                <div className="mono muted" style={{ fontSize: "0.7rem" }}>
                  {h.relevance_key}
                </div>
              </li>
            ))}
            {hits.length === 0 && <li className="muted">No hits yet.</li>}
          </ul>
        </div>
      </div>

      <div className="card">
        <span className="eyebrow">Accumulate · Stage 1</span>
        <h3 className="digest-section-title">Add experience record</h3>
        <div className="lexicon-add">
          <select
            data-testid="experience-category"
            value={category}
            onChange={(e) => setCategory(e.target.value as (typeof CATEGORIES)[number])}
          >
            {CATEGORIES.map((c) => (
              <option key={c} value={c}>
                {c}
              </option>
            ))}
          </select>
          <input
            data-testid="experience-tags"
            placeholder="tags (comma)"
            value={tags}
            onChange={(e) => setTags(e.target.value)}
          />
        </div>
        <textarea
          className="draft-editor"
          rows={3}
          data-testid="experience-summary"
          placeholder="Pattern summary for future retrieval injection"
          value={summary}
          onChange={(e) => setSummary(e.target.value)}
        />
        <button
          type="button"
          className="primary"
          data-testid="experience-add"
          style={{ marginTop: "0.5rem" }}
          disabled={busy || !vaultUnlocked}
          onClick={() => void addRecord()}
        >
          Save local_only
        </button>
      </div>

      <div className="card" data-testid="experience-list-panel">
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
          <h4>Stored records</h4>
          <select
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            data-testid="experience-filter"
          >
            <option value="">all</option>
            {CATEGORIES.map((c) => (
              <option key={c} value={c}>
                {c}
              </option>
            ))}
          </select>
        </div>
        <ul className="gap-list lexicon-list" data-testid="experience-list">
          {records.map((r) => (
            <li key={r.record_id}>
              <span className="chip muted">{r.record_category}</span>{" "}
              <span className={`chip ${r.fleet_status === "local_only" ? "ok" : "warn"}`}>
                {r.fleet_status}
              </span>{" "}
              <strong>{r.summary}</strong>
              <div className="mono muted" style={{ fontSize: "0.7rem" }}>
                {r.relevance_key}
              </div>
              <button
                type="button"
                className="linkish"
                disabled={!vaultUnlocked || busy}
                onClick={() => {
                  const confirmed = window.confirm(
                    `Delete this local experience record?\n\n${r.summary}\n\nThis does not alter matter evidence, but the experience record cannot be recovered after deletion.`,
                  );
                  if (!confirmed) return;
                  void experienceDelete(r.record_id)
                    .then(async () => {
                      await refresh();
                      onNotice("Local experience record permanently deleted. Matter evidence was not changed.");
                    })
                    .catch((err) => onError(err instanceof Error ? err.message : String(err)));
                }}
              >
                delete
              </button>{" "}
              <button
                type="button"
                className="linkish"
                data-testid="experience-fleet-request"
                disabled={!vaultUnlocked || busy}
                onClick={() => void requestFleet(r.record_id)}
                title="AC-11.2: fails while fleet is disabled (default)"
              >
                request fleet review
              </button>
            </li>
          ))}
          {records.length === 0 && (
            <li className="muted">
              {vaultUnlocked
                ? "No records yet — corrections and lexicon saves dual-write here automatically."
                : "Unlock vault to load Experience Store."}
            </li>
          )}
        </ul>
      </div>
    </div>
  );
}
