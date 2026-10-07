//! B-S3-07 counsel-authority overlays and deterministic three-way graph merge.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const MERGE_ENGINE_ID: &str = "caseforge.authority-three-way-merge.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorityOverlay {
    pub target_key: String,
    pub latest_event_id: String,
    pub target_kind: String,
    pub collection: Option<String>,
    pub item_id: Option<String>,
    pub field_name: Option<String>,
    pub doc_id: Option<String>,
    pub page_index: Option<i64>,
    pub word_index: Option<i64>,
    pub base_value: String,
    pub counsel_value: String,
    pub anchor: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictCandidate {
    pub overlay_event_id: String,
    pub target_key: String,
    pub target_kind: String,
    pub conflict_kind: String,
    pub generated_value: Option<String>,
    pub counsel_value: String,
    pub anchor: Value,
}

fn value_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn item_id<'a>(item: &'a Value, collection: &str) -> Option<&'a str> {
    let field = match collection {
        "nodes" => "node_id",
        "edges" => "edge_id",
        "chronology" => "event_id",
        "parties" => "party_id",
        "statutes" => "statute_id",
        "evidence" => "evidence_id",
        "prayer_relief" => "relief_id",
        "contradictions" => "contradiction_id",
        "counsel_facts" => "fact_id",
        _ => "id",
    };
    item.get(field).and_then(Value::as_str)
}

fn source_hash(item: &Value) -> Option<&str> {
    item.pointer("/provenance/sources/0/content_hash")
        .or_else(|| item.pointer("/provenance/source/content_hash"))
        .and_then(Value::as_str)
}

fn source_doc(item: &Value) -> Option<&str> {
    item.pointer("/provenance/sources/0/doc_id")
        .or_else(|| item.pointer("/provenance/source/doc_id"))
        .and_then(Value::as_str)
}

fn anchor_matches(item: &Value, overlay: &AuthorityOverlay) -> bool {
    let expected_hash = overlay
        .anchor
        .get("source_content_hash")
        .and_then(Value::as_str);
    let expected_doc = overlay.anchor.get("source_doc_id").and_then(Value::as_str);
    expected_hash.is_some_and(|h| source_hash(item) == Some(h))
        || (expected_hash.is_none() && expected_doc.is_some_and(|d| source_doc(item) == Some(d)))
}

fn counsel_authoritative(item: &Value) -> bool {
    let p = item.get("provenance").unwrap_or(&Value::Null);
    p.get("counsel_confirmed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || matches!(
            p.get("origin").and_then(Value::as_str),
            Some("USER_EDIT" | "SOURCED_BY_COUNSEL")
        )
        || matches!(
            item.get("origin").and_then(Value::as_str),
            Some("SOURCED_BY_COUNSEL")
        )
}

fn apply_overlay(item: &mut Value, overlay: &AuthorityOverlay) {
    let Some(field) = overlay.field_name.as_deref() else {
        return;
    };
    let Some(obj) = item.as_object_mut() else {
        return;
    };
    obj.insert(field.into(), Value::String(overlay.counsel_value.clone()));
    let p = obj.entry("provenance").or_insert_with(|| json!({}));
    if let Some(prov) = p.as_object_mut() {
        prov.insert("origin".into(), Value::String("USER_EDIT".into()));
        prov.insert("confidence".into(), json!(1.0));
        prov.insert("reviewed".into(), Value::Bool(true));
        prov.insert("counsel_confirmed".into(), Value::Bool(true));
        let lineage = prov
            .entry("correction_lineage")
            .or_insert_with(|| json!([]));
        if let Some(a) = lineage.as_array_mut() {
            if !a
                .iter()
                .any(|v| v.as_str() == Some(&overlay.latest_event_id))
            {
                a.push(Value::String(overlay.latest_event_id.clone()));
            }
        }
        prov.insert(
            "notes".into(),
            Value::String("active counsel overlay applied by B-S3-07 merge".into()),
        );
    }
}

fn preserve_authoritative(prior: &Value, next: &mut Value, collection: &str) {
    let Some(old_items) = prior.get(collection).and_then(Value::as_array) else {
        return;
    };
    let Some(new_items) = next.get_mut(collection).and_then(Value::as_array_mut) else {
        return;
    };
    for old in old_items.iter().filter(|item| counsel_authoritative(item)) {
        let old_id = item_id(old, collection);
        let exists = new_items.iter().any(|candidate| {
            old_id.is_some() && item_id(candidate, collection) == old_id
                || source_hash(old).is_some() && source_hash(candidate) == source_hash(old)
        });
        if !exists {
            new_items.push(old.clone());
        }
    }
}

/// Merge prior generated state + immutable counsel overlays + newly generated graph.
pub fn merge_graph(
    prior: Option<&Value>,
    mut next: Value,
    overlays: &[AuthorityOverlay],
) -> (Value, Vec<ConflictCandidate>) {
    let mut conflicts = Vec::new();
    for overlay in overlays.iter().filter(|o| o.target_kind == "graph_item") {
        let Some(collection) = overlay.collection.as_deref() else {
            continue;
        };
        let Some(field) = overlay.field_name.as_deref() else {
            continue;
        };
        let Some(items) = next.get_mut(collection).and_then(Value::as_array_mut) else {
            continue;
        };

        let exact: Vec<usize> = items.iter().enumerate().filter(|(_, item)| {
            overlay.item_id.as_deref().is_some_and(|id| item_id(item, collection) == Some(id))
                && (anchor_matches(item, overlay)
                    || matches!(value_text(item.get(field)), value if value == overlay.base_value || value == overlay.counsel_value))
        }).map(|(i, _)| i).collect();
        let by_source: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, item)| anchor_matches(item, overlay))
            .map(|(i, _)| i)
            .collect();
        let by_base: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, item)| value_text(item.get(field)) == overlay.base_value)
            .map(|(i, _)| i)
            .collect();
        let selected = exact
            .first()
            .copied()
            .or_else(|| (by_source.len() == 1).then(|| by_source[0]))
            .or_else(|| (by_base.len() == 1).then(|| by_base[0]));

        if let Some(index) = selected {
            let generated = value_text(items[index].get(field));
            if generated != overlay.base_value && generated != overlay.counsel_value {
                conflicts.push(ConflictCandidate {
                    overlay_event_id: overlay.latest_event_id.clone(),
                    target_key: overlay.target_key.clone(),
                    target_kind: overlay.target_kind.clone(),
                    conflict_kind: "MATERIAL_DISAGREEMENT".into(),
                    generated_value: Some(generated),
                    counsel_value: overlay.counsel_value.clone(),
                    anchor: overlay.anchor.clone(),
                });
            }
            apply_overlay(&mut items[index], overlay);
        } else {
            let ambiguous = by_source.len() > 1 || by_base.len() > 1;
            let mut restored = None;
            if let Some(old) = prior
                .and_then(|g| g.get(collection))
                .and_then(Value::as_array)
            {
                restored = old
                    .iter()
                    .find(|item| {
                        overlay
                            .item_id
                            .as_deref()
                            .is_some_and(|id| item_id(item, collection) == Some(id))
                    })
                    .or_else(|| old.iter().find(|item| anchor_matches(item, overlay)))
                    .cloned();
            }
            if let Some(mut old) = restored {
                apply_overlay(&mut old, overlay);
                items.push(old);
            }
            conflicts.push(ConflictCandidate {
                overlay_event_id: overlay.latest_event_id.clone(),
                target_key: overlay.target_key.clone(),
                target_kind: overlay.target_kind.clone(),
                conflict_kind: if ambiguous {
                    "AMBIGUOUS_MATCH"
                } else {
                    "TARGET_DELETED"
                }
                .into(),
                generated_value: None,
                counsel_value: overlay.counsel_value.clone(),
                anchor: overlay.anchor.clone(),
            });
        }
    }

    if let Some(old) = prior {
        for collection in [
            "nodes",
            "edges",
            "parties",
            "chronology",
            "statutes",
            "evidence",
            "prayer_relief",
            "contradictions",
            "counsel_facts",
        ] {
            preserve_authoritative(old, &mut next, collection);
        }
    }
    next["authority"] = json!({"merge_engine_id": MERGE_ENGINE_ID, "precedence": "active_counsel_overlay_over_generated", "open_conflicts_from_merge": conflicts.len()});
    (next, conflicts)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: &str, text: &str, hash: &str) -> Value {
        json!({"event_id":id,"summary":text,"provenance":{"source":{"doc_id":"d","content_hash":hash},"sources":[{"doc_id":"d","content_hash":hash}],"confidence":0.8,"origin":"EXTRACTED","pipeline_version":"p","recorded_at":"2026-08-14T00:00:00Z","counsel_confirmed":false,"correction_lineage":[]}})
    }
    fn overlay() -> AuthorityOverlay {
        AuthorityOverlay {
            target_key: "cfat:v1:k".into(),
            latest_event_id: "ev-counsel".into(),
            target_kind: "graph_item".into(),
            collection: Some("chronology".into()),
            item_id: Some("event_1".into()),
            field_name: Some("summary".into()),
            doc_id: None,
            page_index: None,
            word_index: None,
            base_value: "machine old".into(),
            counsel_value: "counsel truth".into(),
            anchor: json!({"source_content_hash":"h1","source_doc_id":"d"}),
        }
    }
    #[test]
    fn overlay_survives_changed_generation_with_conflict() {
        let prior = json!({"chronology":[item("event_1","machine old","h1")]});
        let next = json!({"chronology":[item("event_9","machine disagrees","h1")],"nodes":[],"edges":[],"parties":[],"statutes":[],"evidence":[],"prayer_relief":[],"contradictions":[],"counsel_facts":[]});
        let (g, c) = merge_graph(Some(&prior), next, &[overlay()]);
        assert_eq!(g["chronology"][0]["summary"], "counsel truth");
        assert_eq!(c[0].conflict_kind, "MATERIAL_DISAGREEMENT");
    }
    #[test]
    fn deleted_counsel_item_is_restored() {
        let prior = json!({"chronology":[item("event_1","counsel truth","h1")]});
        let next = json!({"chronology":[],"nodes":[],"edges":[],"parties":[],"statutes":[],"evidence":[],"prayer_relief":[],"contradictions":[],"counsel_facts":[]});
        let (g, c) = merge_graph(Some(&prior), next, &[overlay()]);
        assert_eq!(g["chronology"][0]["summary"], "counsel truth");
        assert_eq!(c[0].conflict_kind, "TARGET_DELETED");
    }
}
