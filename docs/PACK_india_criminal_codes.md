# Official pack: India Criminal Codes (`caseforge.india.criminal-codes`)

**Built 2026-10-08 · akpkg v2.1 · signed with the development key `caseforge-official-dev-2026` · 16 MB.**

Section-level official text of six Acts:
- the repealed Indian Penal Code, Code of Criminal Procedure and Indian Evidence Act;
- their successors, the Bharatiya Nyaya Sanhita, Bharatiya Nagarik Suraksha Sanhita and Bharatiya Sakshya Adhiniyam;
- plus a STATUTE_SUCCESSION rule table for the Code Bridge.

## Source

All text comes from **India Code**, the Ministry of Law and Justice repository, which has moved to `indiacode.gov.in`. It was crawled with `eval/tools/indiacode_crawl.py` through the site's official DSpace REST API. Each Act's record URL, official PDF and PDF SHA-256 are recorded in `metadata/source_ledger.json`.

| Act | India Code record | Text basis | Sections |
|---|---|---|---|
| Bharatiya Nyaya Sanhita, 2023 | 123456789/496548 | India Code section records | 358 |
| Bharatiya Nagarik Suraksha Sanhita, 2023 | 123456789/496550 | India Code section records | 531 |
| Bharatiya Sakshya Adhiniyam, 2023 | 123456789/496549 | India Code section records | 170 |
| Indian Penal Code, 1860 (repealed) | 123456789/488475 (central "Rep." record) | official PDF | 582 |
| Code of Criminal Procedure, 1973 (repealed) | 123456789/600755 | official PDF | 532 |
| Indian Evidence Act, 1872 (repealed) | 123456789/488783 (central "Rep." record) | official PDF | 184 |

How the repealed codes were sourced:
- **No section records.** India Code keeps no section records for the repealed codes, so their official PDFs are parsed. Footnotes are cut at the footnote rule, superscript markers and the watermark are dropped by font size, small capitals are kept, and section headings are checked for strictly increasing numbers.
- **IPC and CrPC:** every section was found.
- **Evidence Act:** only s.86 is not split out, because its heading has no dash in the official PDF. Its text is inside the s.85C chunk.
- **CrPC PDF:** the repealed-CrPC central record holds a poor scan, so the CrPC text comes from India Code's consolidated PDF (Word source "A1974-2"). That file is listed under Tripura; a Chhattisgarh-listed copy has identical text on all 237 pages.

Each section is stored verbatim, prefixed with a CaseForge label such as `[IPC s.302]`. Repealed Acts are marked `SUPERSEDED` with `effective_to` 2024-06-30. They still govern older cases, and the Code Bridge says which code applies.

## Succession table — DEVELOPMENT_ONLY

`metadata/rule_tables/india_criminal_succession.json` holds 1,208 rows. **None is signed.**

- **3 TRANSITION rows** for IPC→BNS, CrPC→BNSS and IEA→BSA, switching on 2024-07-01.
  - The basis date for IPC→BNS is the offence date. For CrPC→BNSS and IEA→BSA it is the date the proceeding was instituted, used as a proxy for "pending on 1 July 2024".
  - Each row cites the repeal-and-savings section of the new code: BNS 358, BNSS 531, BSA 170.
- **1,205 MAPPING candidates** from `eval/tools/succession_matcher.py`. The matcher measures how much of each old section's wording survives in the new code, and also identifies the receiving sub-section.
  - By class: 533 renumbered, 616 modified, 40 omitted, 14 new, 2 identical.
  - **107 grey-zone pairs are not emitted** as rows. They appear only in the editor worksheet.
- **Spot check against 26 mappings known independently:**
  - 22 agree, for example IPC 302→BNS 103(1), IPC 420→BNS 318(4), CrPC 438→BNSS 482, IEA 65B→BSA 63.
  - The other 4 were held back for review rather than emitted: IPC 498A, 124A, 377 and 497.
  - No emitted row in the sample is wrong.

**Editor worksheet:** `docs/worksheets/Q-S4-02_criminal_succession_candidates.csv`, with scores and empty decision columns. A named legal editor must confirm or correct each row before it is signed.

The pack also ships `metadata/code_aliases.json`, so "Code of Criminal Procedure" matches CrPC and "Evidence Act" matches IEA.

## Before client release

1. **Legal-editor review and signature** of the succession rows (Q-S4-02).
2. **Production signing key**, replacing the development key.
3. **Commencement check:** confirm the commencement notifications, and any provision brought into force on a different date (for example BNS s.106(2)), against the Gazette.

## Rebuild

```
python eval/tools/indiacode_crawl.py <dir> --handles 123456789/496548 123456789/496550 123456789/496549 123456789/488475 123456789/600755 123456789/488783
python eval/tools/build_criminal_codes_pack.py --crawl <dir> --key <official_dev.sk.pem>
```
