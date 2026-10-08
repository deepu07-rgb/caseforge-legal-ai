//! On-disk legal store — B-S4-29.
//!
//! Search used to read every installed pack into memory and tokenize every passage on every question. This module
//! builds, once per installed pack version, a compact index under `<vault>/indexes/legal/packs/` and answers
//! questions from it:
//!
//!   docs.dat      one compressed JSON record per passage (random access through docs.off)
//!   postings.dat  keyword postings (delta + varint) for BM25
//!   terms.dat     term -> postings offset, length, document frequency
//!   meta.dat      per passage: five filter codes + keyword length
//!   ids.txt       chunk ids in pack order (also the row order of the pack's meaning vectors)
//!   index.json    format, pack id, signed content hash, SHA-256 of every file except docs.dat
//!
//! Two-stage retrieval: stage 1 picks about 300 keyword candidates (BM25 over the whole store) and the 100 nearest by
//! meaning; stage 2 loads only those passages and runs the existing re-ranker and fusion (`retrieve_core`). Resident
//! memory is the term dictionary and small tables, not the text. The index is derived data: it is built only from a
//! pack that passed signature and lineage verification, is named by the pack's signed content hash, and is rebuilt if
//! any checked file differs; every passage read back is checked against its own SHA-256.

use super::{chunk_from_record, chunk_search_text, dense, retrieve_core, sha256_prefixed, tokenize, CorpusChunk, MetadataFilters, RetrievedHit};
use crate::packs::{is_v2, list_installed, load_legal_chunks, InstalledPack};
use crate::vault::VaultPaths;
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const FORMAT: u32 = 2;
const LEXICAL_CANDIDATES: usize = 300;
const DENSE_CANDIDATES: usize = 100;
const VERIFIED_FILES: [&str; 5] = ["docs.off", "postings.dat", "terms.dat", "meta.dat", "groups.json"];

#[derive(Serialize, Deserialize)]
struct IndexInfo {
    format: u32,
    pack_id: String,
    content_sha256: String,
    n: u32,
    total_len: u64,
    /// Version and trust level of the pack's first passage (the audit's pack snapshot).
    version: String,
    trust_level: String,
    /// Distinct filter values; meta.dat holds indexes into this table.
    strings: Vec<String>,
    files: BTreeMap<String, String>,
}

/// One authority as the citation audit sees it: pages/passages of the same work (same pack, title and citation) form one
/// group, and the most restrictive status across them applies. Mirrors `audit::group_sources`.
#[derive(Serialize, Deserialize, Clone)]
pub struct GroupRec {
    pub title: String,
    pub citation: String,
    pub source_id: String,
    pub authority_type: String,
    pub trust_level: String,
    pub temporal_status: String,
    pub superseded_status: String,
    pub docs: Vec<u32>,
}

impl GroupRec {
    pub fn new(m: &super::LegalMetadata) -> GroupRec {
        GroupRec {
            title: m.title.clone(),
            citation: m.citation.clone(),
            source_id: m.source_id.clone(),
            authority_type: m.authority_type.clone(),
            trust_level: m.trust_level.clone(),
            temporal_status: m.temporal_status.clone(),
            superseded_status: m.superseded_status.clone(),
            docs: Vec::new(),
        }
    }

    pub fn absorb(&mut self, m: &super::LegalMetadata) {
        if m.trust_level != "OFFICIAL_VERIFIED" {
            self.trust_level = m.trust_level.clone();
        }
        if m.superseded_status != "CURRENT" && self.superseded_status != "SUPERSEDED" {
            self.superseded_status = m.superseded_status.clone();
        }
        if m.temporal_status != "APPLICABLE" {
            self.temporal_status = m.temporal_status.clone();
        }
    }
}

fn file_sha(path: &Path) -> Result<String, String> {
    let mut f = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

fn put_varint(buf: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        buf.push((v as u8) | 0x80);
        v >>= 7;
    }
    buf.push(v as u8);
}

fn get_varint(b: &[u8], pos: &mut usize) -> Option<u64> {
    let (mut v, mut shift) = (0u64, 0u32);
    loop {
        let byte = *b.get(*pos)?;
        *pos += 1;
        v |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(v);
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
}

fn intern(strings: &mut Vec<String>, value: &str) -> Result<u8, String> {
    if let Some(i) = strings.iter().position(|s| s == value) {
        return Ok(i as u8);
    }
    if strings.len() >= 255 {
        return Err("CF_STORE_TOO_MANY_FILTER_VALUES".into());
    }
    strings.push(value.to_string());
    Ok((strings.len() - 1) as u8)
}

/// Builds the index of one verified pack into `dir` (atomically).
fn build(pack: &InstalledPack, dir: &Path) -> Result<(), String> {
    let records = load_legal_chunks(Path::new(&pack.install_path))?; // verifies signature, hashes and lineage
    let tmp = dir.with_file_name(format!("{}.tmp-{}", dir.file_name().and_then(|n| n.to_str()).unwrap_or("index"), Uuid::new_v4()));
    fs::create_dir_all(&tmp).map_err(|e| format!("CF_STORE_WRITE: {e}"))?;
    let result = write_files(pack, &records, &tmp);
    if let Err(e) = result {
        let _ = fs::remove_dir_all(&tmp);
        return Err(e);
    }
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|e| format!("CF_STORE_REPLACE: {e}"))?;
    }
    fs::rename(&tmp, dir).map_err(|e| format!("CF_STORE_COMMIT: {e}"))
}

fn write_files(pack: &InstalledPack, records: &[crate::packs::LegalChunkRecord], tmp: &Path) -> Result<(), String> {
    let io = |e: std::io::Error| format!("CF_STORE_WRITE: {e}");
    let mut docs = BufWriter::new(File::create(tmp.join("docs.dat")).map_err(io)?);
    let (mut offsets, mut meta, mut ids) = (Vec::<u8>::new(), Vec::<u8>::new(), String::new());
    let mut pos = 0u64;
    let mut postings: BTreeMap<String, Vec<(u32, u32)>> = BTreeMap::new();
    let mut strings: Vec<String> = Vec::new();
    let mut total_len = 0u64;
    let mut groups: BTreeMap<(String, String), GroupRec> = BTreeMap::new();
    let (mut first_version, mut first_trust) = (String::new(), String::new());
    for (i, record) in records.iter().enumerate() {
        let chunk = chunk_from_record(record);
        if i == 0 {
            first_version = chunk.metadata.version.clone();
            first_trust = chunk.metadata.trust_level.clone();
        }
        let group = groups.entry((chunk.metadata.title.clone(), chunk.metadata.citation.clone())).or_insert_with(|| GroupRec::new(&chunk.metadata));
        group.absorb(&chunk.metadata);
        group.docs.push(i as u32);
        offsets.extend_from_slice(&pos.to_le_bytes());
        let json = serde_json::to_vec(&chunk).map_err(|e| e.to_string())?;
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::new(6));
        enc.write_all(&json).map_err(io)?;
        let z = enc.finish().map_err(io)?;
        docs.write_all(&z).map_err(io)?;
        pos += z.len() as u64;

        let tokens = tokenize(&chunk_search_text(&chunk));
        let mut tf: HashMap<&str, u32> = HashMap::new();
        for t in &tokens {
            *tf.entry(t.as_str()).or_insert(0) += 1;
        }
        for (t, c) in tf {
            postings.entry(t.to_string()).or_default().push((i as u32, c));
        }
        let m = &chunk.metadata;
        for value in [chunk.jurisdiction.as_deref().unwrap_or(""), &m.authority_type, &m.trust_level, &m.temporal_status, &m.superseded_status] {
            meta.push(intern(&mut strings, value)?);
        }
        meta.extend_from_slice(&(tokens.len() as u32).to_le_bytes());
        total_len += tokens.len() as u64;
        ids.push_str(&chunk.chunk_id);
        ids.push('\n');
    }
    offsets.extend_from_slice(&pos.to_le_bytes());
    docs.flush().map_err(io)?;

    let (mut post_buf, mut terms) = (Vec::<u8>::new(), String::new());
    for (term, list) in &postings {
        let start = post_buf.len();
        let mut prev = 0u32;
        for (doc, tf) in list {
            put_varint(&mut post_buf, u64::from(doc - prev));
            put_varint(&mut post_buf, u64::from(*tf));
            prev = *doc;
        }
        terms.push_str(&format!("{term}\t{start}\t{}\t{}\n", post_buf.len() - start, list.len()));
    }
    fs::write(tmp.join("docs.off"), &offsets).map_err(io)?;
    fs::write(tmp.join("postings.dat"), &post_buf).map_err(io)?;
    fs::write(tmp.join("terms.dat"), terms.as_bytes()).map_err(io)?;
    fs::write(tmp.join("meta.dat"), &meta).map_err(io)?;
    fs::write(tmp.join("ids.txt"), ids.as_bytes()).map_err(io)?;
    fs::write(tmp.join("groups.json"), serde_json::to_vec(&groups.into_values().collect::<Vec<_>>()).map_err(|e| e.to_string())?).map_err(io)?;
    let mut files = BTreeMap::new();
    for name in VERIFIED_FILES.iter().chain(["ids.txt"].iter()) {
        files.insert(name.to_string(), file_sha(&tmp.join(name))?);
    }
    let info = IndexInfo { format: FORMAT, pack_id: pack.pack_id.clone(), content_sha256: pack.content_sha256.clone(), n: records.len() as u32, total_len, version: first_version, trust_level: first_trust, strings, files };
    fs::write(tmp.join("index.json"), serde_json::to_vec_pretty(&info).map_err(|e| e.to_string())?).map_err(io)
}

pub struct PackIndex {
    pub pack_id: String,
    pub content_sha256: String,
    n: u32,
    total_len: u64,
    strings: Vec<String>,
    terms: HashMap<String, (u64, u32, u32)>,
    meta: Vec<[u8; 5]>,
    lens: Vec<u32>,
    offsets: Vec<u64>,
    ids: Vec<String>,
    groups: Vec<GroupRec>,
    version: String,
    trust_level: String,
    postings: Mutex<File>,
    docs: Mutex<File>,
}

impl PackIndex {
    fn open(dir: &Path, pack: &InstalledPack) -> Result<PackIndex, String> {
        let info: IndexInfo = serde_json::from_slice(&fs::read(dir.join("index.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if info.format != FORMAT || info.pack_id != pack.pack_id || info.content_sha256 != pack.content_sha256 {
            return Err("CF_STORE_STALE".into());
        }
        for name in VERIFIED_FILES.iter().chain(["ids.txt"].iter()) {
            if info.files.get(*name).map(String::as_str) != Some(file_sha(&dir.join(name))?.as_str()) {
                return Err(format!("CF_STORE_TAMPERED: {name}"));
            }
        }
        let n = info.n as usize;
        let raw = fs::read(dir.join("meta.dat")).map_err(|e| e.to_string())?;
        if raw.len() != n * 9 {
            return Err("CF_STORE_TAMPERED: meta.dat".into());
        }
        let mut meta = Vec::with_capacity(n);
        let mut lens = Vec::with_capacity(n);
        for row in raw.chunks_exact(9) {
            meta.push([row[0], row[1], row[2], row[3], row[4]]);
            lens.push(u32::from_le_bytes([row[5], row[6], row[7], row[8]]));
        }
        if meta.iter().flatten().any(|c| *c as usize >= info.strings.len().max(1)) {
            return Err("CF_STORE_TAMPERED: meta.dat codes".into());
        }
        let off_raw = fs::read(dir.join("docs.off")).map_err(|e| e.to_string())?;
        if off_raw.len() != (n + 1) * 8 {
            return Err("CF_STORE_TAMPERED: docs.off".into());
        }
        let offsets: Vec<u64> = off_raw.chunks_exact(8).map(|b| u64::from_le_bytes(b.try_into().unwrap_or([0; 8]))).collect();
        let docs = File::open(dir.join("docs.dat")).map_err(|e| e.to_string())?;
        if docs.metadata().map_err(|e| e.to_string())?.len() != *offsets.last().unwrap_or(&0) {
            return Err("CF_STORE_TAMPERED: docs.dat length".into());
        }
        let ids: Vec<String> = fs::read_to_string(dir.join("ids.txt")).map_err(|e| e.to_string())?.lines().map(str::to_string).collect();
        if ids.len() != n {
            return Err("CF_STORE_TAMPERED: ids.txt".into());
        }
        let mut terms = HashMap::new();
        for line in fs::read_to_string(dir.join("terms.dat")).map_err(|e| e.to_string())?.lines() {
            let mut it = line.split('\t');
            let (Some(t), Some(o), Some(l), Some(d)) = (it.next(), it.next(), it.next(), it.next()) else { return Err("CF_STORE_TAMPERED: terms.dat".into()) };
            terms.insert(t.to_string(), (o.parse().map_err(|_| "terms.dat")?, l.parse().map_err(|_| "terms.dat")?, d.parse().map_err(|_| "terms.dat")?));
        }
        let groups: Vec<GroupRec> = serde_json::from_slice(&fs::read(dir.join("groups.json")).map_err(|e| e.to_string())?).map_err(|_| "CF_STORE_TAMPERED: groups.json".to_string())?;
        if groups.iter().map(|g| g.docs.len()).sum::<usize>() != n || groups.iter().flat_map(|g| &g.docs).any(|d| *d as usize >= n) {
            return Err("CF_STORE_TAMPERED: groups.json".into());
        }
        let postings = Mutex::new(File::open(dir.join("postings.dat")).map_err(|e| e.to_string())?);
        dense::register_pack(Path::new(&pack.install_path), &pack.content_sha256, &ids);
        Ok(PackIndex { pack_id: pack.pack_id.clone(), content_sha256: pack.content_sha256.clone(), n: info.n, total_len: info.total_len, strings: info.strings, terms, meta, lens, offsets, ids, groups, version: info.version, trust_level: info.trust_level, postings, docs: Mutex::new(docs) })
    }

    fn ensure(paths: &VaultPaths, pack: &InstalledPack) -> Result<PackIndex, String> {
        let base = paths.root.join("indexes").join("legal").join("packs");
        let dir = base.join(format!("{}-{}", pack.pack_id, &pack.content_sha256[..pack.content_sha256.len().min(16)]));
        match PackIndex::open(&dir, pack) {
            Ok(index) => Ok(index),
            Err(_) => {
                build(pack, &dir)?;
                // Indexes of older versions of this pack are no longer reachable.
                if let Ok(entries) = fs::read_dir(&base) {
                    for e in entries.flatten() {
                        let name = e.file_name().to_string_lossy().to_string();
                        if name.starts_with(&format!("{}-", pack.pack_id)) && e.path() != dir {
                            let _ = fs::remove_dir_all(e.path());
                        }
                    }
                }
                PackIndex::open(&dir, pack)
            }
        }
    }

    fn read_postings(&self, term: &str) -> Vec<(u32, u32)> {
        let Some(&(off, len, _)) = self.terms.get(term) else { return Vec::new() };
        let mut buf = vec![0u8; len as usize];
        let ok = self.postings.lock().map(|mut f| f.seek(SeekFrom::Start(off)).and_then(|_| f.read_exact(&mut buf)).is_ok()).unwrap_or(false);
        let (mut out, mut pos, mut doc) = (Vec::new(), 0usize, 0u64);
        while ok && pos < buf.len() {
            let (Some(d), Some(tf)) = (get_varint(&buf, &mut pos), get_varint(&buf, &mut pos)) else { break };
            doc += d;
            out.push((doc as u32, tf as u32));
        }
        out
    }

    /// Mirrors `filter_allows`: explicit filters, and known-invalid material is always excluded when `closed`.
    fn allowed(&self, f: &MetadataFilters, closed: bool) -> Vec<bool> {
        let eq = |want: &Option<String>, s: &str| want.as_deref().map(|w| w == s).unwrap_or(true);
        let ok: Vec<[bool; 5]> = self
            .strings
            .iter()
            .map(|s| {
                [
                    eq(&f.jurisdiction, s),
                    eq(&f.authority_type, s),
                    eq(&f.trust_level, s),
                    eq(&f.temporal_status, s) && (!closed || (s != "EXPIRED" && s != "NOT_YET_EFFECTIVE")),
                    eq(&f.superseded_status, s) && (!closed || s != "SUPERSEDED"),
                ]
            })
            .collect();
        self.meta.iter().map(|m| (0..5).all(|k| ok[m[k] as usize][k])).collect()
    }

    fn load(&self, doc: u32) -> Result<CorpusChunk, String> {
        let (a, b) = (self.offsets[doc as usize], self.offsets[doc as usize + 1]);
        let mut z = vec![0u8; (b - a) as usize];
        self.docs.lock().map_err(|_| "CF_STORE_LOCK".to_string()).and_then(|mut f| f.seek(SeekFrom::Start(a)).and_then(|_| f.read_exact(&mut z)).map_err(|e| e.to_string()))?;
        let mut json = Vec::new();
        ZlibDecoder::new(&z[..]).read_to_end(&mut json).map_err(|e| format!("CF_STORE_TAMPERED: passage {doc}: {e}"))?;
        let chunk: CorpusChunk = serde_json::from_slice(&json).map_err(|e| format!("CF_STORE_TAMPERED: passage {doc}: {e}"))?;
        if chunk.chunk_id != self.ids[doc as usize] || sha256_prefixed(&chunk.text) != chunk.metadata.checksum {
            return Err(format!("CF_STORE_TAMPERED: passage {doc} fails its checksum"));
        }
        Ok(chunk)
    }
}

/// One authority across the store (see `GroupRec`), with its passages.
pub struct GroupView {
    pub pack_id: String,
    pub rec: GroupRec,
    pub docs: Vec<DocRef>,
}

pub struct Store {
    packs: Vec<PackIndex>,
    views: Vec<GroupView>,
    doc_group: HashMap<DocRef, usize>,
    by_hash: HashMap<u64, (u16, u32)>,
    n_total: u64,
    total_len: u64,
    /// Identity of the installed legal material (pack ids and signed content hashes).
    pub checksum: String,
}

pub type DocRef = (u16, u32);

impl Store {
    fn new(packs: Vec<PackIndex>) -> Store {
        let mut by_hash = HashMap::new();
        for (p, pack) in packs.iter().enumerate() {
            for (d, id) in pack.ids.iter().enumerate() {
                by_hash.insert(dense::key(id), (p as u16, d as u32));
            }
        }
        let mut views: Vec<GroupView> = Vec::new();
        for (p, pack) in packs.iter().enumerate() {
            for g in &pack.groups {
                views.push(GroupView { pack_id: pack.pack_id.clone(), rec: g.clone(), docs: g.docs.iter().map(|d| (p as u16, *d)).collect() });
            }
        }
        views.sort_by(|a, b| (&a.pack_id, &a.rec.title, &a.rec.citation).cmp(&(&b.pack_id, &b.rec.title, &b.rec.citation)));
        let doc_group: HashMap<DocRef, usize> = views.iter().enumerate().flat_map(|(i, v)| v.docs.iter().map(move |d| (*d, i))).collect();
        let mut identity: Vec<String> = packs.iter().map(|p| format!("{}:{}", p.pack_id, p.content_sha256)).collect();
        identity.sort();
        Store {
            n_total: packs.iter().map(|p| u64::from(p.n)).sum(),
            total_len: packs.iter().map(|p| p.total_len).sum(),
            checksum: sha256_prefixed(&identity.join("\n")),
            by_hash,
            views,
            doc_group,
            packs,
        }
    }

    /// Authorities in the order `audit::group_sources` yields them (pack id, title, citation).
    pub fn group_views(&self) -> &[GroupView] {
        &self.views
    }

    pub fn group_of_doc(&self, r: DocRef) -> Option<usize> {
        self.doc_group.get(&r).copied()
    }

    pub fn load_doc(&self, r: DocRef) -> Result<CorpusChunk, String> {
        self.load(r)
    }

    /// Passages most likely to contain a quotation (keyword ranking of its words), whatever their status.
    pub fn quote_candidates(&self, quote: &str, k: usize) -> Vec<DocRef> {
        self.lexical(&tokenize(quote), &MetadataFilters::default(), false, k)
    }

    /// (pack id, version, trust level) of each installed v2 pack, from its first passage.
    pub fn pack_snapshots(&self) -> Vec<(String, String, String)> {
        self.packs.iter().map(|p| (p.pack_id.clone(), p.version.clone(), p.trust_level.clone())).collect()
    }

    pub fn len(&self) -> usize {
        self.n_total as usize
    }

    /// BM25 over the whole store (corpus-wide statistics), `k` best.
    fn lexical(&self, terms: &[String], filters: &MetadataFilters, closed: bool, k: usize) -> Vec<DocRef> {
        if terms.is_empty() || self.n_total == 0 {
            return Vec::new();
        }
        let allowed: Vec<Vec<bool>> = self.packs.iter().map(|p| p.allowed(filters, closed)).collect();
        let (n, avg) = (self.n_total as f64, (self.total_len as f64 / self.n_total as f64).max(1.0));
        let mut scores: HashMap<DocRef, f64> = HashMap::new();
        for term in terms.iter().collect::<HashSet<_>>() {
            let df: u64 = self.packs.iter().map(|p| p.terms.get(term.as_str()).map(|x| u64::from(x.2)).unwrap_or(0)).sum();
            if df == 0 {
                continue;
            }
            let idf = (1.0 + (n - df as f64 + 0.5) / (df as f64 + 0.5)).ln();
            for (pi, pack) in self.packs.iter().enumerate() {
                for (doc, tf) in pack.read_postings(term) {
                    if !allowed[pi][doc as usize] {
                        continue;
                    }
                    let (tf, len) = (f64::from(tf), f64::from(pack.lens[doc as usize]));
                    *scores.entry((pi as u16, doc)).or_insert(0.0) += idf * (tf * 2.2 / (tf + 1.2 * (1.0 - 0.75 + 0.75 * len / avg)));
                }
            }
        }
        let mut ranked: Vec<(DocRef, f64)> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
        ranked.into_iter().take(k).map(|(r, _)| r).collect()
    }

    /// Nearest passages by meaning among those the filters allow.
    fn nearest(&self, query: &[f32], filters: &MetadataFilters, k: usize) -> Vec<DocRef> {
        let allowed: Vec<Vec<bool>> = self.packs.iter().map(|p| p.allowed(filters, true)).collect();
        dense::scan(query, k, &|hash| self.by_hash.get(&hash).map(|&(p, d)| allowed[p as usize][d as usize]).unwrap_or(false))
            .into_iter()
            .filter_map(|(hash, _)| self.by_hash.get(&hash).copied())
            .collect()
    }

    fn load(&self, r: DocRef) -> Result<CorpusChunk, String> {
        self.packs[r.0 as usize].load(r.1)
    }

    pub fn get_chunk_by_id_checked(&self, chunk_id: &str) -> Option<CorpusChunk> {
        self.get_by_chunk_id(chunk_id)
    }

    pub fn get_by_chunk_id(&self, chunk_id: &str) -> Option<CorpusChunk> {
        let r = *self.by_hash.get(&dense::key(chunk_id))?;
        self.load(r).ok().filter(|c| c.chunk_id == chunk_id)
    }
}

static CURRENT: Lazy<Mutex<Option<(String, Arc<Store>)>>> = Lazy::new(|| Mutex::new(None));

/// The store for the vault's installed v2 packs: opened from disk, indexes built for new or changed packs.
pub fn open(paths: &VaultPaths) -> Result<Arc<Store>, String> {
    let mut installed: Vec<InstalledPack> = list_installed(paths).into_iter().filter(|p| is_v2(&p.schema_version)).collect();
    installed.sort_by(|a, b| a.pack_id.cmp(&b.pack_id));
    let key = format!("{}|{}", paths.root.display(), installed.iter().map(|p| format!("{}:{}", p.pack_id, p.content_sha256)).collect::<Vec<_>>().join(","));
    let mut current = CURRENT.lock().map_err(|_| "CF_STORE_LOCK".to_string())?;
    if let Some((k, store)) = current.as_ref() {
        if *k == key {
            return Ok(store.clone());
        }
    }
    let packs = installed.iter().map(|p| PackIndex::ensure(paths, p)).collect::<Result<Vec<_>, _>>()?;
    let store = Arc::new(Store::new(packs));
    *current = Some((key, store.clone()));
    Ok(store)
}

/// Identity of everything a search ran over: the store plus the in-memory passages (older packs, case material).
pub fn combined_checksum(store: &Store, small: &[CorpusChunk]) -> String {
    sha256_prefixed(&format!("{}\n{}", store.checksum, super::corpus_checksum(small)))
}

pub struct Found {
    pub hits: Vec<RetrievedHit>,
    /// The passages the hits (and the citation checks) refer to; not the whole corpus.
    pub chunks: Vec<CorpusChunk>,
    pub corpus_checksum: String,
    pub chunk_count: usize,
}

/// Searches the vault's installed packs plus `small` (in-memory passages). `must_contain` are authority strings whose
/// passages must be available to the caller's citation checks even if they are not among the hits.
pub fn search(paths: &VaultPaths, query_text: &str, terms: &[String], top_k: usize, filters: &MetadataFilters, semantic_available: bool, small: &[CorpusChunk], must_contain: &[String]) -> Result<Found, String> {
    let store = open(paths)?;
    let dense_query = if !cfg!(test) && dense::has_vectors() && !query_text.trim().is_empty() { dense::embed_query(query_text.trim()) } else { None };
    search_with(&store, terms, top_k, filters, semantic_available, small, must_contain, dense_query.as_deref())
}

pub fn search_with(store: &Store, terms: &[String], top_k: usize, filters: &MetadataFilters, semantic_available: bool, small: &[CorpusChunk], must_contain: &[String], dense_query: Option<&[f32]>) -> Result<Found, String> {
    let mut refs: Vec<DocRef> = store.lexical(terms, filters, true, LEXICAL_CANDIDATES.max(top_k * 4));
    if let Some(q) = dense_query {
        refs.extend(store.nearest(q, filters, DENSE_CANDIDATES));
    }
    for authority in must_contain {
        refs.extend(store.lexical(&tokenize(authority), &MetadataFilters::default(), false, 20));
    }
    let mut seen = HashSet::new();
    refs.retain(|r| seen.insert(*r));
    let mut chunks = refs.into_iter().map(|r| store.load(r)).collect::<Result<Vec<_>, _>>()?;
    chunks.extend(small.iter().cloned());
    let hits = retrieve_core(&chunks, terms, top_k, filters, semantic_available, dense_query);
    Ok(Found { hits, chunks, corpus_checksum: combined_checksum(store, small), chunk_count: store.len() + small.len() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn vault() -> (tempfile::TempDir, VaultPaths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = VaultPaths::new(dir.path().to_path_buf());
        let pack = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../packs/official/caseforge.india.partition.highcourt.civil-appeal");
        crate::packs::install_pack_from_dir(&paths, &pack).expect("official pack installs");
        (dir, paths)
    }

    fn india_verified() -> MetadataFilters {
        MetadataFilters { jurisdiction: Some("India".into()), trust_level: Some("OFFICIAL_VERIFIED".into()), ..MetadataFilters::default() }
    }

    #[test]
    fn b_s4_29_store_answers_the_hybrid_goldens_like_the_in_memory_path() {
        let (_dir, paths) = vault();
        let store = open(&paths).unwrap();
        assert_eq!(store.len(), 20);
        let goldens = [
            ("When may the court order sale instead of physical division in a partition suit?", "Power to court to order sale instead of division"),
            ("What is the route of an appeal from an original decree?", "Appeal from original decree"),
            ("What must accompany a delayed appeal?", "Application for condonation of delay"),
            ("What limitation entry applies to an appeal to a High Court under the CPC?", "116. Under the Code of Civil Procedure"),
        ];
        let (mut recalled, mut rr) = (0usize, 0.0f64);
        for (query, expected) in goldens {
            let found = search_with(&store, &tokenize(query), 8, &india_verified(), true, &[], &[], None).unwrap();
            let pos = found.hits.iter().position(|h| found.chunks.iter().any(|c| c.chunk_id == h.chunk_id && c.text.to_ascii_lowercase().contains(&expected.to_ascii_lowercase())));
            if let Some(rank) = pos {
                recalled += 1;
                rr += 1.0 / (rank as f64 + 1.0);
            }
            assert_eq!(found.chunk_count, 20);
        }
        assert!(recalled as f64 / 4.0 >= 0.90, "Recall@8 {recalled}/4");
        assert!(rr / 4.0 >= 0.70, "MRR {}", rr / 4.0);
    }

    #[test]
    fn b_s4_29_filters_are_applied_before_candidates_and_known_invalid_is_excluded() {
        let (_dir, paths) = vault();
        let store = open(&paths).unwrap();
        let terms = tokenize("appeal decree");
        assert!(!store.lexical(&terms, &india_verified(), true, 50).is_empty());
        let other = MetadataFilters { jurisdiction: Some("United States".into()), ..MetadataFilters::default() };
        assert!(store.lexical(&terms, &other, true, 50).is_empty());
        assert!(search_with(&store, &terms, 8, &other, true, &[], &[], None).unwrap().hits.is_empty());
    }

    #[test]
    fn b_s4_29_index_is_reused_rebuilt_when_tampered_and_checks_each_passage() {
        let (dir, paths) = vault();
        let first = open(&paths).unwrap();
        let id = first.packs[0].ids[3].clone();
        let chunk = first.get_by_chunk_id(&id).expect("passage loads by id");
        assert_eq!(chunk.chunk_id, id);
        assert!(first.get_by_chunk_id("cfchunk:not-a-passage").is_none());
        assert!(Arc::ptr_eq(&first, &open(&paths).unwrap()), "same installed packs reuse the opened store");
        let packs_dir = dir.path().join("indexes").join("legal").join("packs");
        let index_dir = fs::read_dir(&packs_dir).unwrap().next().unwrap().unwrap().path();
        // Tampering with a keyword file is detected on the next open and the index is rebuilt from the verified pack.
        fs::write(index_dir.join("terms.dat"), b"appeal\t0\t1\t999\n").unwrap();
        *CURRENT.lock().unwrap() = None;
        let rebuilt = open(&paths).unwrap();
        assert_eq!(rebuilt.len(), 20);
        assert!(!rebuilt.lexical(&tokenize("appeal decree"), &india_verified(), true, 5).is_empty());
        // A passage whose stored bytes no longer match its checksum is refused rather than served.
        let mut docs = fs::read(index_dir.join("docs.dat")).unwrap();
        let mid = docs.len() / 2;
        docs[mid] ^= 0xff;
        fs::write(index_dir.join("docs.dat"), &docs).unwrap();
        *CURRENT.lock().unwrap() = None;
        let damaged = open(&paths).unwrap();
        let all_ok = (0..20u32).all(|d| damaged.packs[0].load(d).is_ok());
        assert!(!all_ok, "a flipped byte in docs.dat must fail a passage check");
    }

    fn rss_mb() -> (u64, u64) {
        let status = fs::read_to_string("/proc/self/status").unwrap_or_default();
        let kb = |key: &str| status.lines().find(|l| l.starts_with(key)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) / 1024;
        (kb("VmRSS:"), kb("VmHWM:"))
    }

    /// Scale measurement on real packs (run by .github/workflows/store-bench.yml; CASEFORGE_BENCH_PACKS = folder of
    /// unzipped signed packs). Prints install, index build, reopen, per-query latency and memory, and the old
    /// whole-corpus load for comparison.
    #[test]
    #[ignore]
    fn b_s4_29_scale_benchmark() {
        use std::time::Instant;
        let root = PathBuf::from(std::env::var("CASEFORGE_BENCH_PACKS").expect("CASEFORGE_BENCH_PACKS"));
        let dir = tempfile::tempdir().unwrap();
        let paths = VaultPaths::new(dir.path().to_path_buf());
        let mut packs: Vec<PathBuf> = fs::read_dir(&root).unwrap().flatten().map(|e| e.path()).filter(|p| p.join("manifest.json").is_file()).collect();
        packs.sort();
        let t = Instant::now();
        for p in &packs {
            crate::packs::install_pack_from_dir(&paths, p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        }
        println!("BENCH packs={} install_s={:.1}", packs.len(), t.elapsed().as_secs_f64());
        let t = Instant::now();
        let store = open(&paths).unwrap();
        println!("BENCH passages={} first_open_build_s={:.1} rss_mb={:?}", store.len(), t.elapsed().as_secs_f64(), rss_mb());
        drop(store);
        *CURRENT.lock().unwrap() = None;
        let t = Instant::now();
        let store = open(&paths).unwrap();
        println!("BENCH reopen_s={:.2} rss_mb={:?}", t.elapsed().as_secs_f64(), rss_mb());
        let queries = ["anticipatory bail", "punishment for cheating", "limitation period for appeal", "electronic record certificate", "arbitration award setting aside", "consumer complaint deficiency in service", "income tax assessment reopening", "environment clearance", "companies act director disqualification", "motor accident compensation"];
        let mut ms = Vec::new();
        let mut returned = 0;
        for q in queries {
            let t = Instant::now();
            let found = search_with(&store, &tokenize(q), 8, &india_verified(), true, &[], &[], None).unwrap();
            ms.push(t.elapsed().as_secs_f64() * 1000.0);
            returned += found.hits.len();
        }
        ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!("BENCH queries={} hits_returned={} median_ms={:.0} max_ms={:.0} rss_mb={:?}", ms.len(), returned, ms[ms.len() / 2], ms[ms.len() - 1], rss_mb());
        assert!(returned > 0);
        let t = Instant::now();
        let corpus = crate::retrieval::build_corpus(&paths, &[]).unwrap();
        println!("BENCH old_full_load_s={:.1} passages={} rss_mb={:?}", t.elapsed().as_secs_f64(), corpus.len(), rss_mb());
        let t = Instant::now();
        let hits = crate::retrieval::retrieve_filtered(&corpus, &tokenize(queries[0]), 8, &india_verified());
        println!("BENCH old_query_s={:.1} hits={}", t.elapsed().as_secs_f64(), hits.len());
    }

    #[test]
    fn b_s4_29_varint_round_trip() {
        let mut b = Vec::new();
        let values = [0u64, 1, 127, 128, 300, 16_384, u32::MAX as u64];
        values.iter().for_each(|v| put_varint(&mut b, *v));
        let mut pos = 0;
        for v in values {
            assert_eq!(get_varint(&b, &mut pos), Some(v));
        }
        assert_eq!(get_varint(&b, &mut pos), None);
    }
}
