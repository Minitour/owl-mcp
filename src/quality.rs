use std::collections::{HashMap, HashSet};

use horned_owl::model::*;
use horned_owl::ontology::set::SetOntology;
use whelk::whelk::model::{ConceptData, ConceptId};
use whelk::whelk::owl::translate_ontology;
use whelk::whelk::reasoner::{assert as whelk_assert, ReasonerState};

const OWL_THING: &str = "http://www.w3.org/2002/07/owl#Thing";
const OWL_NOTHING: &str = "http://www.w3.org/2002/07/owl#Nothing";
const RDFS_LABEL: &str = "http://www.w3.org/2000/01/rdf-schema#label";
const RDFS_COMMENT: &str = "http://www.w3.org/2000/01/rdf-schema#comment";

// ── Output structs ────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct QualityReport {
    pub basic_metrics: BasicMetrics,
    pub metrics: MetricValues,
    pub metrics_scaled: MetricValuesScaled,
    pub model: QualityModel,
}

#[derive(Debug, serde::Serialize)]
pub struct BasicMetrics {
    pub num_classes: usize,
    pub num_leaf_classes: usize,
    pub num_paths: usize,
    pub num_instances: usize,
    pub num_object_properties: usize,
    pub num_data_properties: usize,
    pub max_depth: usize,
    pub sum_of_path_lengths: usize,
    pub sum_of_annotations: usize,
    pub sum_of_attributes: usize,
    pub sum_of_class_relationships: usize,
    pub sum_of_direct_ancestors: usize,
    pub sum_of_direct_ancestors_of_leaves: usize,
    pub sum_of_thing_relationships: usize,
    pub num_multi_parent_classes: usize,
    pub sum_of_ancestors_of_multi_parent_classes: usize,
    pub num_property_usages: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct MetricValues {
    #[serde(rename = "ANOnto")]
    pub an: f64,
    #[serde(rename = "AROnto")]
    pub ar: f64,
    #[serde(rename = "CBOOnto")]
    pub cbo: f64,
    #[serde(rename = "CBOnto2")]
    pub cb2: f64,
    #[serde(rename = "CROnto")]
    pub cr: f64,
    #[serde(rename = "DITOnto")]
    pub dit: f64,
    #[serde(rename = "INROnto")]
    pub inr: f64,
    #[serde(rename = "LCOMOnto")]
    pub lcom: f64,
    #[serde(rename = "NACOnto")]
    pub nac: f64,
    #[serde(rename = "NOCOnto")]
    pub noc: f64,
    #[serde(rename = "NOMOnto")]
    pub nom: f64,
    #[serde(rename = "POnto")]
    pub p: f64,
    #[serde(rename = "PROnto")]
    pub pr: f64,
    #[serde(rename = "RFCOnto")]
    pub rfc: f64,
    #[serde(rename = "RROnto")]
    pub rr: f64,
    #[serde(rename = "TMOnto")]
    pub tm: f64,
    #[serde(rename = "TMOnto2")]
    pub tm2: f64,
    #[serde(rename = "WMCOnto")]
    pub wmc: f64,
    #[serde(rename = "WMCOnto2")]
    pub wmc2: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct MetricValuesScaled {
    #[serde(rename = "ANOnto")]
    pub an: f64,
    #[serde(rename = "AROnto")]
    pub ar: f64,
    #[serde(rename = "CBOOnto")]
    pub cbo: f64,
    #[serde(rename = "CBOnto2")]
    pub cb2: f64,
    #[serde(rename = "CROnto")]
    pub cr: f64,
    #[serde(rename = "DITOnto")]
    pub dit: f64,
    #[serde(rename = "INROnto")]
    pub inr: f64,
    #[serde(rename = "LCOMOnto")]
    pub lcom: f64,
    #[serde(rename = "NACOnto")]
    pub nac: f64,
    #[serde(rename = "NOCOnto")]
    pub noc: f64,
    #[serde(rename = "NOMOnto")]
    pub nom: f64,
    #[serde(rename = "POnto")]
    pub p: f64,
    #[serde(rename = "PROnto")]
    pub pr: f64,
    #[serde(rename = "RFCOnto")]
    pub rfc: f64,
    #[serde(rename = "RROnto")]
    pub rr: f64,
    #[serde(rename = "TMOnto")]
    pub tm: f64,
    #[serde(rename = "TMOnto2")]
    pub tm2: f64,
    #[serde(rename = "WMCOnto")]
    pub wmc: f64,
    #[serde(rename = "WMCOnto2")]
    pub wmc2: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct QualityModel {
    pub oquare_value: f64,
    pub structural: CharacteristicDetail,
    pub functional_adequacy: CharacteristicDetail,
    pub maintainability: CharacteristicDetail,
    pub operability: CharacteristicDetail,
    pub reliability: CharacteristicDetail,
    pub transferability: CharacteristicDetail,
    pub compatibility: CharacteristicDetail,
}

#[derive(Debug, serde::Serialize)]
pub struct CharacteristicDetail {
    pub value: f64,
    pub subcharacteristics: HashMap<String, f64>,
}

// ── Scaling ───────────────────────────────────────────────────────────────────

// Scale 01: LCOM  (>8)=1, (6,8]=2, (4,6]=3, (2,4]=4, <=2=5
fn scale_lcom(v: f64) -> f64 {
    if v > 8.0 {
        1.0
    } else if v > 6.0 {
        2.0
    } else if v > 4.0 {
        3.0
    } else if v > 2.0 {
        4.0
    } else {
        5.0
    }
}

// Scale 02: WMC  (>15)=1, (11,15]=2, (8,11]=3, (5,8]=4, <5=5
fn scale_wmc(v: f64) -> f64 {
    if v > 15.0 {
        1.0
    } else if v > 11.0 {
        2.0
    } else if v > 8.0 {
        3.0
    } else if v > 5.0 {
        4.0
    } else {
        5.0
    }
}

// Scale 03: CBO, DIT, NAC, NOM  (>8)=1, (6,8]=2, (4,6]=3, (2,4]=4, [1,2]=5
fn scale_03(v: f64) -> f64 {
    if v > 8.0 {
        1.0
    } else if v > 6.0 {
        2.0
    } else if v > 4.0 {
        3.0
    } else if v > 2.0 {
        4.0
    } else {
        5.0
    }
}

// Scale 04: NOC, RFC  (>12)=1, (8,12]=2, (6,8]=3, (3,6]=4, [1,3]=5
fn scale_04(v: f64) -> f64 {
    if v > 12.0 {
        1.0
    } else if v > 8.0 {
        2.0
    } else if v > 6.0 {
        3.0
    } else if v > 3.0 {
        4.0
    } else {
        5.0
    }
}

// Scale 05: AN, AR, CR, INR, RR (and inverted variants)
// [0,0.2]=1, (0.2,0.4]=2, (0.4,0.6]=3, (0.6,0.8]=4, >0.8=5
fn scale_05(v: f64) -> f64 {
    if v > 0.8 {
        5.0
    } else if v > 0.6 {
        4.0
    } else if v > 0.4 {
        3.0
    } else if v > 0.2 {
        2.0
    } else {
        1.0
    }
}

// Scale 09: TM  (>0.4)=1, (0.3,0.4]=2, (0.2,0.3]=3, (0.1,0.2]=4, [0,0.1]=5
fn scale_tm(v: f64) -> f64 {
    if v > 0.4 {
        1.0
    } else if v > 0.3 {
        2.0
    } else if v > 0.2 {
        3.0
    } else if v > 0.1 {
        4.0
    } else {
        5.0
    }
}

fn safe_invert(v: f64) -> f64 {
    if v == 0.0 {
        f64::INFINITY
    } else {
        1.0 / v
    }
}

// ── Influence ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum Influence {
    Positive,
    Negative,
}

fn influenced(scaled: f64, influence: Influence) -> f64 {
    match influence {
        Influence::Positive => scaled,
        Influence::Negative => 6.0 - scaled,
    }
}

// ── Subcharacteristic definitions ─────────────────────────────────────────────

struct ScaledMetrics {
    an: f64,
    ar: f64,
    cbo: f64,
    cr: f64,
    dit: f64,
    inr: f64,
    lcom: f64,
    nac: f64,
    noc: f64,
    nom: f64,
    rfc: f64,
    rr: f64,
    tm: f64,
    wmc: f64,
    consistency: f64,
    formal_degree: f64,
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn compute_subcharacteristics(s: &ScaledMetrics) -> HashMap<String, f64> {
    use Influence::*;
    let mut sc: HashMap<String, f64> = HashMap::new();

    // ── Structural ──
    sc.insert(
        "formalisation".into(),
        mean(&[influenced(s.formal_degree, Positive)]),
    );
    sc.insert(
        "formalRelationSupport".into(),
        mean(&[influenced(s.rr, Positive)]),
    );
    sc.insert("cohesion".into(), mean(&[influenced(s.lcom, Positive)]));
    sc.insert(
        "consistency".into(),
        mean(&[influenced(s.consistency, Positive)]),
    );
    sc.insert("redundancy".into(), mean(&[influenced(s.an, Positive)]));
    sc.insert("tagledness".into(), mean(&[influenced(s.tm, Positive)]));

    // ── Functional Adequacy ──
    sc.insert(
        "referenceOntology".into(),
        mean(&[
            influenced(s.rr, Positive),
            influenced(s.formal_degree, Positive),
        ]),
    );
    sc.insert(
        "controlledVocabulary".into(),
        mean(&[influenced(s.an, Positive)]),
    );
    sc.insert(
        "schemaAndValueReconciliation".into(),
        mean(&[
            influenced(s.ar, Positive),
            influenced(s.rr, Positive),
            influenced(s.consistency, Positive),
            influenced(s.formal_degree, Positive),
        ]),
    );
    sc.insert(
        "consistentSearchAndQuery".into(),
        mean(&[
            influenced(s.an, Positive),
            influenced(s.ar, Positive),
            influenced(s.inr, Positive),
            influenced(s.rr, Positive),
            influenced(s.formal_degree, Positive),
        ]),
    );
    sc.insert(
        "knowledgeAcquisition".into(),
        mean(&[
            influenced(s.ar, Positive),
            influenced(s.nom, Positive),
            influenced(s.rr, Positive),
        ]),
    );
    sc.insert(
        "clusteringAndSimilarity".into(),
        mean(&[influenced(s.ar, Positive), influenced(s.rr, Positive)]),
    );
    sc.insert(
        "indexingAndLinking".into(),
        mean(&[
            influenced(s.ar, Positive),
            influenced(s.inr, Positive),
            influenced(s.rr, Positive),
        ]),
    );
    sc.insert(
        "resultsRepresentation".into(),
        mean(&[influenced(s.ar, Positive), influenced(s.cr, Positive)]),
    );
    sc.insert(
        "textAnalysis".into(),
        mean(&[influenced(s.formal_degree, Positive)]),
    );
    sc.insert(
        "guidanceAndDecisionTrees".into(),
        mean(&[influenced(s.ar, Positive), influenced(s.inr, Positive)]),
    );
    sc.insert(
        "knowledgeReuse".into(),
        mean(&[
            influenced(s.an, Positive),
            influenced(s.ar, Positive),
            influenced(s.inr, Positive),
            influenced(s.lcom, Positive),
            influenced(s.nac, Positive),
            influenced(s.nom, Positive),
            influenced(s.consistency, Positive),
            influenced(s.formal_degree, Positive),
        ]),
    );
    sc.insert(
        "infering".into(),
        mean(&[influenced(s.formal_degree, Positive)]),
    );

    // ── Maintainability ──
    sc.insert(
        "modularity".into(),
        mean(&[influenced(s.cbo, Positive), influenced(s.wmc, Positive)]),
    );
    sc.insert(
        "reusability".into(),
        mean(&[
            influenced(s.cbo, Positive),
            influenced(s.dit, Positive),
            influenced(s.noc, Negative),
            influenced(s.nom, Positive),
            influenced(s.rfc, Positive),
            influenced(s.wmc, Positive),
        ]),
    );
    sc.insert(
        "analysability".into(),
        mean(&[
            influenced(s.cbo, Positive),
            influenced(s.dit, Positive),
            influenced(s.lcom, Positive),
            influenced(s.nom, Positive),
            influenced(s.rfc, Positive),
            influenced(s.wmc, Positive),
        ]),
    );
    sc.insert(
        "changeability".into(),
        mean(&[
            influenced(s.cbo, Positive),
            influenced(s.dit, Positive),
            influenced(s.lcom, Positive),
            influenced(s.noc, Positive),
            influenced(s.nom, Positive),
            influenced(s.rfc, Positive),
            influenced(s.wmc, Positive),
        ]),
    );
    sc.insert(
        "modificationStability".into(),
        mean(&[
            influenced(s.cbo, Positive),
            influenced(s.lcom, Positive),
            influenced(s.noc, Positive),
            influenced(s.rfc, Positive),
            influenced(s.wmc, Positive),
        ]),
    );
    sc.insert(
        "testeability".into(),
        mean(&[
            influenced(s.cbo, Positive),
            influenced(s.dit, Positive),
            influenced(s.lcom, Positive),
            influenced(s.nom, Positive),
            influenced(s.rfc, Positive),
            influenced(s.wmc, Positive),
        ]),
    );

    // ── Reliability ──
    sc.insert(
        "recoverability".into(),
        mean(&[
            influenced(s.dit, Negative),
            influenced(s.lcom, Negative),
            influenced(s.nom, Negative),
            influenced(s.wmc, Negative),
        ]),
    );
    sc.insert("availability".into(), mean(&[influenced(s.lcom, Positive)]));

    // ── Operability ──
    sc.insert(
        "lerneability".into(),
        mean(&[
            influenced(s.cbo, Positive),
            influenced(s.lcom, Positive),
            influenced(s.noc, Negative),
            influenced(s.nom, Positive),
            influenced(s.rfc, Positive),
            influenced(s.wmc, Positive),
        ]),
    );

    // ── Transferability ──
    sc.insert(
        "adaptability".into(),
        mean(&[
            influenced(s.cbo, Positive),
            influenced(s.dit, Positive),
            influenced(s.rfc, Positive),
            influenced(s.wmc, Positive),
        ]),
    );

    // ── Compatibility ──
    sc.insert(
        "replaceability".into(),
        mean(&[
            influenced(s.dit, Positive),
            influenced(s.noc, Positive),
            influenced(s.nom, Positive),
            influenced(s.wmc, Positive),
        ]),
    );

    sc
}

fn char_mean(sc: &HashMap<String, f64>, keys: &[&str]) -> (f64, HashMap<String, f64>) {
    let sub: HashMap<String, f64> = keys
        .iter()
        .filter_map(|k| sc.get(*k).map(|v| (k.to_string(), *v)))
        .collect();
    let vals: Vec<f64> = keys.iter().filter_map(|k| sc.get(*k).copied()).collect();
    (mean(&vals), sub)
}

fn compute_characteristics(sc: &HashMap<String, f64>) -> QualityModel {
    let (s_val, s_sub) = char_mean(
        sc,
        &[
            "formalisation",
            "formalRelationSupport",
            "cohesion",
            "consistency",
            "redundancy",
            "tagledness",
        ],
    );
    let (fa_val, fa_sub) = char_mean(
        sc,
        &[
            "referenceOntology",
            "controlledVocabulary",
            "schemaAndValueReconciliation",
            "consistentSearchAndQuery",
            "knowledgeAcquisition",
            "clusteringAndSimilarity",
            "indexingAndLinking",
            "resultsRepresentation",
            "textAnalysis",
            "guidanceAndDecisionTrees",
            "knowledgeReuse",
            "infering",
        ],
    );
    let (m_val, m_sub) = char_mean(
        sc,
        &[
            "modularity",
            "reusability",
            "analysability",
            "changeability",
            "modificationStability",
            "testeability",
        ],
    );
    let (o_val, o_sub) = char_mean(sc, &["lerneability"]);
    let (r_val, r_sub) = char_mean(sc, &["recoverability", "availability"]);
    let (t_val, t_sub) = char_mean(sc, &["adaptability"]);
    let (c_val, c_sub) = char_mean(sc, &["replaceability"]);

    let oquare_value = mean(&[s_val, fa_val, m_val, o_val, r_val, t_val, c_val]);

    QualityModel {
        oquare_value,
        structural: CharacteristicDetail {
            value: s_val,
            subcharacteristics: s_sub,
        },
        functional_adequacy: CharacteristicDetail {
            value: fa_val,
            subcharacteristics: fa_sub,
        },
        maintainability: CharacteristicDetail {
            value: m_val,
            subcharacteristics: m_sub,
        },
        operability: CharacteristicDetail {
            value: o_val,
            subcharacteristics: o_sub,
        },
        reliability: CharacteristicDetail {
            value: r_val,
            subcharacteristics: r_sub,
        },
        transferability: CharacteristicDetail {
            value: t_val,
            subcharacteristics: t_sub,
        },
        compatibility: CharacteristicDetail {
            value: c_val,
            subcharacteristics: c_sub,
        },
    }
}

// ── Hierarchy extraction from whelk ───────────────────────────────────────────

/// Build direct parent map from whelk's transitive closure.
/// A direct parent is a superclass with no intermediate named class between it and the subclass.
fn build_direct_parents(
    state: &ReasonerState,
    class_ids: &[ConceptId],
) -> HashMap<ConceptId, HashSet<ConceptId>> {
    let class_set: HashSet<ConceptId> = class_ids.iter().copied().collect();
    let mut direct_parents: HashMap<ConceptId, HashSet<ConceptId>> = HashMap::new();

    for &cls in class_ids {
        let all_supers: HashSet<ConceptId> = state
            .closure_subs_by_subclass
            .get(&cls)
            .map(|s| {
                s.iter()
                    .copied()
                    .filter(|&sup| sup != cls && class_set.contains(&sup))
                    .collect()
            })
            .unwrap_or_default();

        // A parent P is direct if no other superclass S of cls is also a subclass of P
        let mut direct = all_supers.clone();
        for &p in &all_supers {
            for &q in &all_supers {
                if p != q {
                    let q_is_sub_of_p = state
                        .closure_subs_by_subclass
                        .get(&q)
                        .is_some_and(|s| s.iter().any(|&x| x == p));
                    if q_is_sub_of_p {
                        direct.remove(&p);
                    }
                }
            }
        }
        direct_parents.insert(cls, direct);
    }

    direct_parents
}

fn build_direct_children(
    direct_parents: &HashMap<ConceptId, HashSet<ConceptId>>,
) -> HashMap<ConceptId, HashSet<ConceptId>> {
    let mut children: HashMap<ConceptId, HashSet<ConceptId>> = HashMap::new();
    for (&child, parents) in direct_parents {
        for &parent in parents {
            children.entry(parent).or_default().insert(child);
        }
    }
    children
}

/// Enumerate all paths from a leaf to Thing, return (sum_of_path_lengths, num_paths, max_depth).
fn compute_paths(
    thing_id: ConceptId,
    leaf_classes: &[ConceptId],
    direct_parents: &HashMap<ConceptId, HashSet<ConceptId>>,
) -> (usize, usize, usize) {
    let mut total_length = 0usize;
    let mut total_paths = 0usize;
    let mut max_depth = 0usize;

    for &leaf in leaf_classes {
        let mut stack: Vec<(ConceptId, usize, HashSet<ConceptId>)> =
            vec![(leaf, 0, HashSet::new())];
        while let Some((current, depth, visited)) = stack.pop() {
            if current == thing_id {
                total_length += depth;
                total_paths += 1;
                if depth > max_depth {
                    max_depth = depth;
                }
                continue;
            }
            if let Some(parents) = direct_parents.get(&current) {
                if parents.is_empty() && current != thing_id {
                    // No parents and not Thing: treat as depth+1 path to Thing
                    let d = depth + 1;
                    total_length += d;
                    total_paths += 1;
                    if d > max_depth {
                        max_depth = d;
                    }
                } else {
                    for &p in parents {
                        if !visited.contains(&p) {
                            let mut new_visited = visited.clone();
                            new_visited.insert(p);
                            stack.push((p, depth + 1, new_visited));
                        }
                    }
                }
            } else {
                // Class not in map: treat as direct child of Thing
                let d = depth + 1;
                total_length += d;
                total_paths += 1;
                if d > max_depth {
                    max_depth = d;
                }
            }
        }
    }

    (total_length, total_paths, max_depth)
}

// ── Main evaluation ───────────────────────────────────────────────────────────

pub fn evaluate(ontology: &SetOntology<ArcStr>) -> QualityReport {
    // Step 1: Run whelk reasoner
    let translated = translate_ontology(ontology);
    let reasoner = whelk_assert(&translated);

    // Step 2: Collect declared class IRIs (excluding OWL builtins)
    let mut declared_classes: HashSet<String> = HashSet::new();
    let mut num_object_properties: usize = 0;
    let mut num_data_properties: usize = 0;
    let mut num_instances: usize = 0;
    let mut sum_annotations: usize = 0;

    // Maps for counting property usage and attributes
    let mut property_usage_count: usize = 0;
    let mut sum_attributes: usize = 0;

    for ac in ontology.iter() {
        match &ac.component {
            Component::DeclareClass(DeclareClass(Class(iri))) => {
                let s: &str = iri.as_ref();
                if s != OWL_THING && s != OWL_NOTHING {
                    declared_classes.insert(s.to_string());
                }
            }
            Component::DeclareObjectProperty(_) => num_object_properties += 1,
            Component::DeclareDataProperty(_) => num_data_properties += 1,
            Component::DeclareNamedIndividual(_) => num_instances += 1,
            _ => {}
        }
    }

    // Count annotations (rdfs:label + rdfs:comment on classes)
    for ac in ontology.iter() {
        if let Component::AnnotationAssertion(AnnotationAssertion {
            subject: AnnotationSubject::IRI(subject_iri),
            ann,
        }) = &ac.component
        {
            let subj: &str = subject_iri.as_ref();
            let prop: &str = ann.ap.0.as_ref();
            if declared_classes.contains(subj) && (prop == RDFS_LABEL || prop == RDFS_COMMENT) {
                sum_annotations += 1;
            }
        }
    }

    // Count property usage: how many axioms reference each property
    let mut obj_prop_iris: HashSet<String> = HashSet::new();
    let mut data_prop_iris: HashSet<String> = HashSet::new();
    for ac in ontology.iter() {
        match &ac.component {
            Component::DeclareObjectProperty(DeclareObjectProperty(ObjectProperty(iri))) => {
                obj_prop_iris.insert(iri.to_string());
            }
            Component::DeclareDataProperty(DeclareDataProperty(DataProperty(iri))) => {
                data_prop_iris.insert(iri.to_string());
            }
            _ => {}
        }
    }

    for ac in ontology.iter() {
        let axiom_str = format!("{:?}", ac.component);
        for prop_iri in obj_prop_iris.iter().chain(data_prop_iris.iter()) {
            if axiom_str.contains(prop_iri.as_str()) {
                property_usage_count += 1;
            }
        }
    }
    // Subtract declarations themselves (they're not "usages")
    property_usage_count = property_usage_count
        .saturating_sub(obj_prop_iris.len())
        .saturating_sub(data_prop_iris.len());

    // Count attributes: data properties with domain axioms pointing to classes
    for ac in ontology.iter() {
        if let Component::DataPropertyDomain(_) = &ac.component {
            sum_attributes += 1;
        }
    }
    // Also count object property domains as attributes per the Java impl
    for ac in ontology.iter() {
        if let Component::ObjectPropertyDomain(_) = &ac.component {
            sum_attributes += 1;
        }
    }

    // Step 3: Build inferred hierarchy from whelk
    // Find ConceptIds for all declared classes + Thing
    let thing_id = reasoner
        .interner
        .find_concept(&ConceptData::AtomicConcept(OWL_THING.to_string()));
    let thing_id = match thing_id {
        Some(id) => id,
        None => reasoner.interner.top(),
    };

    let mut class_ids: Vec<ConceptId> = Vec::new();
    let mut class_id_set: HashSet<ConceptId> = HashSet::new();

    // Include Thing
    class_ids.push(thing_id);
    class_id_set.insert(thing_id);

    for class_iri in &declared_classes {
        if let Some(id) = reasoner
            .interner
            .find_concept(&ConceptData::AtomicConcept(class_iri.clone()))
        {
            if class_id_set.insert(id) {
                class_ids.push(id);
            }
        }
    }

    let num_classes = declared_classes.len();

    // Build direct parent/child maps (only among named classes + Thing)
    let direct_parents = build_direct_parents(&reasoner, &class_ids);
    let direct_children = build_direct_children(&direct_parents);

    // Identify leaf classes (classes with no children, excluding Thing)
    let leaf_classes: Vec<ConceptId> = class_ids
        .iter()
        .copied()
        .filter(|&id| id != thing_id && direct_children.get(&id).is_none_or(|ch| ch.is_empty()))
        .collect();
    let num_leaf_classes = leaf_classes.len();

    // Sum of direct ancestors (inferred)
    let sum_direct_ancestors: usize = class_ids
        .iter()
        .filter(|&&id| id != thing_id)
        .map(|&id| direct_parents.get(&id).map_or(0, |p| p.len()))
        .sum();

    // Sum of direct ancestors of leaf classes
    let sum_direct_ancestors_leaves: usize = leaf_classes
        .iter()
        .map(|&id| direct_parents.get(&id).map_or(0, |p| p.len()))
        .sum();

    // Sum of class relationships (= total direct subclasses across all classes)
    let sum_class_relationships: usize = class_ids
        .iter()
        .map(|&id| direct_children.get(&id).map_or(0, |ch| ch.len()))
        .sum();

    // Sum of Thing relationships (direct children of Thing)
    let sum_thing_relationships: usize = direct_children.get(&thing_id).map_or(0, |ch| ch.len());

    // Multi-parent classes
    let multi_parent_classes: Vec<ConceptId> = class_ids
        .iter()
        .copied()
        .filter(|&id| id != thing_id && direct_parents.get(&id).is_some_and(|p| p.len() > 1))
        .collect();
    let num_multi_parent = multi_parent_classes.len();
    let sum_ancestors_multi_parent: usize = multi_parent_classes
        .iter()
        .map(|&id| direct_parents.get(&id).map_or(0, |p| p.len()))
        .sum();

    // Paths from leaves to Thing
    let (sum_path_lengths, num_paths, max_depth) =
        compute_paths(thing_id, &leaf_classes, &direct_parents);

    // Asserted direct ancestors (for POnto)
    let mut asserted_direct_ancestors: usize = 0;
    for ac in ontology.iter() {
        if let Component::SubClassOf(SubClassOf { sub, sup }) = &ac.component {
            if let (
                ClassExpression::Class(Class(sub_iri)),
                ClassExpression::Class(Class(sup_iri)),
            ) = (sub, sup)
            {
                let s: &str = sub_iri.as_ref();
                let p: &str = sup_iri.as_ref();
                if declared_classes.contains(s) && (declared_classes.contains(p) || p == OWL_THING)
                {
                    asserted_direct_ancestors += 1;
                }
            }
        }
    }

    // Step 4: Compute raw metrics
    let nc = num_classes as f64;
    let nl = num_leaf_classes as f64;
    let np = num_paths as f64;

    let an = if nc > 0.0 {
        sum_annotations as f64 / nc
    } else {
        0.0
    };
    let ar = if nc > 0.0 {
        sum_attributes as f64 / nc
    } else {
        0.0
    };
    let cbo = if nc > 0.0 {
        sum_direct_ancestors as f64 / nc
    } else {
        0.0
    };
    let cb2 = cbo; // CBOnto2: same formula when sumOfThingRelationships is handled same way
    let cr = if nc > 0.0 {
        num_instances as f64 / nc
    } else {
        0.0
    };
    let dit = max_depth as f64;
    let inr = if nc > 0.0 {
        sum_class_relationships as f64 / nc
    } else {
        0.0
    };
    let lcom = if np > 0.0 {
        sum_path_lengths as f64 / np
    } else {
        0.0
    };
    let nac = if nl > 0.0 {
        sum_direct_ancestors_leaves as f64 / nl
    } else {
        0.0
    };
    let noc_denom = nc - nl;
    let noc = if noc_denom > 0.0 {
        sum_class_relationships as f64 / noc_denom
    } else {
        0.0
    };
    let nom = if nc > 0.0 {
        property_usage_count as f64 / nc
    } else {
        0.0
    };
    let p_onto = if nc > 0.0 {
        asserted_direct_ancestors as f64 / nc
    } else {
        0.0
    };
    let pr = {
        let denom = property_usage_count as f64 + sum_class_relationships as f64;
        if denom > 0.0 {
            property_usage_count as f64 / denom
        } else {
            0.0
        }
    };
    let rfc = if nc > 0.0 {
        (property_usage_count as f64 + sum_direct_ancestors as f64) / nc
    } else {
        0.0
    };
    let rr = {
        let denom = sum_class_relationships as f64 + property_usage_count as f64;
        if denom > 0.0 {
            sum_class_relationships as f64 / denom
        } else {
            0.0
        }
    };
    let tm = if nc > 1.0 {
        num_multi_parent as f64 / (nc - 1.0)
    } else {
        0.0
    };
    let tm2 = if num_multi_parent > 0 {
        sum_ancestors_multi_parent as f64 / num_multi_parent as f64
    } else {
        0.0
    };
    let wmc = if nl > 0.0 {
        sum_path_lengths as f64 / nl
    } else {
        0.0
    };
    let wmc2 = if nl > 0.0 { num_paths as f64 / nl } else { 0.0 };

    let raw = MetricValues {
        an,
        ar,
        cbo,
        cb2,
        cr,
        dit,
        inr,
        lcom,
        nac,
        noc,
        nom,
        p: p_onto,
        pr,
        rfc,
        rr,
        tm,
        tm2,
        wmc,
        wmc2,
    };

    // Step 5: Scale metrics
    let s_an = scale_05(an);
    let s_ar = scale_05(ar);
    let s_cbo = scale_03(cbo);
    let s_cb2 = scale_05(safe_invert(cb2));
    let s_cr = scale_05(cr);
    let s_dit = scale_03(dit);
    let s_inr = scale_05(inr);
    let s_lcom = scale_lcom(lcom);
    let s_nac = scale_03(nac);
    let s_noc = scale_04(noc);
    let s_nom = scale_03(nom);
    let s_p = scale_05(safe_invert(p_onto));
    let s_pr = scale_05(pr);
    let s_rfc = scale_04(rfc);
    let s_rr = scale_05(rr);
    let s_tm = scale_tm(tm);
    let s_tm2 = scale_05(safe_invert(tm2));
    let s_wmc = scale_wmc(wmc);
    let s_wmc2 = scale_05(safe_invert(wmc2));

    let scaled = MetricValuesScaled {
        an: s_an,
        ar: s_ar,
        cbo: s_cbo,
        cb2: s_cb2,
        cr: s_cr,
        dit: s_dit,
        inr: s_inr,
        lcom: s_lcom,
        nac: s_nac,
        noc: s_noc,
        nom: s_nom,
        p: s_p,
        pr: s_pr,
        rfc: s_rfc,
        rr: s_rr,
        tm: s_tm,
        tm2: s_tm2,
        wmc: s_wmc,
        wmc2: s_wmc2,
    };

    // Step 6: Compute subcharacteristics and characteristics
    let sm = ScaledMetrics {
        an: s_an,
        ar: s_ar,
        cbo: s_cbo,
        cr: s_cr,
        dit: s_dit,
        inr: s_inr,
        lcom: s_lcom,
        nac: s_nac,
        noc: s_noc,
        nom: s_nom,
        rfc: s_rfc,
        rr: s_rr,
        tm: s_tm,
        wmc: s_wmc,
        consistency: 5.0,
        formal_degree: 5.0,
    };

    let subchars = compute_subcharacteristics(&sm);
    let model = compute_characteristics(&subchars);

    let basic = BasicMetrics {
        num_classes,
        num_leaf_classes,
        num_paths,
        num_instances,
        num_object_properties,
        num_data_properties,
        max_depth,
        sum_of_path_lengths: sum_path_lengths,
        sum_of_annotations: sum_annotations,
        sum_of_attributes: sum_attributes,
        sum_of_class_relationships: sum_class_relationships,
        sum_of_direct_ancestors: sum_direct_ancestors,
        sum_of_direct_ancestors_of_leaves: sum_direct_ancestors_leaves,
        sum_of_thing_relationships: sum_thing_relationships,
        num_multi_parent_classes: num_multi_parent,
        sum_of_ancestors_of_multi_parent_classes: sum_ancestors_multi_parent,
        num_property_usages: property_usage_count,
    };

    QualityReport {
        basic_metrics: basic,
        metrics: raw,
        metrics_scaled: scaled,
        model,
    }
}
