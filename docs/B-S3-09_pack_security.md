# B-S3-09 — Knowledge-pack security

Audit finding #19 was reachable through `pack_install`: a user-selected directory supplied a
manifest whose file paths were joined to the source root, an existing destination was deleted before
the replacement was fully usable, and official encryption failures were treated as warnings. The
security invariant is now: no untrusted path may escape the validated pack tree, no pack may elevate
its trust domain or introduce active content, and no working version is replaced until the candidate
is fully validated, protected, and integrity-checked.

The installer accepts extracted directories only. Archive transport is explicitly unsupported until
an extraction/containment contract is frozen. It applies deny-unknown-field runtime schemas,
semantic versions, strict pack IDs, forward-slash relative paths, canonical-root containment, and
Windows device/ADS rules. Absolute, traversal, alternate-separator, symlink, junction/reparse,
unlisted, duplicate, unsupported, active HTML/script, and unexpected filesystem entries fail closed.

Limits are 4,096 listed files, 64 MiB per file, 512 MiB total, 16 path levels, and a 1 MiB manifest.
Only passive `content/*.txt` and declared `metadata/*.json` are supported. Metadata is parsed as
data; retrieval snippets are React text nodes and are never rendered with raw HTML or executed.

Trust is enforced twice: manifest/source/chunk consistency prevents custom material from carrying
`OFFICIAL_VERIFIED`, while installed registries reject official/custom pack-ID crossover and source
ID collisions. Official signature, full file hashes, metadata lineage, headnote scan, protection,
protected asset counts, ciphertext integrity, and decrypt smoke checks must all pass.

Install and upgrade use a same-volume staging directory. The candidate is copied without following
reparse points, revalidated, protected, and verified. Only then is the previous directory renamed to
a backup and the candidate activated. Registry writes are atomic. Any activation or registry failure
restores the prior directory and registry; the backup is removed only after commit.

The cumulative fixture inventory is `packs/security-fixtures/b_s3_09_cases.json`; the real-boundary
Rust regressions generate isolated temporary pack directories. Evidence is written to
`eval/reports/b_s3_09_pack_security.json`.

Verification on 2026-08-14: all five B-S3-09 acceptance gates passed across 13 cumulative fixture
classes. Six real-boundary security tests (including legitimate signed official install, forced
protection rollback, failed custom upgrade rollback, and Windows junction rejection) passed. The
full Rust library suite passed 58/58, the production frontend build passed, and B-S3-08, B-S3-07,
retrieval, and confidence regressions stayed green. The workspace measured 3.286 GiB; with the
shared 4.361 GiB Ollama store, the development footprint was 7.648 GiB.
