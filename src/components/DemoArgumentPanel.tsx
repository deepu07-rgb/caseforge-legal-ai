import { useMemo, useState } from "react";
import {
  DEMO_PACKS,
  scoreTree,
  type DemoPack,
  type LinkScores,
  type ScoredLimb,
} from "../lib/demoArgumentIntegrity";

const LINK_ORDER: (keyof LinkScores)[] = ["Fact", "Evidence", "Law", "Exposure"];

function viChipClass(vi: number): string {
  if (vi <= 25) return "chip ok";
  if (vi <= 50) return "chip";
  return "chip warn";
}

function LimbCard({ limb, threshold }: { limb: ScoredLimb; threshold: number }) {
  return (
    <article className="limb-card">
      <div className="limb-head">
        <div>
          <div className="limb-id">{limb.node_id}</div>
          <h4 className="limb-label">{limb.label}</h4>
        </div>
        <span className={viChipClass(limb.vi)} title="Vulnerability Index (higher = weaker)">
          VI <strong>{limb.vi}</strong>
          {limb.viMatchesExpected ? (
            <span className="muted"> = expected</span>
          ) : (
            <span className="muted"> · exp {limb.expected_VI}</span>
          )}
        </span>
      </div>

      {limb.theory && <p className="theory">{limb.theory}</p>}

      <div className="table-wrap">
        <table className="table mono">
          <thead>
            <tr>
              <th>Link</th>
              <th>Score</th>
              <th>Bar</th>
              <th>Missing</th>
            </tr>
          </thead>
          <tbody>
            {LINK_ORDER.map((k) => {
              const score = limb.links[k];
              const isMin = k === limb.minLink;
              const missing = score < threshold;
              return (
                <tr key={k}>
                  <td>
                    {k}
                    {isMin ? " ← min" : ""}
                  </td>
                  <td>{score}</td>
                  <td style={{ minWidth: 120 }}>
                    <div className="vi-meter-wrap">
                      <div className="meter" aria-hidden>
                        <span
                          style={{
                            width: `${score}%`,
                            background: missing ? "var(--amber)" : undefined,
                          }}
                        />
                      </div>
                    </div>
                  </td>
                  <td>{missing ? <span className="chip warn">&lt;{threshold}</span> : "—"}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      {limb.strength_assessment?.overall && (
        <p className="theory" style={{ marginTop: "0.85rem", marginBottom: 0 }}>
          <strong style={{ color: "var(--ink)", fontFamily: "var(--font-ui)", fontSize: "0.78rem", letterSpacing: "0.06em", textTransform: "uppercase" }}>
            {limb.strength_assessment.overall}
          </strong>
          {limb.strength_assessment.why ? ` — ${limb.strength_assessment.why}` : null}
        </p>
      )}

      {(limb.strength_assessment?.where_argument_needs_more_strength?.length ?? 0) > 0 && (
        <div className="strengthen-box">
          <strong>Where the argument needs more strength</strong>
          <ul>
            {limb.strength_assessment!.where_argument_needs_more_strength!.map((g, i) => (
              <li key={`${g.link}-${i}`}>
                <span className="mono">
                  {g.link} ({g.current_score}, {g.gap})
                </span>
                : {g.action}
              </li>
            ))}
          </ul>
        </div>
      )}
    </article>
  );
}

export default function DemoArgumentPanel() {
  const [packId, setPackId] = useState(DEMO_PACKS[0].id);
  const pack: DemoPack = DEMO_PACKS.find((p) => p.id === packId) ?? DEMO_PACKS[0];
  const scored = useMemo(() => scoreTree(pack.tree), [pack]);

  return (
    <div className="page-grid">
      <div className="card hero-card" style={{ gridTemplateColumns: "1fr" }}>
        <div>
          <div className="eyebrow">Integrity lab</div>
          <h3>Argument integrity drills</h3>
          <p>
            Three synthetic practice matters (easy / medium / hard). Formula:{" "}
            <span className="mono">VI = 100 − min(Fact, Evidence, Law, Exposure)</span>. Pedagogy
            only — not GOLDEN, not law, does not unlock B-S1-06.
          </p>
        </div>
      </div>

      <div className="lab-layout">
        <aside className="page-grid" style={{ gap: "0.85rem" }}>
          <div className="card">
            <div className="stat-label" style={{ marginBottom: "0.85rem" }}>
              Practice packs
            </div>
            {DEMO_PACKS.map((p) => (
              <button
                key={p.id}
                type="button"
                className={`lab-pack-card ${p.id === packId ? "active" : ""}`}
                onClick={() => setPackId(p.id)}
              >
                <span className="diff">{p.meta.difficulty}</span>
                <span className="title">{p.meta.label}</span>
                <span className="meta-line">{p.meta.jurisdiction}</span>
              </button>
            ))}
          </div>

          <div className="card">
            <div className="stat-label" style={{ marginBottom: "0.65rem" }}>
              Matter snapshot
            </div>
            <table className="table">
              <tbody>
                <tr>
                  <th>Case</th>
                  <td>{pack.meta.label}</td>
                </tr>
                <tr>
                  <th>Jurisdiction</th>
                  <td>
                    <span className="chip matter">{pack.meta.jurisdiction}</span>
                  </td>
                </tr>
                <tr>
                  <th>Relief</th>
                  <td>{pack.tree.relief_root.label}</td>
                </tr>
                <tr>
                  <th>Weakest</th>
                  <td>
                    {scored.weakest ? (
                      <>
                        {scored.weakest.node_id} · VI <strong>{scored.weakest.vi}</strong>
                      </>
                    ) : (
                      "—"
                    )}
                  </td>
                </tr>
                <tr>
                  <th>Packet</th>
                  <td className="mono" style={{ fontSize: "0.72rem", wordBreak: "break-all" }}>
                    {pack.packetPath}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>

          <div className="card">
            <div className="stat-label" style={{ marginBottom: "0.55rem" }}>
              Teaching goals
            </div>
            <ul
              style={{
                margin: 0,
                paddingLeft: "1.1rem",
                fontFamily: "var(--font-reading)",
                color: "var(--ink-muted)",
                fontSize: "0.92rem",
                lineHeight: 1.5,
              }}
            >
              {pack.meta.primary_teaching.map((t) => (
                <li key={t}>{t}</li>
              ))}
            </ul>
          </div>
        </aside>

        <div className="page-grid" style={{ gap: "0.85rem" }}>
          <div className="completeness">
            <div className="completeness-head">
              <div className="stat-label">Drill coverage (integrity)</div>
              <span className={viChipClass(scored.weakest?.vi ?? 100)}>
                Weakest VI {scored.weakest?.vi ?? "—"}
              </span>
            </div>
            <div className="meter" style={{ height: 3 }}>
              <span
                style={{
                  width: `${Math.max(8, 100 - (scored.weakest?.vi ?? 0))}%`,
                }}
              />
            </div>
            <p className="muted" style={{ margin: "0.65rem 0 0", fontSize: "0.88rem" }}>
              Higher VI = weaker limb. Missing link threshold:{" "}
              <span className="mono">{scored.threshold}</span>
            </p>
          </div>

          {scored.limbs.map((limb) => (
            <LimbCard key={limb.node_id} limb={limb} threshold={scored.threshold} />
          ))}

          <div className="system-pulse-bar">
            <span>{pack.meta.disclaimer}</span>
            <span className="mono">npm run demo:score</span>
          </div>
        </div>
      </div>
    </div>
  );
}
