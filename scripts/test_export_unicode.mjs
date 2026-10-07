#!/usr/bin/env node
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "esbuild";

const sample = "Section § 12 — ‘quoted’ → amount ₹1,250; threshold ≤ 5; café";
const dir = await mkdtemp(join(tmpdir(), "caseforge-unicode-"));
try {
  const outfile = join(dir, "exportDraft.mjs");
  await build({ entryPoints: ["src/lib/exportDraft.ts"], bundle: true, platform: "node", format: "esm", outfile });
  const { renderPlainText, renderDocx, renderPdf } = await import(pathToFileURL(outfile).href);
  const draft = {
    template_id: "counsel_memo",
    title: sample,
    body_lines: [sample],
    provenance: {
      export_id: "unicode-test", generated_at: "2026-08-13T00:00:00Z", template_id: "counsel_memo",
      case_id: "unicode", jurisdiction: "India", pipeline_id: "caseforge.draft_export.v1",
      footer_id: "caseforge.export_footer.v1", footer_text: sample,
      g5_fields: ["model_hash", "prompt_hash", "retrieval_ids"], extractor_id: null,
      ai_spans: [], llm_used: false,
    },
    footer_lines: [sample],
  };
  assert.match(renderPlainText(draft), new RegExp(sample.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  const docx = renderDocx(draft);
  assert.ok(Buffer.from(docx).includes(Buffer.from(sample, "utf8")), "DOCX XML must retain exact UTF-8");
  const pdfAscii = Buffer.from(renderPdf(draft)).toString("ascii");
  const actualHex = "FEFF" + [...sample].flatMap((ch) => {
    const code = ch.codePointAt(0);
    if (code <= 0xffff) return [code.toString(16).padStart(4, "0").toUpperCase()];
    const n = code - 0x10000;
    return [(0xd800 + (n >> 10)).toString(16).toUpperCase(), (0xdc00 + (n & 1023)).toString(16).toUpperCase()];
  }).join("");
  assert.ok(pdfAscii.includes(`/ActualText <${actualHex}>`), "PDF must retain exact Unicode in ActualText");
  assert.ok([...pdfAscii].every((ch) => ch.charCodeAt(0) < 128), "PDF writer must keep byte offsets ASCII-safe");
  console.log("PASS B-S3-01 Unicode plain-text/DOCX/PDF round trip");
} finally {
  await rm(dir, { recursive: true, force: true });
}
