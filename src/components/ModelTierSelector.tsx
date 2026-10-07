import { useEffect, useState } from "react";
import { llmSetTier, llmTiers, type LlmStatus, type LlmTiers } from "../lib/ipc";

/**
 * B-S4-00: choose the local model tier for this session.
 * "default" ships in the base install; "addon" is the optional larger model (>= 8 GB RAM).
 * Selecting a tier never downloads anything; an uninstalled or unsupported tier simply
 * reports "not ready" with the sidecar's reason.
 */
export function ModelTierSelector({
  llm,
  onChanged,
}: {
  llm: LlmStatus | null;
  onChanged: (status: LlmStatus) => void;
}) {
  const [catalogue, setCatalogue] = useState<LlmTiers | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    llmTiers()
      .then(setCatalogue)
      .catch((e) => setError(String(e)));
  }, []);

  const active = llm?.tier ?? catalogue?.active ?? "default";

  async function choose(tier: "default" | "addon") {
    setBusy(true);
    setError(null);
    try {
      onChanged(await llmSetTier(tier));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  if (!catalogue) {
    return error ? <div className="muted">Model tiers unavailable: {error}</div> : null;
  }

  return (
    <div style={{ marginTop: "0.85rem" }}>
      <label className="eyebrow" htmlFor="model-tier">
        Model tier
      </label>
      <select
        id="model-tier"
        value={active}
        disabled={busy}
        onChange={(e) => void choose(e.target.value as "default" | "addon")}
      >
        {catalogue.tiers.map((t) => (
          <option key={t.tier} value={t.tier}>
            {t.tier === "default" ? "Standard (base install)" : "Optional add-on"} — {t.model} · {t.parameters} · {t.license}
          </option>
        ))}
      </select>
      {llm?.tier_error && <div className="notice warn" style={{ marginTop: "0.5rem" }}>{llm.tier_error}</div>}
      {error && <div className="notice warn" style={{ marginTop: "0.5rem" }}>{error}</div>}
      <div className="muted" style={{ marginTop: "0.35rem" }}>
        Applies to this session only. Nothing is downloaded automatically.
      </div>
    </div>
  );
}
