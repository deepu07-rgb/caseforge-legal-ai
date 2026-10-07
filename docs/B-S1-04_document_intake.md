# B-S1-04 — Document intake

## Allowed sources (PRD §5)

| Format | Extension |
|--------|-----------|
| PDF | `.pdf` |
| DOCX | `.docx` |
| JPG | `.jpg`, `.jpeg` |
| PNG | `.png` |

**Out of scope:** Phone Capture Companion, OCR/HTR, auto-type ML (AC-5.1 later).

## Storage

- Originals as **SQLCipher BLOBs** in open case vault (`documents.content`)
- Metadata: `doc_id`, `type_tag`, `language_tag`, `source_format`, `byte_size`
- `page_count` / `per_page_quality_grade` = **null / placeholder** until docpipe

## UX

- Drag-and-drop multi-file
- Choose files / choose folder (WebView2 directory picker)
- Documents list with **editable type tag** (one-click correction)
- Extension-based type stub only (not ML)

## Commands

- `doc_intake_meta`
- `doc_list`
- `doc_ingest` (base64 payload)
- `doc_update_type_tag`

## Verify

```powershell
python eval/checks/doc_intake_b_s1_04.py
npm run build
```
