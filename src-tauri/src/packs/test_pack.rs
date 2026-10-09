//! Test support and end-to-end tests for packs that carry judgments, treatment edges and currency dates (B-S4-13).
//! Builds small CUSTOM packs (no signature) that pass the real installer, so install, delta, store, treatment lookup and
//! the citation audit are exercised for real.

use super::*;
use serde_json::{json, Value};

pub struct TSource {
    pub title: &'static str,
    pub citation: &'static str,
    pub decision_date: &'static str,
    /// (paragraph number, text)
    pub paragraphs: Vec<(&'static str, String)>,
}

pub struct Built {
    pub dir: PathBuf,
    pub pack_id: String,
    pub version: String,
    pub content_sha256: String,
    pub source_ids: Vec<String>,
    /// chunk ids per source, in paragraph order
    pub chunk_ids: Vec<Vec<String>>,
    /// chunk texts per source, in paragraph order
    pub chunk_texts: Vec<Vec<String>>,
}

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn pretty(v: &Value) -> Vec<u8> {
    (serde_json::to_string_pretty(v).unwrap() + "\n").into_bytes()
}

/// Writes a complete pack. `edges` gets the ids once they are known and returns the treatment edges (None: no edges
/// file). `currency` None: the manifest states no currency date.
pub fn write_pack(dir: &Path, pack_id: &str, version: &str, currency: Option<&str>, sources: &[TSource], edges: &dyn Fn(&Built) -> Option<Vec<Value>>) -> Built {
    let mut files: Vec<PackFileEntry> = Vec::new();
    fn entry(dir: &Path, files: &mut Vec<PackFileEntry>, rel: &str, bytes: &[u8], role: &str) {
        write(&dir.join(rel), bytes);
        files.push(PackFileEntry { path: rel.into(), sha256: sha256_bytes(bytes), bytes: Some(bytes.len() as u64), role: Some(role.into()) });
    }
    let mut texts = Vec::new();
    for (i, s) in sources.iter().enumerate() {
        let blocks: Vec<String> = s.paragraphs.iter().map(|(n, t)| format!("[para {n}]\n{t}")).collect();
        let full = format!("[{} | {}]\n{}", s.title, s.citation, blocks.join("\n\n"));
        entry(dir, &mut files, &format!("content/j{i}.txt"), (full.clone() + "\n").as_bytes(), "AUTHORITATIVE_CONTENT");
        texts.push((full, blocks));
    }
    let authoritative = authoritative_hash(&files);
    let common = |s: &TSource| {
        json!({
            "schema_version": "2.0.0", "title": s.title, "jurisdiction": "India", "jurisdiction_path": ["India", "Supreme Court"], "authority_type": "JUDGMENT",
            "issuing_authority": "Supreme Court of India", "citation": s.citation, "effective_from": s.decision_date, "effective_to": "UNKNOWN", "version": "test",
            "amendment": "NONE", "pack_id": pack_id, "trust_level": "CUSTOM", "superseded_status": "UNKNOWN", "publisher": "test", "source_reference": "test",
            "content_kind": "AUTHORITATIVE_TEXT", "derived_from_source_id": null, "court": "Supreme Court of India", "case_name": s.title, "case_number": s.citation,
            "decision_date": s.decision_date, "precedential_status": "UNKNOWN", "holding_tags": [], "proposition_tags": [],
        })
    };
    let (mut src_json, mut chunk_json) = (Vec::new(), Vec::new());
    let (mut source_ids, mut chunk_ids, mut chunk_texts) = (Vec::new(), Vec::new(), Vec::new());
    for (i, s) in sources.iter().enumerate() {
        let (full, blocks) = &texts[i];
        let checksum = prefixed_sha256(full);
        let sid = expected_source_id(pack_id, &checksum).unwrap();
        let mut rec = common(s);
        let o = rec.as_object_mut().unwrap();
        for (k, v) in [("source_id", json!(sid)), ("source_file", json!(format!("content/j{i}.txt"))), ("page", json!(1)), ("paragraph", json!("whole judgment")), ("paragraph_number", json!("all")),
                       ("section", json!("judgment")), ("heading_path", json!([s.title])), ("exact_text", json!(full)), ("checksum", json!(checksum))] {
            o.insert(k.into(), v);
        }
        src_json.push(rec);
        let (mut ids, mut tx) = (Vec::new(), Vec::new());
        for ((n, _), block) in s.paragraphs.iter().zip(blocks) {
            let ck = prefixed_sha256(block);
            let cid = expected_chunk_id(pack_id, &sid, &ck);
            let mut c = common(s);
            let o = c.as_object_mut().unwrap();
            for (k, v) in [("source_id", json!(sid)), ("chunk_id", json!(cid)), ("source_file", json!(format!("content/j{i}.txt"))), ("page", json!(1)), ("paragraph", json!(format!("para {n}"))),
                           ("paragraph_number", json!(n)), ("section", json!(format!("para {n}"))), ("heading_path", json!([s.title, format!("para {n}")])), ("exact_text", json!(block)),
                           ("checksum", json!(ck)), ("source_checksum", json!(checksum)), ("pack_checksum", json!(authoritative)), ("temporal_status", json!("APPLICABLE"))] {
                o.insert(k.into(), v);
            }
            chunk_json.push(c);
            ids.push(cid);
            tx.push(block.clone());
        }
        source_ids.push(sid);
        chunk_ids.push(ids);
        chunk_texts.push(tx);
    }
    entry(dir, &mut files, "metadata/legal_sources.json", &pretty(&Value::Array(src_json)), "LEGAL_SOURCES");
    entry(dir, &mut files, "metadata/legal_chunks.json", &pretty(&Value::Array(chunk_json)), "LEGAL_CHUNKS");
    let mut built = Built { dir: dir.to_path_buf(), pack_id: pack_id.into(), version: version.into(), content_sha256: String::new(), source_ids, chunk_ids, chunk_texts };
    let edge_list = edges(&built);
    if let Some(list) = &edge_list {
        entry(dir, &mut files, super::super::treatment::EDGES_FILE, &pretty(&Value::Array(list.clone())), "TREATMENT_EDGES");
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    built.content_sha256 = canonical_hash(&files);
    let mut man = json!({
        "schema_version": "2.1.0", "metadata_version": "2.0.0", "pack_id": pack_id, "pack_kind": "custom", "jurisdiction": "India", "jurisdiction_path": ["India", "Supreme Court"],
        "version": version, "display_name": format!("Test pack {pack_id}"), "description": "test", "content_sha256": built.content_sha256, "authoritative_content_sha256": authoritative,
        "files": files, "headnotes_stripped": false, "trust_level": "CUSTOM", "controlled_beta_eligible": false, "sources_file": "metadata/legal_sources.json",
        "chunks_file": "metadata/legal_chunks.json", "publisher": "test", "updated_at": "2026-04-01", "source_status": "CURRENT", "supersedes_version": null, "superseded_by_version": null,
        "editorial_review_status": "NONE", "rights_basis": "test",
    });
    if let Some(c) = currency {
        man["currency_date"] = json!(c);
    }
    if edge_list.is_some() {
        man["treatment_edges_file"] = json!(super::super::treatment::EDGES_FILE);
    }
    write(&dir.join("manifest.json"), &pretty(&man));
    built
}

/// Makes a delta of `full` against `base`: the same manifest, naming the base, with only the files that differ.
pub fn make_delta(full: &Built, base: &Built, out: &Path) {
    let man: Value = serde_json::from_str(&fs::read_to_string(full.dir.join("manifest.json")).unwrap()).unwrap();
    let base_man: Value = serde_json::from_str(&fs::read_to_string(base.dir.join("manifest.json")).unwrap()).unwrap();
    let base_hash = |p: &str| base_man["files"].as_array().unwrap().iter().find(|f| f["path"] == p).map(|f| f["sha256"].clone());
    for f in man["files"].as_array().unwrap() {
        let p = f["path"].as_str().unwrap();
        if base_hash(p).as_ref() != Some(&f["sha256"]) {
            write(&out.join(p), &fs::read(full.dir.join(p)).unwrap());
        }
    }
    let mut m = man.clone();
    m["delta"] = json!({"base_version": base.version, "base_content_sha256": base.content_sha256});
    write(&out.join("manifest.json"), &pretty(&m));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::treatment::{self, TreatmentSet};

    fn vault() -> (tempfile::TempDir, VaultPaths) {
        let d = tempfile::tempdir().unwrap();
        let p = VaultPaths::new(d.path().to_path_buf());
        (d, p)
    }

    fn judgments() -> Vec<TSource> {
        vec![
            TSource { title: "Alpha v. Beta", citation: "(2001) 1 SCC 100", decision_date: "2001-02-01", paragraphs: vec![("1", "The tenant remained in possession after the lease ended and the landlord sued for eviction.".into()), ("2", "The claim is allowed.".into())] },
            TSource { title: "Rho v. Sigma", citation: "(2012) 5 SCC 200", decision_date: "2012-05-01", paragraphs: vec![("11", "The facts are not in dispute.".into()), ("12", "We have considered the submissions. The view taken in Alpha v. Beta (2001) 1 SCC 100 is hereby overruled. Accordingly the appeal is allowed.".into())] },
            TSource { title: "Tau v. Upsilon", citation: "(2014) 2 SCC 300", decision_date: "2014-03-01", paragraphs: vec![("3", "The decision in Alpha v. Beta (2001) 1 SCC 100 has not been overruled and applies here.".into())] },
        ]
    }

    fn excerpt_edge(b: &Built, citing: usize, cited: usize, para: usize, treatment: &str, phrase_start: &str, phrase_end: &str, id: &str, pattern: &str) -> Value {
        let text = &b.chunk_texts[citing][para];
        let from = text.find(phrase_start).unwrap();
        let to = text.find(phrase_end).unwrap() + phrase_end.len();
        let excerpt = &text[from..to];
        let cs = text[..from].chars().count();
        json!({
            "schema_version": "1.0.0", "edge_id": id,
            "citing": {"source_id": b.source_ids[citing], "chunk_id": b.chunk_ids[citing][para], "paragraph": format!("para {}", ["11", "12"][para.min(1)]), "char_start": cs, "char_end": cs + excerpt.chars().count(), "excerpt": excerpt},
            "cited_source_id": b.source_ids[cited], "treatment": treatment, "detection": {"method": "PATTERN_PLUS_VERIFIER", "pattern_id": pattern},
            "content_kind": "DERIVED", "affects_supersession": false, "pack_build": {"pack_id": b.pack_id, "version": b.version, "built_at": "2026-04-01T00:00:00Z"},
        })
    }

    fn edges_for(b: &Built) -> Option<Vec<Value>> {
        Some(vec![
            excerpt_edge(b, 1, 0, 1, "OVERRULED", "The view taken", "is hereby overruled.", "e-overrule", "overruled.v2"),
            // a stored edge that claims treatment its excerpt does not support (negated): must never be shown
            {
                let mut e = excerpt_edge(b, 2, 0, 0, "OVERRULED", "The decision in Alpha", "has not been overruled", "e-negated", "overruled.v2");
                e["citing"]["paragraph"] = json!("para 3");
                e
            },
        ])
    }

    #[test]
    fn a_planted_overruling_surfaces_on_the_overruled_citation_with_the_exact_citing_paragraph() {
        let (_v, paths) = vault();
        let src = tempfile::tempdir().unwrap();
        let built = write_pack(src.path(), "test.judgments", "1.0.0", Some("2026-03-31"), &judgments(), &edges_for);
        install_pack_from_dir(&paths, src.path()).expect("the pack installs");
        let store = crate::retrieval::store::open(&paths).unwrap();
        let set = TreatmentSet::load(&paths);
        assert_eq!(set.scanned_packs, vec!["test.judgments".to_string()]);
        // lookup: only the verified edge; the negated one is rejected
        let l = set.lookup(&built.source_ids[0], "JUDGMENT", &|id| store.get_by_chunk_id(id));
        assert_eq!((l.state, l.edges.len()), ("FOUND", 1), "{:?}", l.rejected);
        assert_eq!(l.rejected, vec![("e-negated".to_string(), "NEGATED_OR_ARGUED".to_string())]);
        let e = &l.edges[0];
        assert_eq!((e.treatment.as_str(), e.citing_title.as_str(), e.paragraph.as_deref()), ("OVERRULED", "Rho v. Sigma", Some("para 12")));
        let chunk = store.get_by_chunk_id(&e.citing_chunk_id).unwrap();
        assert_eq!(chunk.text.chars().skip(e.char_start).take(e.char_end - e.char_start).collect::<String>(), e.excerpt);
        // the audit of a draft that cites the overruled case
        let draft = "The tenant relies on Alpha v. Beta (2001) 1 SCC 100 for the rule that possession after expiry is lawful.";
        let mut report = crate::audit::audit_with_store(&store, &[], draft, "draft.txt", "txt", None).unwrap();
        let before = report.items.iter().find(|i| i.kind == "CITATION").unwrap().result.clone();
        assert_eq!(before, "CUSTOM_SOURCE", "a custom pack is never an official verification: {:?}", report.items);
        treatment::decorate_report(&paths, &store, &mut report);
        let item = report.items.iter().find(|i| i.kind == "CITATION").unwrap();
        assert_eq!(item.result, "CUSTOM_SOURCE", "treatment adds a note but never changes the standing of the citation");
        assert_eq!(item.notes.len(), 1);
        assert!(item.notes[0].text.starts_with("Overruled in the installed corpus: Rho v. Sigma ((2012) 5 SCC 200), para 12:") && item.notes[0].text.contains("is hereby overruled."), "{}", item.notes[0].text);
        // AC-S4-08.3: the audit's pack snapshot carries the pack's currency date
        assert_eq!(report.pack_snapshot[0].currency_date.as_deref(), Some("2026-03-31"));
        // a citation of a judgment nobody treats stays VERIFIED, with "no treatment found" on lookup
        let l = set.lookup(&built.source_ids[2], "JUDGMENT", &|id| store.get_by_chunk_id(id));
        assert_eq!((l.state, l.edges.len()), ("NONE_FOUND", 0));
    }

    #[test]
    fn install_refuses_malformed_treatment_edges_and_a_missing_currency_date() {
        let build = |currency: Option<&str>, tweak: &dyn Fn(&mut Vec<Value>)| {
            let src = tempfile::tempdir().unwrap();
            write_pack(src.path(), "test.judgments", "1.0.0", currency, &judgments(), &|b| {
                let mut e = edges_for(b).unwrap();
                tweak(&mut e);
                Some(e)
            });
            let (_v, paths) = vault();
            install_pack_from_dir(&paths, src.path()).err()
        };
        assert!(build(Some("2026-03-31"), &|_| {}).is_none());
        assert!(build(None, &|_| {}).unwrap().starts_with("CF_PACK_TREATMENT_INVALID"), "a pack with treatment edges must say how current it is");
        let e = build(Some("2026-03-31"), &|e| e[0]["affects_supersession"] = json!(true)).unwrap();
        assert!(e.starts_with("CF_PACK_TREATMENT_INVALID"), "{e}");
        let e = build(Some("2026-03-31"), &|e| e[0]["content_kind"] = json!("AUTHORITATIVE_TEXT")).unwrap();
        assert!(e.starts_with("CF_PACK_TREATMENT_INVALID"), "{e}");
        let e = build(Some("2026-03-31"), &|e| e[1]["treatment"] = json!("GOOD_LAW")).unwrap();
        assert!(e.starts_with("CF_PACK_TREATMENT_INVALID"), "{e}");
    }

    #[test]
    fn an_edge_whose_excerpt_was_altered_after_the_build_is_not_shown() {
        let (_v, paths) = vault();
        let src = tempfile::tempdir().unwrap();
        let built = write_pack(src.path(), "test.judgments", "1.0.0", Some("2026-03-31"), &judgments(), &|b| {
            let mut e = edges_for(b).unwrap();
            // shift the stated place by one character: the excerpt is no longer the text found there
            e[0]["citing"]["char_start"] = json!(e[0]["citing"]["char_start"].as_u64().unwrap() + 1);
            e[0]["citing"]["char_end"] = json!(e[0]["citing"]["char_end"].as_u64().unwrap() + 1);
            Some(e)
        });
        install_pack_from_dir(&paths, src.path()).unwrap();
        let store = crate::retrieval::store::open(&paths).unwrap();
        let l = TreatmentSet::load(&paths).lookup(&built.source_ids[0], "JUDGMENT", &|id| store.get_by_chunk_id(id));
        assert_eq!((l.state, l.edges.len()), ("NONE_FOUND", 0));
        assert!(l.rejected.iter().any(|(id, r)| id == "e-overrule" && r == "EXCERPT_MISMATCH"));
    }

    fn base_and_full() -> (tempfile::TempDir, Built, tempfile::TempDir, Built) {
        let base_dir = tempfile::tempdir().unwrap();
        let base = write_pack(base_dir.path(), "test.judgments", "1.0.0", Some("2026-01-31"), &judgments(), &|_| None);
        let full_dir = tempfile::tempdir().unwrap();
        let full = write_pack(full_dir.path(), "test.judgments", "1.1.0", Some("2026-02-28"), &judgments(), &edges_for);
        (base_dir, base, full_dir, full)
    }

    #[test]
    fn delta_pack_installs_on_its_base_and_adds_treatment_without_resending_the_judgments() {
        let (_v, paths) = vault();
        let (base_dir, base, _fd, full) = base_and_full();
        install_pack_from_dir(&paths, base_dir.path()).unwrap();
        let delta = tempfile::tempdir().unwrap();
        make_delta(&full, &base, delta.path());
        // only what changed is in the delta
        assert!(!delta.path().join("content/j0.txt").exists() && !delta.path().join("metadata/legal_chunks.json").exists());
        assert!(delta.path().join(treatment::EDGES_FILE).exists());
        let r = install_pack_from_dir(&paths, delta.path()).expect("the delta installs on its base");
        assert_eq!((r.pack.version.as_str(), r.pack.content_sha256.as_str()), ("1.1.0", full.content_sha256.as_str()));
        let installed = list_installed(&paths);
        assert_eq!(installed.len(), 1, "the delta replaced the base, it did not add a second copy");
        // the result is the full pack: judgments readable, treatment present, currency date updated
        let store = crate::retrieval::store::open(&paths).unwrap();
        let l = TreatmentSet::load(&paths).lookup(&full.source_ids[0], "JUDGMENT", &|id| store.get_by_chunk_id(id));
        assert_eq!((l.state, l.edges.len()), ("FOUND", 1));
        assert_eq!(treatment::pack_stamps(&paths)[0].currency_date.as_deref(), Some("2026-02-28"));
        assert!(!fs::read_to_string(Path::new(&installed[0].install_path).join("manifest.json")).unwrap().contains("\"delta\""));
    }

    #[test]
    fn delta_pack_fails_closed_when_its_base_differs_or_is_missing_and_leaves_the_installed_pack_alone() {
        let (_v, paths) = vault();
        let (base_dir, base, _fd, full) = base_and_full();
        let delta = tempfile::tempdir().unwrap();
        make_delta(&full, &base, delta.path());
        // no base installed
        let e = install_pack_from_dir(&paths, delta.path()).unwrap_err();
        assert!(e.starts_with("CF_PACK_DELTA_BASE_MISSING"), "{e}");
        install_pack_from_dir(&paths, base_dir.path()).unwrap();
        // a delta built against some other base
        let mut m: Value = serde_json::from_str(&fs::read_to_string(delta.path().join("manifest.json")).unwrap()).unwrap();
        m["delta"]["base_content_sha256"] = json!("0".repeat(64));
        fs::write(delta.path().join("manifest.json"), pretty(&m)).unwrap();
        let e = install_pack_from_dir(&paths, delta.path()).unwrap_err();
        assert!(e.starts_with("CF_PACK_DELTA_BASE_MISMATCH"), "{e}");
        let installed = list_installed(&paths);
        assert_eq!((installed.len(), installed[0].version.as_str(), installed[0].content_sha256.as_str()), (1, "1.0.0", base.content_sha256.as_str()), "the base is untouched");
        // a delta that is not newer than its base
        make_delta(&full, &base, delta.path());
        let mut m: Value = serde_json::from_str(&fs::read_to_string(delta.path().join("manifest.json")).unwrap()).unwrap();
        m["version"] = json!("1.0.0");
        fs::write(delta.path().join("manifest.json"), pretty(&m)).unwrap();
        let e = install_pack_from_dir(&paths, delta.path()).unwrap_err();
        assert!(e.starts_with("CF_PACK_DELTA_NOT_NEWER"), "{e}");
    }

    #[test]
    fn delta_pack_with_a_tampered_file_or_an_unavailable_inherited_file_is_refused_and_the_base_survives() {
        let (_v, paths) = vault();
        let (base_dir, base, _fd, full) = base_and_full();
        install_pack_from_dir(&paths, base_dir.path()).unwrap();
        // tampered: the edges file is changed after the manifest was made
        let tampered = tempfile::tempdir().unwrap();
        make_delta(&full, &base, tampered.path());
        fs::write(tampered.path().join(treatment::EDGES_FILE), "[]\n").unwrap();
        assert!(install_pack_from_dir(&paths, tampered.path()).is_err());
        // inherited file the base does not have with that content: change a base file's expected hash in the delta manifest
        let lying = tempfile::tempdir().unwrap();
        make_delta(&full, &base, lying.path());
        let mut m: Value = serde_json::from_str(&fs::read_to_string(lying.path().join("manifest.json")).unwrap()).unwrap();
        for f in m["files"].as_array_mut().unwrap() {
            if f["path"] == "metadata/legal_chunks.json" {
                f["sha256"] = json!("f".repeat(64));
            }
        }
        fs::write(lying.path().join("manifest.json"), pretty(&m)).unwrap();
        let e = install_pack_from_dir(&paths, lying.path()).unwrap_err();
        assert!(e.starts_with("CF_PACK_DELTA_FILE_MISMATCH") || e.contains("hash"), "{e}");
        let installed = list_installed(&paths);
        assert_eq!((installed.len(), installed[0].version.as_str()), (1, "1.0.0"));
        assert!(crate::retrieval::store::open(&paths).is_ok(), "the base pack still opens");
    }
}
