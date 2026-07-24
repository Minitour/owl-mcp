//! OWL 2 EL consistency checking and inferred-subsumption materialization via whelk.

use std::collections::HashSet;
use std::path::Path;

use horned_owl::curie::PrefixMapping;
use horned_owl::model::*;
use horned_owl::ontology::set::SetOntology;
use whelk::whelk::model::ConceptData;
use whelk::whelk::owl::translate_ontology;
use whelk::whelk::reasoner::{assert as whelk_assert, ReasonerState};

use crate::ontology::owl_api::{write_ontology_to_path, OwlApiError};

const OWL_THING: &str = "http://www.w3.org/2002/07/owl#Thing";
const OWL_NOTHING: &str = "http://www.w3.org/2002/07/owl#Nothing";

/// JSON-serializable report returned by [`check`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConsistencyReport {
    /// Whether the merged ontology is consistent under OWL 2 EL (whelk).
    ///
    /// `false` when `owl:Thing` is unsatisfiable or any named class is equivalent
    /// to `owl:Nothing`. Full OWL 2 DL inconsistency (cardinality, etc.) is not detected.
    pub consistent: bool,
    /// Named classes inferred equivalent to `owl:Nothing` (empty when all are satisfiable).
    pub unsatisfiable_classes: Vec<String>,
    /// Number of newly inferred `SubClassOf` axioms (set when materialization is requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inferred_axioms_count: Option<usize>,
    /// Reasoner that actually ran (`"whelk"`).
    pub reasoner: String,
}

/// Merge several ontologies into one by unioning all axioms.
pub fn merge_ontologies(ontologies: &[&SetOntology<ArcStr>]) -> SetOntology<ArcStr> {
    let mut merged = SetOntology::new();
    for onto in ontologies {
        for ac in onto.iter() {
            merged.insert(ac.clone());
        }
    }
    merged
}

/// Collect named classes inferred to be subclasses of `owl:Nothing`.
fn unsatisfiable_named_classes(state: &ReasonerState) -> Vec<String> {
    let bottom = state.interner.bottom();
    let Some(subs) = state.closure_subs_by_superclass.get(&bottom) else {
        return Vec::new();
    };

    let mut out: Vec<String> = subs
        .iter()
        .filter_map(|&id| {
            if let ConceptData::AtomicConcept(iri) = state.interner.concept_data(id) {
                let s = iri.as_str();
                if s != OWL_NOTHING && s != OWL_THING {
                    return Some(s.to_string());
                }
            }
            None
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Whether `owl:Thing` is subsumed by `owl:Nothing` (ontology-level inconsistency).
fn thing_unsatisfiable(state: &ReasonerState) -> bool {
    let top = state.interner.top();
    let bottom = state.interner.bottom();
    state.is_subclass_of(top, bottom)
}

/// Collect asserted named `SubClassOf` pairs `(sub_iri, super_iri)`.
fn asserted_named_subclass_pairs(ontology: &SetOntology<ArcStr>) -> HashSet<(String, String)> {
    let mut pairs = HashSet::new();
    for ac in ontology.iter() {
        if let Component::SubClassOf(SubClassOf { sub, sup }) = &ac.component {
            if let (
                ClassExpression::Class(Class(sub_iri)),
                ClassExpression::Class(Class(sup_iri)),
            ) = (sub, sup)
            {
                pairs.insert((sub_iri.as_ref().to_string(), sup_iri.as_ref().to_string()));
            }
        }
    }
    pairs
}

/// Inferred named subsumptions that are not already asserted.
///
/// Includes `SubClassOf(C owl:Nothing)` for unsatisfiable classes so materialization
/// witnesses inconsistency. Excludes reflexive pairs, `owl:Nothing` as subclass, and
/// `owl:Thing` as subclass.
pub fn inferred_subclass_pairs(
    ontology: &SetOntology<ArcStr>,
    state: &ReasonerState,
) -> Vec<(String, String)> {
    let asserted = asserted_named_subclass_pairs(ontology);
    let mut pairs: Vec<(String, String)> = state
        .named_subsumptions()
        .into_iter()
        .filter_map(|(sub, sup)| {
            if sub == sup || sub == OWL_NOTHING || sub == OWL_THING {
                return None;
            }
            let key = (sub.to_string(), sup.to_string());
            if asserted.contains(&key) {
                return None;
            }
            Some(key)
        })
        .collect();
    pairs.sort();
    pairs.dedup();
    pairs
}

/// Run the reasoner and return a copy of `ontology` with inferred named `SubClassOf` axioms added.
pub fn reason_and_materialize(ontology: &SetOntology<ArcStr>) -> SetOntology<ArcStr> {
    let translated = translate_ontology(ontology);
    let state = whelk_assert(&translated);
    materialize_inferences(ontology, &state).0
}

/// Insert inferred `SubClassOf` axioms into a clone of `ontology` and return it with the count.
pub fn materialize_inferences(
    ontology: &SetOntology<ArcStr>,
    state: &ReasonerState,
) -> (SetOntology<ArcStr>, usize) {
    let pairs = inferred_subclass_pairs(ontology, state);
    let count = pairs.len();
    let mut out = ontology.clone();
    let build = Build::<ArcStr>::new_arc();
    for (sub, sup) in pairs {
        let axiom = SubClassOf {
            sub: ClassExpression::Class(build.class(sub)),
            sup: ClassExpression::Class(build.class(sup)),
        };
        out.insert(AnnotatedComponent::from(Component::SubClassOf(axiom)));
    }
    (out, count)
}

/// Run the whelk EL reasoner over `ontology` and return a consistency report.
///
/// When `want_inferred` is true, also computes how many new named `SubClassOf`
/// axioms would be added by materialization (without mutating the ontology).
pub fn check(ontology: &SetOntology<ArcStr>, want_inferred: bool) -> ConsistencyReport {
    let translated = translate_ontology(ontology);
    let state = whelk_assert(&translated);

    let unsatisfiable_classes = unsatisfiable_named_classes(&state);
    let consistent = !thing_unsatisfiable(&state) && unsatisfiable_classes.is_empty();

    let inferred_axioms_count = if want_inferred {
        Some(inferred_subclass_pairs(ontology, &state).len())
    } else {
        None
    };

    ConsistencyReport {
        consistent,
        unsatisfiable_classes,
        inferred_axioms_count,
        reasoner: "whelk".to_string(),
    }
}

/// Reason over `ontology`, optionally write a materialized copy to `output_path`,
/// and return the consistency report (always including `inferred_axioms_count`
/// when writing, or when `want_inferred` is true).
pub fn check_and_maybe_write(
    ontology: &SetOntology<ArcStr>,
    prefixes: &PrefixMapping,
    output_path: Option<&Path>,
    want_inferred: bool,
) -> Result<(ConsistencyReport, Option<SetOntology<ArcStr>>), OwlApiError> {
    let translated = translate_ontology(ontology);
    let state = whelk_assert(&translated);

    let unsatisfiable_classes = unsatisfiable_named_classes(&state);
    let consistent = !thing_unsatisfiable(&state) && unsatisfiable_classes.is_empty();

    let (materialized, inferred_count) = if output_path.is_some() || want_inferred {
        let (mat, count) = materialize_inferences(ontology, &state);
        (Some(mat), Some(count))
    } else {
        (None, None)
    };

    if let Some(path) = output_path {
        let mat = materialized
            .as_ref()
            .expect("materialized ontology required when output_path is set");
        write_ontology_to_path(path, mat, prefixes)?;
    }

    Ok((
        ConsistencyReport {
            consistent,
            unsatisfiable_classes,
            inferred_axioms_count: inferred_count,
            reasoner: "whelk".to_string(),
        },
        materialized,
    ))
}

/// Normalize a reasoner id: accept `elk` / `whelk` (case-insensitive). Returns the
/// canonical id `"whelk"`, or an error message for unsupported reasoners.
pub fn normalize_reasoner_id(reasoner: Option<&str>) -> Result<&'static str, String> {
    match reasoner {
        None => Ok("whelk"),
        Some(s) => match s.trim().to_ascii_lowercase().as_str() {
            "whelk" | "elk" => Ok("whelk"),
            other => Err(format!(
                "Unsupported reasoner '{other}'. Only OWL 2 EL reasoners are available: whelk (alias: elk)."
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use horned_owl::io::ofn::reader::read_with_build as ofn_read;
    use std::io::{BufReader, Cursor};

    fn parse_ofn(src: &str) -> SetOntology<ArcStr> {
        let build = Build::new_arc();
        let reader = BufReader::new(Cursor::new(src.as_bytes()));
        let (onto, _): (SetOntology<ArcStr>, _) = ofn_read(reader, &build).unwrap();
        onto
    }

    #[test]
    fn consistent_el_ontology() {
        let onto = parse_ofn(
            r#"Ontology(<http://example.org/consistent>
  Declaration(Class(<http://example.org/Animal>))
  Declaration(Class(<http://example.org/Dog>))
  SubClassOf(<http://example.org/Dog> <http://example.org/Animal>)
)"#,
        );
        let report = check(&onto, true);
        assert!(report.consistent);
        assert!(report.unsatisfiable_classes.is_empty());
        assert_eq!(report.reasoner, "whelk");
        assert!(report.inferred_axioms_count.is_some());
    }

    #[test]
    fn unsatisfiable_via_disjoint_parents() {
        // Unsat ⊑ A, Unsat ⊑ B, DisjointClasses(A B) ⇒ Unsat ⊑ Nothing
        let onto = parse_ofn(
            r#"Ontology(<http://example.org/unsat>
  Declaration(Class(<http://example.org/A>))
  Declaration(Class(<http://example.org/B>))
  Declaration(Class(<http://example.org/Unsat>))
  DisjointClasses(<http://example.org/A> <http://example.org/B>)
  SubClassOf(<http://example.org/Unsat> <http://example.org/A>)
  SubClassOf(<http://example.org/Unsat> <http://example.org/B>)
)"#,
        );
        let report = check(&onto, false);
        assert!(
            !report.consistent
                || report
                    .unsatisfiable_classes
                    .iter()
                    .any(|c| c.contains("Unsat")),
            "expected inconsistent or Unsat listed: {:?}",
            report
        );
        assert!(
            report
                .unsatisfiable_classes
                .iter()
                .any(|c| c == "http://example.org/Unsat"),
            "Unsat should be listed: {:?}",
            report.unsatisfiable_classes
        );
        assert!(!report.consistent);
    }

    #[test]
    fn materialize_includes_unsatisfiable_bottom_subsumption() {
        let onto = parse_ofn(
            r#"Ontology(<http://example.org/unsat>
  Declaration(Class(<http://example.org/A>))
  Declaration(Class(<http://example.org/B>))
  Declaration(Class(<http://example.org/Unsat>))
  DisjointClasses(<http://example.org/A> <http://example.org/B>)
  SubClassOf(<http://example.org/Unsat> <http://example.org/A>)
  SubClassOf(<http://example.org/Unsat> <http://example.org/B>)
)"#,
        );
        let translated = translate_ontology(&onto);
        let state = whelk_assert(&translated);
        let pairs = inferred_subclass_pairs(&onto, &state);
        assert!(
            pairs
                .iter()
                .any(|(s, p)| { s == "http://example.org/Unsat" && p == OWL_NOTHING }),
            "materialization should include Unsat ⊑ Nothing: {:?}",
            pairs
        );

        let (mat, _) = materialize_inferences(&onto, &state);
        let has_bottom = mat.iter().any(|ac| {
            matches!(
                &ac.component,
                Component::SubClassOf(SubClassOf {
                    sub: ClassExpression::Class(Class(sub)),
                    sup: ClassExpression::Class(Class(sup)),
                }) if sub.as_ref() == "http://example.org/Unsat"
                    && sup.as_ref() == OWL_NOTHING
            )
        });
        assert!(
            has_bottom,
            "materialized ontology should contain Unsat ⊑ Nothing"
        );
    }

    #[test]
    fn merge_two_ontologies() {
        let schema = parse_ofn(
            r#"Ontology(<http://example.org/schema>
  Declaration(Class(<http://example.org/Animal>))
  Declaration(Class(<http://example.org/Dog>))
  SubClassOf(<http://example.org/Dog> <http://example.org/Animal>)
)"#,
        );
        let abox = parse_ofn(
            r#"Ontology(<http://example.org/abox>
  Declaration(NamedIndividual(<http://example.org/fido>))
  ClassAssertion(<http://example.org/Dog> <http://example.org/fido>)
)"#,
        );
        let merged = merge_ontologies(&[&schema, &abox]);
        let report = check(&merged, false);
        assert!(report.consistent);
        assert!(report.unsatisfiable_classes.is_empty());
    }

    #[test]
    fn materialize_writes_inferred_subclass() {
        // GrandDog ⊑ Dog ⊑ Animal ⇒ infer GrandDog ⊑ Animal
        let onto = parse_ofn(
            r#"Ontology(<http://example.org/hier>
  Declaration(Class(<http://example.org/Animal>))
  Declaration(Class(<http://example.org/Dog>))
  Declaration(Class(<http://example.org/GrandDog>))
  SubClassOf(<http://example.org/Dog> <http://example.org/Animal>)
  SubClassOf(<http://example.org/GrandDog> <http://example.org/Dog>)
)"#,
        );
        let translated = translate_ontology(&onto);
        let state = whelk_assert(&translated);
        let pairs = inferred_subclass_pairs(&onto, &state);
        assert!(
            pairs.iter().any(|(s, p)| {
                s == "http://example.org/GrandDog" && p == "http://example.org/Animal"
            }),
            "expected GrandDog ⊑ Animal in inferred pairs: {:?}",
            pairs
        );

        let dir = tempfile::tempdir().unwrap();
        let out_path = dir.path().join("reasoned.ofn");
        let (report, _) =
            check_and_maybe_write(&onto, &PrefixMapping::default(), Some(&out_path), true).unwrap();
        assert!(report.consistent);
        assert!(report.inferred_axioms_count.unwrap() >= 1);

        let content = std::fs::read_to_string(&out_path).unwrap();
        assert!(
            content.contains("GrandDog") && content.contains("Animal"),
            "reasoned file should mention GrandDog and Animal: {content}"
        );
        // Reload to ensure valid OWL
        let reloaded = parse_ofn(&content);
        assert!(check(&reloaded, false).consistent);
    }

    #[test]
    fn with_reasoning_exposes_inferred_subclass_to_sparql() {
        // GrandDog ⊑ Dog ⊑ Animal — without reasoning, GrandDog ⊑ Animal is not asserted
        let onto = parse_ofn(
            r#"Ontology(<http://example.org/hier>
  Declaration(Class(<http://example.org/Animal>))
  Declaration(Class(<http://example.org/Dog>))
  Declaration(Class(<http://example.org/GrandDog>))
  SubClassOf(<http://example.org/Dog> <http://example.org/Animal>)
  SubClassOf(<http://example.org/GrandDog> <http://example.org/Dog>)
)"#,
        );
        let asserted_bytes = crate::ontology::owl_api::ontology_to_rdf_bytes(&onto).unwrap();
        let q = r#"PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
            SELECT ?s WHERE { ?s rdfs:subClassOf <http://example.org/Animal> }"#;
        let asserted = crate::sparql::query(&[asserted_bytes], q).unwrap();
        let v: serde_json::Value = serde_json::from_str(&asserted).unwrap();
        let asserted_subs: Vec<&str> = v["results"]["bindings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["s"]["value"].as_str().unwrap())
            .collect();
        assert!(
            asserted_subs.contains(&"http://example.org/Dog"),
            "asserted should include Dog: {:?}",
            asserted_subs
        );
        assert!(
            !asserted_subs.contains(&"http://example.org/GrandDog"),
            "without reasoning GrandDog should not appear: {:?}",
            asserted_subs
        );

        let reasoned = reason_and_materialize(&onto);
        let reasoned_bytes = crate::ontology::owl_api::ontology_to_rdf_bytes(&reasoned).unwrap();
        let reasoned_out = crate::sparql::query(&[reasoned_bytes], q).unwrap();
        let v2: serde_json::Value = serde_json::from_str(&reasoned_out).unwrap();
        let reasoned_subs: Vec<&str> = v2["results"]["bindings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["s"]["value"].as_str().unwrap())
            .collect();
        assert!(
            reasoned_subs.contains(&"http://example.org/GrandDog"),
            "with reasoning GrandDog should appear: {:?}",
            reasoned_subs
        );
    }

    #[test]
    fn normalize_reasoner_accepts_elk_and_whelk() {
        assert_eq!(normalize_reasoner_id(None).unwrap(), "whelk");
        assert_eq!(normalize_reasoner_id(Some("ELK")).unwrap(), "whelk");
        assert_eq!(normalize_reasoner_id(Some("whelk")).unwrap(), "whelk");
        assert!(normalize_reasoner_id(Some("hermit")).is_err());
    }
}
