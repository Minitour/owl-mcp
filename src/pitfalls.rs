use std::collections::{HashMap, HashSet};

use horned_owl::model::*;
use horned_owl::ontology::set::SetOntology;

const STANDARD_NAMESPACES: &[&str] = &[
    "http://www.w3.org/2002/07/owl#",
    "http://www.w3.org/2000/01/rdf-schema#",
    "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
    "http://www.w3.org/2001/XMLSchema#",
    "http://www.w3.org/XML/1998/namespace",
];

const RDFS_LABEL: &str = "http://www.w3.org/2000/01/rdf-schema#label";
const RDFS_COMMENT: &str = "http://www.w3.org/2000/01/rdf-schema#comment";

const LICENSE_PROPERTIES: &[&str] = &[
    "http://purl.org/dc/terms/license",
    "http://purl.org/dc/elements/1.1/rights",
    "http://creativecommons.org/ns#license",
    "http://schema.org/license",
    "http://www.w3.org/1999/xhtml/vocab#license",
];

const OWL_FILE_EXTENSIONS: &[&str] = &[".owl", ".rdf", ".ttl", ".n3", ".rdfxml"];

const MISC_TOKENS: &[&str] = &[
    "other",
    "misc",
    "miscellanea",
    "miscellaneous",
    "miscellany",
];

const IS_NAMES: &[&str] = &["is", "isa", "is-a", "is_a"];

#[derive(Debug, serde::Serialize)]
pub struct PitfallReport {
    pub summary: ReportSummary,
    pub pitfalls: Vec<DetectedPitfall>,
}

#[derive(Debug, serde::Serialize)]
pub struct ReportSummary {
    pub num_classes: usize,
    pub num_object_properties: usize,
    pub num_data_properties: usize,
    pub total_pitfall_instances: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DetectedPitfall {
    pub id: String,
    pub title: String,
    pub description: String,
    pub importance: String,
    pub affected_elements: Vec<String>,
    pub num_affected_elements: usize,
}

fn is_standard(iri: &str) -> bool {
    STANDARD_NAMESPACES.iter().any(|ns| iri.starts_with(ns))
}

fn ope_iri(ope: &ObjectPropertyExpression<ArcStr>) -> &str {
    match ope {
        ObjectPropertyExpression::ObjectProperty(op) => op.0.as_ref(),
        ObjectPropertyExpression::InverseObjectProperty(op) => op.0.as_ref(),
    }
}

fn collect_class_iris(ce: &ClassExpression<ArcStr>) -> Vec<String> {
    match ce {
        ClassExpression::Class(c) => vec![c.0.as_ref().to_string()],
        ClassExpression::ObjectIntersectionOf(ces) | ClassExpression::ObjectUnionOf(ces) => {
            ces.iter().flat_map(collect_class_iris).collect()
        }
        ClassExpression::ObjectComplementOf(inner) => collect_class_iris(inner),
        ClassExpression::ObjectSomeValuesFrom { bce, .. }
        | ClassExpression::ObjectAllValuesFrom { bce, .. }
        | ClassExpression::ObjectMinCardinality { bce, .. }
        | ClassExpression::ObjectMaxCardinality { bce, .. }
        | ClassExpression::ObjectExactCardinality { bce, .. } => collect_class_iris(bce),
        _ => vec![],
    }
}

fn local_name(iri: &str) -> &str {
    iri.rsplit_once('#')
        .or_else(|| iri.rsplit_once('/'))
        .map(|(_, name)| name)
        .unwrap_or(iri)
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum NamingStyle {
    UpperCamel,
    LowerCamel,
    SnakeCase,
    KebabCase,
    Unknown,
}

fn detect_naming_style(name: &str) -> NamingStyle {
    if name.is_empty() {
        return NamingStyle::Unknown;
    }
    if name.contains('-') {
        return NamingStyle::KebabCase;
    }
    if name.contains('_') {
        return NamingStyle::SnakeCase;
    }
    let first = name.chars().next().unwrap();
    if first.is_uppercase() {
        NamingStyle::UpperCamel
    } else if name.chars().any(|c| c.is_uppercase()) {
        NamingStyle::LowerCamel
    } else {
        NamingStyle::Unknown
    }
}

fn namespace_of(iri: &str) -> &str {
    if let Some(pos) = iri.rfind('#') {
        &iri[..=pos]
    } else if let Some(pos) = iri.rfind('/') {
        &iri[..=pos]
    } else {
        iri
    }
}

fn normalize_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn tokenize_name(name: &str) -> Vec<String> {
    let replaced = name.replace(['-', '_'], " ");
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in replaced.chars() {
        if ch == ' ' {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else if ch.is_uppercase() && !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
            current.push(ch);
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens.iter().map(|t| t.to_lowercase()).collect()
}

fn annotation_text(av: &AnnotationValue<ArcStr>) -> Option<&str> {
    match av {
        AnnotationValue::Literal(lit) => Some(lit.literal().as_ref()),
        _ => None,
    }
}

fn collect_labels_and_comments(
    ontology: &SetOntology<ArcStr>,
) -> (HashMap<String, Vec<String>>, HashMap<String, Vec<String>>) {
    let mut labels: HashMap<String, Vec<String>> = HashMap::new();
    let mut comments: HashMap<String, Vec<String>> = HashMap::new();
    for ac in ontology.iter() {
        if let Component::AnnotationAssertion(aa) = &ac.component {
            if let AnnotationSubject::IRI(subject_iri) = &aa.subject {
                let subj = subject_iri.as_ref().to_string();
                let prop: &str = aa.ann.ap.0.as_ref();
                if let Some(text) = annotation_text(&aa.ann.av) {
                    if prop == RDFS_LABEL {
                        labels.entry(subj).or_default().push(text.to_string());
                    } else if prop == RDFS_COMMENT {
                        comments.entry(subj).or_default().push(text.to_string());
                    }
                }
            }
        }
    }
    (labels, comments)
}

fn collect_obj_prop_domains_ranges(
    ontology: &SetOntology<ArcStr>,
) -> (HashMap<String, Vec<String>>, HashMap<String, Vec<String>>) {
    let mut domains: HashMap<String, Vec<String>> = HashMap::new();
    let mut ranges: HashMap<String, Vec<String>> = HashMap::new();
    for ac in ontology.iter() {
        match &ac.component {
            Component::ObjectPropertyDomain(ax) => {
                let prop = ope_iri(&ax.ope).to_string();
                domains
                    .entry(prop)
                    .or_default()
                    .extend(collect_class_iris(&ax.ce));
            }
            Component::ObjectPropertyRange(ax) => {
                let prop = ope_iri(&ax.ope).to_string();
                ranges
                    .entry(prop)
                    .or_default()
                    .extend(collect_class_iris(&ax.ce));
            }
            _ => {}
        }
    }
    (domains, ranges)
}

// ── Main entry point ─────────────────────────────────────────────────────────

pub fn scan(ontology: &SetOntology<ArcStr>, filter: Option<&HashSet<String>>) -> PitfallReport {
    let should_check = |id: &str| -> bool { filter.is_none_or(|f| f.contains(id)) };

    let mut pitfalls = Vec::new();

    if should_check("P02") {
        pitfalls.extend(check_p02(ontology));
    }
    if should_check("P03") {
        pitfalls.extend(check_p03(ontology));
    }
    if should_check("P04") {
        pitfalls.extend(check_p04(ontology));
    }
    if should_check("P05") {
        pitfalls.extend(check_p05(ontology));
    }
    if should_check("P06") {
        pitfalls.extend(check_p06(ontology));
    }
    if should_check("P07") {
        pitfalls.extend(check_p07(ontology));
    }
    if should_check("P08") {
        pitfalls.extend(check_p08(ontology));
    }
    if should_check("P10") {
        pitfalls.extend(check_p10(ontology));
    }
    if should_check("P11") {
        pitfalls.extend(check_p11(ontology));
    }
    if should_check("P12") {
        pitfalls.extend(check_p12(ontology));
    }
    if should_check("P13") {
        pitfalls.extend(check_p13(ontology));
    }
    if should_check("P19") {
        pitfalls.extend(check_p19(ontology));
    }
    if should_check("P20") {
        pitfalls.extend(check_p20(ontology));
    }
    if should_check("P21") {
        pitfalls.extend(check_p21(ontology));
    }
    if should_check("P22") {
        pitfalls.extend(check_p22(ontology));
    }
    if should_check("P24") {
        pitfalls.extend(check_p24(ontology));
    }
    if should_check("P25") {
        pitfalls.extend(check_p25(ontology));
    }
    if should_check("P26") {
        pitfalls.extend(check_p26(ontology));
    }
    if should_check("P27") {
        pitfalls.extend(check_p27(ontology));
    }
    if should_check("P28") {
        pitfalls.extend(check_p28(ontology));
    }
    if should_check("P29") {
        pitfalls.extend(check_p29(ontology));
    }
    if should_check("P30") {
        pitfalls.extend(check_p30(ontology));
    }
    if should_check("P31") {
        pitfalls.extend(check_p31(ontology));
    }
    if should_check("P32") {
        pitfalls.extend(check_p32(ontology));
    }
    if should_check("P33") {
        pitfalls.extend(check_p33(ontology));
    }
    if should_check("P34") {
        pitfalls.extend(check_p34(ontology));
    }
    if should_check("P35") {
        pitfalls.extend(check_p35(ontology));
    }
    if should_check("P36") {
        pitfalls.extend(check_p36(ontology));
    }
    if should_check("P38") {
        pitfalls.extend(check_p38(ontology));
    }
    if should_check("P39") {
        pitfalls.extend(check_p39(ontology));
    }
    if should_check("P41") {
        pitfalls.extend(check_p41(ontology));
    }

    let mut num_classes = 0usize;
    let mut num_object_properties = 0usize;
    let mut num_data_properties = 0usize;
    for ac in ontology.iter() {
        match &ac.component {
            Component::DeclareClass(dc) if !is_standard(dc.0 .0.as_ref()) => num_classes += 1,
            Component::DeclareObjectProperty(dop) if !is_standard(dop.0 .0.as_ref()) => {
                num_object_properties += 1
            }
            Component::DeclareDataProperty(ddp) if !is_standard(ddp.0 .0.as_ref()) => {
                num_data_properties += 1
            }
            _ => {}
        }
    }

    let total_instances: usize = pitfalls
        .iter()
        .map(|p| p.num_affected_elements.max(1))
        .sum();

    PitfallReport {
        summary: ReportSummary {
            num_classes,
            num_object_properties,
            num_data_properties,
            total_pitfall_instances: total_instances,
        },
        pitfalls,
    }
}

// ── P02: Creating synonyms as classes ────────────────────────────────────────

fn check_p02(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut affected: Vec<String> = Vec::new();

    for ac in ontology.iter() {
        if let Component::EquivalentClasses(ax) = &ac.component {
            let class_iris: Vec<String> =
                ax.0.iter()
                    .flat_map(collect_class_iris)
                    .filter(|iri| !is_standard(iri))
                    .collect();
            for i in 0..class_iris.len() {
                for j in (i + 1)..class_iris.len() {
                    let ns_i = namespace_of(&class_iris[i]);
                    let ns_j = namespace_of(&class_iris[j]);
                    if ns_i == ns_j {
                        let name_i = local_name(&class_iris[i]).to_lowercase();
                        let name_j = local_name(&class_iris[j]).to_lowercase();
                        if name_i != name_j {
                            if !affected.contains(&class_iris[i]) {
                                affected.push(class_iris[i].clone());
                            }
                            if !affected.contains(&class_iris[j]) {
                                affected.push(class_iris[j].clone());
                            }
                        }
                    }
                }
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P02".to_string(),
        title: "Creating synonyms as classes".to_string(),
        description: "Classes in the same namespace are declared equivalent but have different \
                      local names. This suggests they may be synonyms that should be merged \
                      into a single class with alternative labels."
            .to_string(),
        importance: "Minor".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P03: Creating the relationship "is" ──────────────────────────────────────

fn check_p03(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut affected: Vec<String> = Vec::new();

    for ac in ontology.iter() {
        if let Component::DeclareObjectProperty(dop) = &ac.component {
            let iri = dop.0 .0.as_ref();
            if !is_standard(iri) {
                let name = local_name(iri).to_lowercase();
                if IS_NAMES.contains(&name.as_str()) {
                    affected.push(iri.to_string());
                }
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P03".to_string(),
        title: "Creating the relationship \"is\" instead of using OWL primitives".to_string(),
        description: "An object property named 'is', 'isa', 'is-a', or 'is_a' was found. \
                      This relationship is typically better expressed using rdfs:subClassOf, \
                      rdf:type, or owl:sameAs."
            .to_string(),
        importance: "Critical".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P04: Creating unconnected ontology elements ──────────────────────────────

fn check_p04(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut declared_classes: HashSet<String> = HashSet::new();
    let mut declared_obj_props: HashSet<String> = HashSet::new();
    let mut declared_data_props: HashSet<String> = HashSet::new();
    let mut referenced_classes: HashSet<String> = HashSet::new();
    let mut referenced_props: HashSet<String> = HashSet::new();

    for ac in ontology.iter() {
        match &ac.component {
            Component::DeclareClass(dc) => {
                let iri = dc.0 .0.as_ref();
                if !is_standard(iri) {
                    declared_classes.insert(iri.to_string());
                }
            }
            Component::DeclareObjectProperty(dop) => {
                let iri = dop.0 .0.as_ref();
                if !is_standard(iri) {
                    declared_obj_props.insert(iri.to_string());
                }
            }
            Component::DeclareDataProperty(ddp) => {
                let iri = ddp.0 .0.as_ref();
                if !is_standard(iri) {
                    declared_data_props.insert(iri.to_string());
                }
            }
            Component::SubClassOf(ax) => {
                referenced_classes.extend(collect_class_iris(&ax.sub));
                referenced_classes.extend(collect_class_iris(&ax.sup));
            }
            Component::EquivalentClasses(ax) => {
                for ce in &ax.0 {
                    referenced_classes.extend(collect_class_iris(ce));
                }
            }
            Component::DisjointClasses(ax) => {
                for ce in &ax.0 {
                    referenced_classes.extend(collect_class_iris(ce));
                }
            }
            Component::ObjectPropertyDomain(ax) => {
                referenced_classes.extend(collect_class_iris(&ax.ce));
                referenced_props.insert(ope_iri(&ax.ope).to_string());
            }
            Component::ObjectPropertyRange(ax) => {
                referenced_classes.extend(collect_class_iris(&ax.ce));
                referenced_props.insert(ope_iri(&ax.ope).to_string());
            }
            Component::DataPropertyDomain(ax) => {
                referenced_classes.extend(collect_class_iris(&ax.ce));
                referenced_props.insert(ax.dp.0.as_ref().to_string());
            }
            Component::DataPropertyRange(ax) => {
                referenced_props.insert(ax.dp.0.as_ref().to_string());
            }
            Component::SubObjectPropertyOf(ax) => {
                referenced_props.insert(ope_iri(&ax.sup).to_string());
                if let SubObjectPropertyExpression::ObjectPropertyExpression(sub) = &ax.sub {
                    referenced_props.insert(ope_iri(sub).to_string());
                }
            }
            Component::SubDataPropertyOf(ax) => {
                referenced_props.insert(ax.sub.0.as_ref().to_string());
                referenced_props.insert(ax.sup.0.as_ref().to_string());
            }
            Component::InverseObjectProperties(ax) => {
                referenced_props.insert(ax.0 .0.as_ref().to_string());
                referenced_props.insert(ax.1 .0.as_ref().to_string());
            }
            Component::EquivalentObjectProperties(ax) => {
                for ope in &ax.0 {
                    referenced_props.insert(ope_iri(ope).to_string());
                }
            }
            Component::DisjointObjectProperties(ax) => {
                for ope in &ax.0 {
                    referenced_props.insert(ope_iri(ope).to_string());
                }
            }
            Component::EquivalentDataProperties(ax) => {
                for dp in &ax.0 {
                    referenced_props.insert(dp.0.as_ref().to_string());
                }
            }
            Component::DisjointDataProperties(ax) => {
                for dp in &ax.0 {
                    referenced_props.insert(dp.0.as_ref().to_string());
                }
            }
            Component::ClassAssertion(ax) => {
                referenced_classes.extend(collect_class_iris(&ax.ce));
            }
            Component::ObjectPropertyAssertion(ax) => {
                referenced_props.insert(ope_iri(&ax.ope).to_string());
            }
            Component::DataPropertyAssertion(ax) => {
                referenced_props.insert(ax.dp.0.as_ref().to_string());
            }
            _ => {}
        }
    }

    let mut unconnected: Vec<String> = declared_classes
        .iter()
        .filter(|iri| !referenced_classes.contains(iri.as_str()))
        .cloned()
        .collect();

    unconnected.extend(
        declared_obj_props
            .iter()
            .chain(declared_data_props.iter())
            .filter(|iri| !referenced_props.contains(iri.as_str()))
            .cloned(),
    );

    if unconnected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P04".to_string(),
        title: "Creating unconnected ontology elements".to_string(),
        description: "Ontology elements (classes, object properties and datatype properties) \
                      exist in isolation, with no relation to the rest of the ontology."
            .to_string(),
        importance: "Minor".to_string(),
        num_affected_elements: unconnected.len(),
        affected_elements: unconnected,
    }]
}

// ── P05: Defining wrong inverse relationships ───────────────────────────────

fn check_p05(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let (domains, ranges) = collect_obj_prop_domains_ranges(ontology);
    let mut affected: Vec<String> = Vec::new();

    for ac in ontology.iter() {
        if let Component::InverseObjectProperties(ax) = &ac.component {
            let p1: &str = ax.0 .0.as_ref();
            let p2: &str = ax.1 .0.as_ref();
            if p1 == p2 || is_standard(p1) || is_standard(p2) {
                continue;
            }
            let d1 = domains.get(p1);
            let r1 = ranges.get(p1);
            let d2 = domains.get(p2);
            let r2 = ranges.get(p2);
            let has_domain_range_info =
                d1.is_some() || r1.is_some() || d2.is_some() || r2.is_some();
            if !has_domain_range_info {
                continue;
            }
            let domain_range_mismatch = d1 != r2 || r1 != d2;
            if domain_range_mismatch {
                if !affected.contains(&p1.to_string()) {
                    affected.push(p1.to_string());
                }
                if !affected.contains(&p2.to_string()) {
                    affected.push(p2.to_string());
                }
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P05".to_string(),
        title: "Defining wrong inverse relationships".to_string(),
        description: "Inverse properties have mismatched domains and ranges. For a correct \
                      inverse, domain(P1) should equal range(P2) and range(P1) should equal \
                      domain(P2)."
            .to_string(),
        importance: "Critical".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P06: Including cycles in a class hierarchy ──────────────────────────────

fn check_p06(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();

    for ac in ontology.iter() {
        if let Component::SubClassOf(ax) = &ac.component {
            if let ClassExpression::Class(sub) = &ax.sub {
                let sub_iri = sub.0.as_ref();
                if is_standard(sub_iri) {
                    continue;
                }
                let super_iris = collect_class_iris(&ax.sup);
                for sup_iri in super_iris {
                    if !is_standard(&sup_iri) && sup_iri != sub_iri {
                        graph.entry(sub_iri.to_string()).or_default().push(sup_iri);
                    }
                }
            }
        }
    }

    let mut in_cycle: HashSet<String> = HashSet::new();
    let all_nodes: Vec<String> = graph.keys().cloned().collect();

    for start in &all_nodes {
        if in_cycle.contains(start) {
            continue;
        }
        let mut visited: HashSet<String> = HashSet::new();
        let mut stack: Vec<String> = vec![start.clone()];
        while let Some(node) = stack.pop() {
            if node == *start && visited.contains(start) {
                in_cycle.extend(visited);
                break;
            }
            if !visited.insert(node.clone()) {
                continue;
            }
            if let Some(supers) = graph.get(&node) {
                for s in supers {
                    stack.push(s.clone());
                }
            }
        }
    }

    if in_cycle.is_empty() {
        return vec![];
    }

    let affected: Vec<String> = in_cycle.into_iter().collect();
    vec![DetectedPitfall {
        id: "P06".to_string(),
        title: "Including cycles in a class hierarchy".to_string(),
        description: "A cycle was detected in the class hierarchy (e.g. A subClassOf B and \
                      B subClassOf A). This may cause reasoning issues and usually indicates \
                      a modeling error."
            .to_string(),
        importance: "Critical".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P07: Merging different concepts in the same class ────────────────────────

fn check_p07(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut affected: Vec<String> = Vec::new();

    for ac in ontology.iter() {
        if let Component::DeclareClass(dc) = &ac.component {
            let iri = dc.0 .0.as_ref();
            if is_standard(iri) {
                continue;
            }
            let tokens = tokenize_name(local_name(iri));
            if tokens.iter().any(|t| t == "and" || t == "or") {
                affected.push(iri.to_string());
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P07".to_string(),
        title: "Merging different concepts in the same class".to_string(),
        description: "A class name contains 'and' or 'or', suggesting it may represent \
                      multiple concepts that should be modeled as separate classes."
            .to_string(),
        importance: "Minor".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P08: Missing annotations ─────────────────────────────────────────────────

fn check_p08(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut entities: HashSet<String> = HashSet::new();
    let mut has_label: HashSet<String> = HashSet::new();
    let mut has_comment: HashSet<String> = HashSet::new();

    for ac in ontology.iter() {
        match &ac.component {
            Component::DeclareClass(dc) => {
                let iri = dc.0 .0.as_ref();
                if !is_standard(iri) {
                    entities.insert(iri.to_string());
                }
            }
            Component::DeclareObjectProperty(dop) => {
                let iri = dop.0 .0.as_ref();
                if !is_standard(iri) {
                    entities.insert(iri.to_string());
                }
            }
            Component::DeclareDataProperty(ddp) => {
                let iri = ddp.0 .0.as_ref();
                if !is_standard(iri) {
                    entities.insert(iri.to_string());
                }
            }
            Component::AnnotationAssertion(aa) => {
                if let AnnotationSubject::IRI(subject_iri) = &aa.subject {
                    let subj: &str = subject_iri.as_ref();
                    let prop: &str = aa.ann.ap.0.as_ref();
                    if prop == RDFS_LABEL {
                        has_label.insert(subj.to_string());
                    } else if prop == RDFS_COMMENT {
                        has_comment.insert(subj.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    let mut results = Vec::new();

    let missing_both: Vec<String> = entities
        .iter()
        .filter(|e| !has_label.contains(e.as_str()) && !has_comment.contains(e.as_str()))
        .cloned()
        .collect();

    if !missing_both.is_empty() {
        results.push(DetectedPitfall {
            id: "P08-A".to_string(),
            title: "Missing annotations - Label & Comment".to_string(),
            description: "Ontology elements lack both rdfs:label and rdfs:comment annotations. \
                          Human-readable annotations help understanding and reusing ontologies."
                .to_string(),
            importance: "Minor".to_string(),
            num_affected_elements: missing_both.len(),
            affected_elements: missing_both,
        });
    }

    let missing_label: Vec<String> = entities
        .iter()
        .filter(|e| !has_label.contains(e.as_str()) && has_comment.contains(e.as_str()))
        .cloned()
        .collect();

    if !missing_label.is_empty() {
        results.push(DetectedPitfall {
            id: "P08-L".to_string(),
            title: "Missing annotations - Label".to_string(),
            description: "Ontology elements have rdfs:comment but lack rdfs:label annotations."
                .to_string(),
            importance: "Minor".to_string(),
            num_affected_elements: missing_label.len(),
            affected_elements: missing_label,
        });
    }

    let missing_comment: Vec<String> = entities
        .iter()
        .filter(|e| has_label.contains(e.as_str()) && !has_comment.contains(e.as_str()))
        .cloned()
        .collect();

    if !missing_comment.is_empty() {
        results.push(DetectedPitfall {
            id: "P08-C".to_string(),
            title: "Missing annotations - Comment".to_string(),
            description: "Ontology elements have rdfs:label but lack rdfs:comment annotations."
                .to_string(),
            importance: "Minor".to_string(),
            num_affected_elements: missing_comment.len(),
            affected_elements: missing_comment,
        });
    }

    results
}

// ── P10: Missing disjointness ────────────────────────────────────────────────

fn check_p10(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut num_classes = 0usize;
    let mut has_disjoint = false;

    for ac in ontology.iter() {
        match &ac.component {
            Component::DeclareClass(dc) if !is_standard(dc.0 .0.as_ref()) => {
                num_classes += 1;
            }
            Component::DisjointClasses(_) => {
                has_disjoint = true;
            }
            _ => {}
        }
    }

    if num_classes >= 2 && !has_disjoint {
        vec![DetectedPitfall {
            id: "P10".to_string(),
            title: "Missing disjointness".to_string(),
            description:
                "The ontology defines multiple classes but includes no DisjointClasses axioms. \
                 Specifying class disjointness helps reasoners detect inconsistencies."
                    .to_string(),
            importance: "Important".to_string(),
            affected_elements: vec![],
            num_affected_elements: 0,
        }]
    } else {
        vec![]
    }
}

// ── P11: Missing domain or range in properties ───────────────────────────────

fn check_p11(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut obj_props: HashSet<String> = HashSet::new();
    let mut data_props: HashSet<String> = HashSet::new();
    let mut has_obj_domain: HashSet<String> = HashSet::new();
    let mut has_obj_range: HashSet<String> = HashSet::new();
    let mut has_data_domain: HashSet<String> = HashSet::new();
    let mut has_data_range: HashSet<String> = HashSet::new();

    for ac in ontology.iter() {
        match &ac.component {
            Component::DeclareObjectProperty(dop) => {
                let iri = dop.0 .0.as_ref();
                if !is_standard(iri) {
                    obj_props.insert(iri.to_string());
                }
            }
            Component::DeclareDataProperty(ddp) => {
                let iri = ddp.0 .0.as_ref();
                if !is_standard(iri) {
                    data_props.insert(iri.to_string());
                }
            }
            Component::ObjectPropertyDomain(ax) => {
                has_obj_domain.insert(ope_iri(&ax.ope).to_string());
            }
            Component::ObjectPropertyRange(ax) => {
                has_obj_range.insert(ope_iri(&ax.ope).to_string());
            }
            Component::DataPropertyDomain(ax) => {
                has_data_domain.insert(ax.dp.0.as_ref().to_string());
            }
            Component::DataPropertyRange(ax) => {
                has_data_range.insert(ax.dp.0.as_ref().to_string());
            }
            _ => {}
        }
    }

    let mut missing: Vec<String> = Vec::new();

    for prop in &obj_props {
        if !has_obj_domain.contains(prop) || !has_obj_range.contains(prop) {
            missing.push(prop.clone());
        }
    }
    for prop in &data_props {
        if !has_data_domain.contains(prop) || !has_data_range.contains(prop) {
            missing.push(prop.clone());
        }
    }

    if missing.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P11".to_string(),
        title: "Missing domain or range in properties".to_string(),
        description: "Object and/or datatype properties are missing domain or range \
                      (or both). Defining domains and ranges improves reasoning and documentation."
            .to_string(),
        importance: "Important".to_string(),
        num_affected_elements: missing.len(),
        affected_elements: missing,
    }]
}

// ── P12: Equivalent properties not explicitly declared ───────────────────────

fn check_p12(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut obj_props: Vec<String> = Vec::new();
    let mut data_props: Vec<String> = Vec::new();
    let mut equiv_obj: HashSet<(String, String)> = HashSet::new();
    let mut equiv_data: HashSet<(String, String)> = HashSet::new();
    let mut sub_obj: HashSet<(String, String)> = HashSet::new();
    let mut sub_data: HashSet<(String, String)> = HashSet::new();

    for ac in ontology.iter() {
        match &ac.component {
            Component::DeclareObjectProperty(dop) => {
                let iri = dop.0 .0.as_ref();
                if !is_standard(iri) {
                    obj_props.push(iri.to_string());
                }
            }
            Component::DeclareDataProperty(ddp) => {
                let iri = ddp.0 .0.as_ref();
                if !is_standard(iri) {
                    data_props.push(iri.to_string());
                }
            }
            Component::EquivalentObjectProperties(ax) => {
                for i in 0..ax.0.len() {
                    for j in (i + 1)..ax.0.len() {
                        let a = ope_iri(&ax.0[i]).to_string();
                        let b = ope_iri(&ax.0[j]).to_string();
                        equiv_obj.insert((a.clone(), b.clone()));
                        equiv_obj.insert((b, a));
                    }
                }
            }
            Component::EquivalentDataProperties(ax) => {
                for i in 0..ax.0.len() {
                    for j in (i + 1)..ax.0.len() {
                        let a = ax.0[i].0.as_ref().to_string();
                        let b = ax.0[j].0.as_ref().to_string();
                        equiv_data.insert((a.clone(), b.clone()));
                        equiv_data.insert((b, a));
                    }
                }
            }
            Component::SubObjectPropertyOf(ax) => {
                if let SubObjectPropertyExpression::ObjectPropertyExpression(sub) = &ax.sub {
                    sub_obj.insert((ope_iri(sub).to_string(), ope_iri(&ax.sup).to_string()));
                }
            }
            Component::SubDataPropertyOf(ax) => {
                sub_data.insert((ax.sub.0.as_ref().to_string(), ax.sup.0.as_ref().to_string()));
            }
            _ => {}
        }
    }

    let mut affected: Vec<String> = Vec::new();

    for i in 0..obj_props.len() {
        for j in (i + 1)..obj_props.len() {
            let a = &obj_props[i];
            let b = &obj_props[j];
            if equiv_obj.contains(&(a.clone(), b.clone())) {
                continue;
            }
            if sub_obj.contains(&(a.clone(), b.clone()))
                || sub_obj.contains(&(b.clone(), a.clone()))
            {
                continue;
            }
            let na = normalize_name(local_name(a));
            let nb = normalize_name(local_name(b));
            if !na.is_empty() && na == nb {
                if !affected.contains(a) {
                    affected.push(a.clone());
                }
                if !affected.contains(b) {
                    affected.push(b.clone());
                }
            }
        }
    }

    for i in 0..data_props.len() {
        for j in (i + 1)..data_props.len() {
            let a = &data_props[i];
            let b = &data_props[j];
            if equiv_data.contains(&(a.clone(), b.clone())) {
                continue;
            }
            if sub_data.contains(&(a.clone(), b.clone()))
                || sub_data.contains(&(b.clone(), a.clone()))
            {
                continue;
            }
            let na = normalize_name(local_name(a));
            let nb = normalize_name(local_name(b));
            if !na.is_empty() && na == nb {
                if !affected.contains(a) {
                    affected.push(a.clone());
                }
                if !affected.contains(b) {
                    affected.push(b.clone());
                }
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P12".to_string(),
        title: "Equivalent properties not explicitly declared".to_string(),
        description: "Properties with identical normalized local names exist but are not \
                      declared equivalent. Consider adding owl:equivalentProperty axioms \
                      or renaming to avoid confusion."
            .to_string(),
        importance: "Important".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P13: Missing inverse relationships ───────────────────────────────────────

fn check_p13(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut obj_props: HashSet<String> = HashSet::new();
    let mut has_inverse: HashSet<String> = HashSet::new();
    let mut symmetric: HashSet<String> = HashSet::new();
    let (domains, ranges) = collect_obj_prop_domains_ranges(ontology);

    for ac in ontology.iter() {
        match &ac.component {
            Component::DeclareObjectProperty(dop) => {
                let iri = dop.0 .0.as_ref();
                if !is_standard(iri) {
                    obj_props.insert(iri.to_string());
                }
            }
            Component::InverseObjectProperties(ax) => {
                has_inverse.insert(ax.0 .0.as_ref().to_string());
                has_inverse.insert(ax.1 .0.as_ref().to_string());
            }
            Component::SymmetricObjectProperty(ax) => {
                symmetric.insert(ope_iri(&ax.0).to_string());
            }
            _ => {}
        }
    }

    let missing: Vec<String> = obj_props
        .iter()
        .filter(|p| !has_inverse.contains(p.as_str()) && !symmetric.contains(p.as_str()))
        .cloned()
        .collect();

    if missing.is_empty() {
        return vec![];
    }

    let mut suggested: Vec<String> = Vec::new();
    let mut maybe_symmetric: Vec<String> = Vec::new();
    let mut no_suggestion: Vec<String> = Vec::new();

    for prop in &missing {
        let d = domains.get(prop);
        let r = ranges.get(prop);
        match (d, r) {
            (Some(dom), Some(rng)) if dom == rng => {
                maybe_symmetric.push(prop.clone());
            }
            (Some(dom), Some(rng)) => {
                let has_candidate = missing.iter().any(|other| {
                    other != prop
                        && domains.get(other) == Some(rng)
                        && ranges.get(other) == Some(dom)
                });
                if has_candidate {
                    suggested.push(prop.clone());
                } else {
                    no_suggestion.push(prop.clone());
                }
            }
            _ => {
                no_suggestion.push(prop.clone());
            }
        }
    }

    let mut results = Vec::new();

    if !suggested.is_empty() {
        results.push(DetectedPitfall {
            id: "P13-Y".to_string(),
            title: "Missing inverse relationships - inverse candidate found".to_string(),
            description: "Object properties without an inverse declaration where another \
                          property exists with matching swapped domain/range."
                .to_string(),
            importance: "Important".to_string(),
            num_affected_elements: suggested.len(),
            affected_elements: suggested,
        });
    }

    if !maybe_symmetric.is_empty() {
        results.push(DetectedPitfall {
            id: "P13-S".to_string(),
            title: "Missing inverse relationships - possibly symmetric".to_string(),
            description: "Object properties with equal domain and range that lack an \
                          inverse. These may need to be declared symmetric instead."
                .to_string(),
            importance: "Important".to_string(),
            num_affected_elements: maybe_symmetric.len(),
            affected_elements: maybe_symmetric,
        });
    }

    if !no_suggestion.is_empty() {
        results.push(DetectedPitfall {
            id: "P13-N".to_string(),
            title: "Missing inverse relationships - no suggestion".to_string(),
            description: "Object properties without an inverse declaration and no obvious \
                          inverse candidate was found. Consider whether an inverse applies."
                .to_string(),
            importance: "Important".to_string(),
            num_affected_elements: no_suggestion.len(),
            affected_elements: no_suggestion,
        });
    }

    results
}

// ── P19: Defining multiple domains or ranges in properties ───────────────────

fn check_p19(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut obj_domain_count: HashMap<String, usize> = HashMap::new();
    let mut obj_range_count: HashMap<String, usize> = HashMap::new();
    let mut data_domain_count: HashMap<String, usize> = HashMap::new();
    let mut data_range_count: HashMap<String, usize> = HashMap::new();

    for ac in ontology.iter() {
        match &ac.component {
            Component::ObjectPropertyDomain(ax) => {
                let iri = ope_iri(&ax.ope).to_string();
                *obj_domain_count.entry(iri).or_insert(0) += 1;
            }
            Component::ObjectPropertyRange(ax) => {
                let iri = ope_iri(&ax.ope).to_string();
                *obj_range_count.entry(iri).or_insert(0) += 1;
            }
            Component::DataPropertyDomain(ax) => {
                let iri = ax.dp.0.as_ref().to_string();
                *data_domain_count.entry(iri).or_insert(0) += 1;
            }
            Component::DataPropertyRange(ax) => {
                let iri = ax.dp.0.as_ref().to_string();
                *data_range_count.entry(iri).or_insert(0) += 1;
            }
            _ => {}
        }
    }

    let mut affected: Vec<String> = Vec::new();

    for counts in [
        &obj_domain_count,
        &obj_range_count,
        &data_domain_count,
        &data_range_count,
    ] {
        for (iri, count) in counts {
            if *count > 1 && !is_standard(iri) && !affected.contains(iri) {
                affected.push(iri.clone());
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P19".to_string(),
        title: "Defining multiple domains or ranges in properties".to_string(),
        description:
            "A property has multiple rdfs:domain or rdfs:range statements. In OWL these are \
             interpreted as a conjunction (intersection), which may not be the intended meaning. \
             Use owl:unionOf if a union is desired."
                .to_string(),
        importance: "Critical".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P20: Misusing ontology annotations ───────────────────────────────────────

fn check_p20(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let (labels, comments) = collect_labels_and_comments(ontology);
    let mut affected: Vec<String> = Vec::new();

    let mut entities: HashSet<String> = HashSet::new();
    for ac in ontology.iter() {
        match &ac.component {
            Component::DeclareClass(dc) if !is_standard(dc.0 .0.as_ref()) => {
                entities.insert(dc.0 .0.as_ref().to_string());
            }
            Component::DeclareObjectProperty(dop) if !is_standard(dop.0 .0.as_ref()) => {
                entities.insert(dop.0 .0.as_ref().to_string());
            }
            Component::DeclareDataProperty(ddp) if !is_standard(ddp.0 .0.as_ref()) => {
                entities.insert(ddp.0 .0.as_ref().to_string());
            }
            _ => {}
        }
    }

    for entity in &entities {
        let entity_labels = labels.get(entity);
        let entity_comments = comments.get(entity);

        if let (Some(lbls), Some(cmts)) = (entity_labels, entity_comments) {
            for lbl in lbls {
                for cmt in cmts {
                    let lbl_trimmed = lbl.trim();
                    let cmt_trimmed = cmt.trim();
                    let is_problematic = lbl_trimmed.is_empty()
                        || cmt_trimmed.is_empty()
                        || lbl_trimmed.eq_ignore_ascii_case(cmt_trimmed)
                        || lbl_trimmed.split_whitespace().count()
                            > cmt_trimmed.split_whitespace().count();
                    if is_problematic && !affected.contains(entity) {
                        affected.push(entity.clone());
                    }
                }
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P20".to_string(),
        title: "Misusing ontology annotations".to_string(),
        description: "Annotation content appears swapped or misused: the label is longer \
                      than the comment, or they are identical, or one is empty."
            .to_string(),
        importance: "Minor".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P21: Using a miscellaneous class ─────────────────────────────────────────

fn check_p21(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut affected: Vec<String> = Vec::new();

    for ac in ontology.iter() {
        if let Component::DeclareClass(dc) = &ac.component {
            let iri = dc.0 .0.as_ref();
            if is_standard(iri) {
                continue;
            }
            let tokens = tokenize_name(local_name(iri));
            if tokens.iter().any(|t| MISC_TOKENS.contains(&t.as_str())) {
                affected.push(iri.to_string());
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P21".to_string(),
        title: "Using a miscellaneous class".to_string(),
        description: "A class name contains tokens like 'other' or 'miscellaneous', \
                      suggesting a catch-all class. Such classes weaken the ontology's \
                      conceptual clarity."
            .to_string(),
        importance: "Minor".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P22: Using different naming conventions ──────────────────────────────────

fn check_p22(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut class_styles: HashMap<NamingStyle, Vec<String>> = HashMap::new();
    let mut prop_styles: HashMap<NamingStyle, Vec<String>> = HashMap::new();

    for ac in ontology.iter() {
        let (iri, is_class) = match &ac.component {
            Component::DeclareClass(dc) => (Some(dc.0 .0.as_ref()), true),
            Component::DeclareObjectProperty(dop) => (Some(dop.0 .0.as_ref()), false),
            Component::DeclareDataProperty(ddp) => (Some(ddp.0 .0.as_ref()), false),
            _ => (None, false),
        };

        if let Some(iri) = iri {
            if is_standard(iri) {
                continue;
            }
            let name = local_name(iri);
            let style = detect_naming_style(name);
            if style != NamingStyle::Unknown {
                let map = if is_class {
                    &mut class_styles
                } else {
                    &mut prop_styles
                };
                map.entry(style).or_default().push(iri.to_string());
            }
        }
    }

    let mut affected: Vec<String> = Vec::new();
    if class_styles.len() > 1 {
        affected.extend(class_styles.values().flatten().cloned());
    }
    if prop_styles.len() > 1 {
        affected.extend(prop_styles.values().flatten().cloned());
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P22".to_string(),
        title: "Using different naming conventions".to_string(),
        description: "The ontology uses mixed naming conventions within the same entity type. \
                      Classes should follow a consistent style (e.g. UpperCamelCase) and \
                      properties should follow a consistent style (e.g. lowerCamelCase)."
            .to_string(),
        importance: "Minor".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P24: Using recursive definitions ─────────────────────────────────────────

fn check_p24(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut affected: Vec<String> = Vec::new();

    for ac in ontology.iter() {
        if let Component::SubClassOf(ax) = &ac.component {
            if let ClassExpression::Class(sub_class) = &ax.sub {
                let sub_iri = sub_class.0.as_ref();
                if !is_standard(sub_iri) {
                    let super_iris = collect_class_iris(&ax.sup);
                    if super_iris.contains(&sub_iri.to_string())
                        && !affected.contains(&sub_iri.to_string())
                    {
                        affected.push(sub_iri.to_string());
                    }
                }
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P24".to_string(),
        title: "Using recursive definitions".to_string(),
        description: "A class appears in its own SubClassOf axiom's super expression, \
                      creating a recursive definition that may cause reasoning issues."
            .to_string(),
        importance: "Important".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P25: Defining a relationship as inverse to itself ────────────────────────

fn check_p25(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut affected: Vec<String> = Vec::new();

    for ac in ontology.iter() {
        if let Component::InverseObjectProperties(ax) = &ac.component {
            let p1: &str = ax.0 .0.as_ref();
            let p2: &str = ax.1 .0.as_ref();
            if p1 == p2 && !affected.contains(&p1.to_string()) {
                affected.push(p1.to_string());
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P25".to_string(),
        title: "Defining a relationship as inverse to itself".to_string(),
        description: "A property is declared as its own inverse. This is likely an error; \
                      the property may instead need to be declared symmetric."
            .to_string(),
        importance: "Important".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P26: Defining inverse relationships for a symmetric one ─────────────────

fn check_p26(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut symmetric: HashSet<String> = HashSet::new();
    let mut has_inverse: HashSet<String> = HashSet::new();

    for ac in ontology.iter() {
        match &ac.component {
            Component::SymmetricObjectProperty(ax) => {
                let iri = ope_iri(&ax.0).to_string();
                if !is_standard(&iri) {
                    symmetric.insert(iri);
                }
            }
            Component::InverseObjectProperties(ax) => {
                has_inverse.insert(ax.0 .0.as_ref().to_string());
                has_inverse.insert(ax.1 .0.as_ref().to_string());
            }
            _ => {}
        }
    }

    let affected: Vec<String> = symmetric
        .iter()
        .filter(|p| has_inverse.contains(p.as_str()))
        .cloned()
        .collect();

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P26".to_string(),
        title: "Defining inverse relationships for a symmetric one".to_string(),
        description: "A symmetric property also has an inverse declared. Symmetric properties \
                      are their own inverse by definition, so an explicit inverse is redundant \
                      and potentially confusing."
            .to_string(),
        importance: "Minor".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P27: Defining wrong equivalent properties ───────────────────────────────

fn check_p27(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let (domains, ranges) = collect_obj_prop_domains_ranges(ontology);
    let mut affected: Vec<String> = Vec::new();

    for ac in ontology.iter() {
        if let Component::EquivalentObjectProperties(ax) = &ac.component {
            let props: Vec<String> = ax.0.iter().map(|ope| ope_iri(ope).to_string()).collect();
            for i in 0..props.len() {
                for j in (i + 1)..props.len() {
                    if is_standard(&props[i]) || is_standard(&props[j]) {
                        continue;
                    }
                    let d_i = domains.get(&props[i]);
                    let d_j = domains.get(&props[j]);
                    let r_i = ranges.get(&props[i]);
                    let r_j = ranges.get(&props[j]);
                    let has_info = d_i.is_some() || d_j.is_some() || r_i.is_some() || r_j.is_some();
                    if !has_info {
                        continue;
                    }
                    if d_i != d_j || r_i != r_j {
                        if !affected.contains(&props[i]) {
                            affected.push(props[i].clone());
                        }
                        if !affected.contains(&props[j]) {
                            affected.push(props[j].clone());
                        }
                    }
                }
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P27".to_string(),
        title: "Defining wrong equivalent properties".to_string(),
        description: "Properties declared equivalent have different domains or ranges. \
                      Equivalent properties should share the same domain and range semantics."
            .to_string(),
        importance: "Important".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P28: Defining wrong symmetric relationships ─────────────────────────────

fn check_p28(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let (domains, ranges) = collect_obj_prop_domains_ranges(ontology);
    let mut affected: Vec<String> = Vec::new();

    for ac in ontology.iter() {
        if let Component::SymmetricObjectProperty(ax) = &ac.component {
            let iri = ope_iri(&ax.0).to_string();
            if is_standard(&iri) {
                continue;
            }
            let d = domains.get(&iri);
            let r = ranges.get(&iri);
            if d.is_some() && r.is_some() && d != r {
                affected.push(iri);
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P28".to_string(),
        title: "Defining wrong symmetric relationships".to_string(),
        description: "A symmetric property has different domain and range. For a property \
                      to be symmetric, its domain and range must be the same class."
            .to_string(),
        importance: "Important".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P29: Defining wrong transitive relationships ────────────────────────────

fn check_p29(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let (domains, ranges) = collect_obj_prop_domains_ranges(ontology);
    let mut affected: Vec<String> = Vec::new();

    for ac in ontology.iter() {
        if let Component::TransitiveObjectProperty(ax) = &ac.component {
            let iri = ope_iri(&ax.0).to_string();
            if is_standard(&iri) {
                continue;
            }
            let d = domains.get(&iri);
            let r = ranges.get(&iri);
            if d.is_some() && r.is_some() && d != r {
                affected.push(iri);
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P29".to_string(),
        title: "Defining wrong transitive relationships".to_string(),
        description: "A transitive property has different domain and range. For a property \
                      to be transitive, its domain and range should be the same class."
            .to_string(),
        importance: "Important".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P30: Equivalent classes not explicitly declared ──────────────────────────

fn check_p30(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut classes: Vec<String> = Vec::new();
    let mut equiv_pairs: HashSet<(String, String)> = HashSet::new();

    let (labels, _) = collect_labels_and_comments(ontology);

    for ac in ontology.iter() {
        match &ac.component {
            Component::DeclareClass(dc) => {
                let iri = dc.0 .0.as_ref();
                if !is_standard(iri) {
                    classes.push(iri.to_string());
                }
            }
            Component::EquivalentClasses(ax) => {
                let iris: Vec<String> =
                    ax.0.iter()
                        .flat_map(collect_class_iris)
                        .filter(|i| !is_standard(i))
                        .collect();
                for i in 0..iris.len() {
                    for j in (i + 1)..iris.len() {
                        equiv_pairs.insert((iris[i].clone(), iris[j].clone()));
                        equiv_pairs.insert((iris[j].clone(), iris[i].clone()));
                    }
                }
            }
            _ => {}
        }
    }

    let mut affected: Vec<String> = Vec::new();

    for i in 0..classes.len() {
        for j in (i + 1)..classes.len() {
            if equiv_pairs.contains(&(classes[i].clone(), classes[j].clone())) {
                continue;
            }
            let name_i = normalize_name(local_name(&classes[i]));
            let name_j = normalize_name(local_name(&classes[j]));
            let names_match = !name_i.is_empty() && name_i == name_j;

            let labels_match = if !names_match {
                if let (Some(li), Some(lj)) = (labels.get(&classes[i]), labels.get(&classes[j])) {
                    li.iter().any(|a| {
                        lj.iter()
                            .any(|b| normalize_name(a) == normalize_name(b) && !a.trim().is_empty())
                    })
                } else {
                    false
                }
            } else {
                false
            };

            if names_match || labels_match {
                if !affected.contains(&classes[i]) {
                    affected.push(classes[i].clone());
                }
                if !affected.contains(&classes[j]) {
                    affected.push(classes[j].clone());
                }
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P30".to_string(),
        title: "Equivalent classes not explicitly declared".to_string(),
        description: "Classes with identical normalized names or labels are not declared \
                      equivalent. Consider adding owl:equivalentClass axioms or renaming \
                      to avoid ambiguity."
            .to_string(),
        importance: "Important".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P31: Defining wrong equivalent classes ──────────────────────────────────

fn check_p31(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut affected: Vec<String> = Vec::new();

    for ac in ontology.iter() {
        if let Component::EquivalentClasses(ax) = &ac.component {
            let iris: Vec<String> =
                ax.0.iter()
                    .flat_map(collect_class_iris)
                    .filter(|i| !is_standard(i))
                    .collect();
            for i in 0..iris.len() {
                for j in (i + 1)..iris.len() {
                    let name_i = local_name(&iris[i]).to_lowercase();
                    let name_j = local_name(&iris[j]).to_lowercase();
                    if name_i == name_j {
                        continue;
                    }
                    let is_substring = (!name_i.is_empty()
                        && !name_j.is_empty()
                        && name_i.len() >= 3
                        && name_j.len() >= 3)
                        && (name_i.contains(&name_j) || name_j.contains(&name_i));
                    if is_substring {
                        if !affected.contains(&iris[i]) {
                            affected.push(iris[i].clone());
                        }
                        if !affected.contains(&iris[j]) {
                            affected.push(iris[j].clone());
                        }
                    }
                }
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P31".to_string(),
        title: "Defining wrong equivalent classes".to_string(),
        description: "Classes declared equivalent have names where one is a substring of \
                      the other, suggesting a hierarchical (subclass) rather than equivalence \
                      relationship."
            .to_string(),
        importance: "Important".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P32: Several classes with the same label ────────────────────────────────

fn check_p32(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let (labels, _) = collect_labels_and_comments(ontology);
    let mut equiv_pairs: HashSet<(String, String)> = HashSet::new();
    let mut classes: HashSet<String> = HashSet::new();

    for ac in ontology.iter() {
        match &ac.component {
            Component::DeclareClass(dc) if !is_standard(dc.0 .0.as_ref()) => {
                classes.insert(dc.0 .0.as_ref().to_string());
            }
            Component::EquivalentClasses(ax) => {
                let iris: Vec<String> =
                    ax.0.iter()
                        .flat_map(collect_class_iris)
                        .filter(|i| !is_standard(i))
                        .collect();
                for i in 0..iris.len() {
                    for j in (i + 1)..iris.len() {
                        equiv_pairs.insert((iris[i].clone(), iris[j].clone()));
                        equiv_pairs.insert((iris[j].clone(), iris[i].clone()));
                    }
                }
            }
            _ => {}
        }
    }

    let mut label_to_classes: HashMap<String, Vec<String>> = HashMap::new();
    for cls in &classes {
        if let Some(lbls) = labels.get(cls) {
            for lbl in lbls {
                let normalized = lbl.trim().to_lowercase();
                if !normalized.is_empty() {
                    label_to_classes
                        .entry(normalized)
                        .or_default()
                        .push(cls.clone());
                }
            }
        }
    }

    let mut affected: Vec<String> = Vec::new();
    for group in label_to_classes.values() {
        if group.len() < 2 {
            continue;
        }
        let mut non_equiv_found = false;
        'outer: for i in 0..group.len() {
            for j in (i + 1)..group.len() {
                if !equiv_pairs.contains(&(group[i].clone(), group[j].clone())) {
                    non_equiv_found = true;
                    break 'outer;
                }
            }
        }
        if non_equiv_found {
            for cls in group {
                if !affected.contains(cls) {
                    affected.push(cls.clone());
                }
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P32".to_string(),
        title: "Several classes with the same label".to_string(),
        description: "Multiple non-equivalent classes share the same rdfs:label. This creates \
                      ambiguity for humans and tools consuming the ontology."
            .to_string(),
        importance: "Minor".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P33: Creating a property chain with just one property ───────────────────

fn check_p33(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut affected: Vec<String> = Vec::new();

    for ac in ontology.iter() {
        if let Component::SubObjectPropertyOf(ax) = &ac.component {
            if let SubObjectPropertyExpression::ObjectPropertyChain(chain) = &ax.sub {
                if chain.len() == 1 {
                    let sup_iri = ope_iri(&ax.sup).to_string();
                    if !affected.contains(&sup_iri) {
                        affected.push(sup_iri);
                    }
                }
            }
        }
    }

    if affected.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P33".to_string(),
        title: "Creating a property chain with just one property".to_string(),
        description: "A SubPropertyChainOf axiom has only one property in the chain. \
                      This is equivalent to a simple SubPropertyOf and the chain syntax \
                      is unnecessary."
            .to_string(),
        importance: "Minor".to_string(),
        num_affected_elements: affected.len(),
        affected_elements: affected,
    }]
}

// ── P34: Untyped class ───────────────────────────────────────────────────────

fn check_p34(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut declared_classes: HashSet<String> = HashSet::new();
    let mut referenced_classes: HashSet<String> = HashSet::new();

    for ac in ontology.iter() {
        match &ac.component {
            Component::DeclareClass(dc) => {
                declared_classes.insert(dc.0 .0.as_ref().to_string());
            }
            Component::SubClassOf(ax) => {
                for iri in collect_class_iris(&ax.sub) {
                    referenced_classes.insert(iri);
                }
                for iri in collect_class_iris(&ax.sup) {
                    referenced_classes.insert(iri);
                }
            }
            Component::EquivalentClasses(ax) => {
                for ce in &ax.0 {
                    for iri in collect_class_iris(ce) {
                        referenced_classes.insert(iri);
                    }
                }
            }
            Component::DisjointClasses(ax) => {
                for ce in &ax.0 {
                    for iri in collect_class_iris(ce) {
                        referenced_classes.insert(iri);
                    }
                }
            }
            Component::ObjectPropertyDomain(ax) => {
                for iri in collect_class_iris(&ax.ce) {
                    referenced_classes.insert(iri);
                }
            }
            Component::ObjectPropertyRange(ax) => {
                for iri in collect_class_iris(&ax.ce) {
                    referenced_classes.insert(iri);
                }
            }
            Component::DataPropertyDomain(ax) => {
                for iri in collect_class_iris(&ax.ce) {
                    referenced_classes.insert(iri);
                }
            }
            Component::ClassAssertion(ax) => {
                for iri in collect_class_iris(&ax.ce) {
                    referenced_classes.insert(iri);
                }
            }
            _ => {}
        }
    }

    let untyped: Vec<String> = referenced_classes
        .iter()
        .filter(|iri| !declared_classes.contains(iri.as_str()) && !is_standard(iri))
        .cloned()
        .collect();

    if untyped.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P34".to_string(),
        title: "Untyped class".to_string(),
        description:
            "Classes are used in axioms but lack an explicit Declaration(Class(...)) axiom. \
                      While OWL does not strictly require declarations, they improve clarity \
                      and help tools validate the ontology."
                .to_string(),
        importance: "Minor".to_string(),
        num_affected_elements: untyped.len(),
        affected_elements: untyped,
    }]
}

// ── P35: Untyped property ────────────────────────────────────────────────────

fn check_p35(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut declared_obj_props: HashSet<String> = HashSet::new();
    let mut declared_data_props: HashSet<String> = HashSet::new();
    let mut referenced_props: HashSet<String> = HashSet::new();

    for ac in ontology.iter() {
        match &ac.component {
            Component::DeclareObjectProperty(dop) => {
                declared_obj_props.insert(dop.0 .0.as_ref().to_string());
            }
            Component::DeclareDataProperty(ddp) => {
                declared_data_props.insert(ddp.0 .0.as_ref().to_string());
            }
            Component::ObjectPropertyDomain(ax) => {
                referenced_props.insert(ope_iri(&ax.ope).to_string());
            }
            Component::ObjectPropertyRange(ax) => {
                referenced_props.insert(ope_iri(&ax.ope).to_string());
            }
            Component::SubObjectPropertyOf(ax) => {
                referenced_props.insert(ope_iri(&ax.sup).to_string());
                if let SubObjectPropertyExpression::ObjectPropertyExpression(sub) = &ax.sub {
                    referenced_props.insert(ope_iri(sub).to_string());
                }
            }
            Component::InverseObjectProperties(ax) => {
                referenced_props.insert(ax.0 .0.as_ref().to_string());
                referenced_props.insert(ax.1 .0.as_ref().to_string());
            }
            Component::DataPropertyDomain(ax) => {
                referenced_props.insert(ax.dp.0.as_ref().to_string());
            }
            Component::DataPropertyRange(ax) => {
                referenced_props.insert(ax.dp.0.as_ref().to_string());
            }
            Component::SubDataPropertyOf(ax) => {
                referenced_props.insert(ax.sub.0.as_ref().to_string());
                referenced_props.insert(ax.sup.0.as_ref().to_string());
            }
            _ => {}
        }
    }

    let declared_all: HashSet<&String> = declared_obj_props
        .iter()
        .chain(declared_data_props.iter())
        .collect();

    let untyped: Vec<String> = referenced_props
        .iter()
        .filter(|iri| !declared_all.contains(iri) && !is_standard(iri))
        .cloned()
        .collect();

    if untyped.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P35".to_string(),
        title: "Untyped property".to_string(),
        description: "Properties are used in axioms but lack an explicit Declaration axiom. \
                      Adding property declarations clarifies whether each property is an \
                      object property, datatype property, or annotation property."
            .to_string(),
        importance: "Important".to_string(),
        num_affected_elements: untyped.len(),
        affected_elements: untyped,
    }]
}

// ── P36: URI contains file extension ─────────────────────────────────────────

fn check_p36(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    for ac in ontology.iter() {
        if let Component::OntologyID(oid) = &ac.component {
            if let Some(iri) = &oid.iri {
                let iri_str: &str = iri.as_ref();
                for ext in OWL_FILE_EXTENSIONS {
                    if iri_str.contains(ext) {
                        return vec![DetectedPitfall {
                            id: "P36".to_string(),
                            title: "URI contains file extension".to_string(),
                            description: format!(
                                "The ontology URI '{}' contains a file extension '{}'. \
                                 Best practices recommend avoiding technology-specific file \
                                 extensions in persistent URIs.",
                                iri_str, ext
                            ),
                            importance: "Minor".to_string(),
                            affected_elements: vec![iri_str.to_string()],
                            num_affected_elements: 1,
                        }];
                    }
                }
            }
        }
    }

    vec![]
}

// ── P38: No OWL ontology declaration ─────────────────────────────────────────

fn check_p38(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let has_ontology_id = ontology.iter().any(|ac| {
        if let Component::OntologyID(oid) = &ac.component {
            oid.iri.is_some()
        } else {
            false
        }
    });

    if !has_ontology_id {
        vec![DetectedPitfall {
            id: "P38".to_string(),
            title: "No OWL ontology declaration".to_string(),
            description: "The ontology lacks an explicit ontology IRI declaration. \
                          Every OWL ontology should declare its IRI to be properly identified, \
                          imported, and versioned."
                .to_string(),
            importance: "Important".to_string(),
            affected_elements: vec![],
            num_affected_elements: 0,
        }]
    } else {
        vec![]
    }
}

// ── P39: Ambiguous namespace ─────────────────────────────────────────────────

fn check_p39(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let has_iri = ontology.iter().any(|ac| {
        if let Component::OntologyID(oid) = &ac.component {
            oid.iri.is_some()
        } else {
            false
        }
    });

    if !has_iri {
        vec![DetectedPitfall {
            id: "P39".to_string(),
            title: "Ambiguous namespace".to_string(),
            description: "The ontology has no explicit IRI declaration, making its namespace \
                          ambiguous. Without a base namespace, entity URIs may depend on the \
                          file location."
                .to_string(),
            importance: "Important".to_string(),
            affected_elements: vec![],
            num_affected_elements: 0,
        }]
    } else {
        vec![]
    }
}

// ── P41: No license declared ─────────────────────────────────────────────────

fn check_p41(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let has_license = ontology.iter().any(|ac| {
        if let Component::OntologyAnnotation(oa) = &ac.component {
            let prop: &str = oa.0.ap.0.as_ref();
            LICENSE_PROPERTIES.contains(&prop)
        } else if let Component::AnnotationAssertion(aa) = &ac.component {
            let prop: &str = aa.ann.ap.0.as_ref();
            LICENSE_PROPERTIES.contains(&prop)
        } else {
            false
        }
    });

    if !has_license {
        vec![DetectedPitfall {
            id: "P41".to_string(),
            title: "No license declared".to_string(),
            description: "The ontology metadata does not include a license statement. \
                          Declaring a license (using dcterms:license or similar) clarifies \
                          how the ontology may be used and redistributed."
                .to_string(),
            importance: "Important".to_string(),
            affected_elements: vec![],
            num_affected_elements: 0,
        }]
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use horned_owl::model::Build;
    use std::io::BufReader;
    use std::io::Cursor;

    fn parse_ofn(content: &str) -> SetOntology<ArcStr> {
        let build = Build::new_arc();
        let cursor = Cursor::new(content.as_bytes());
        let reader = BufReader::new(cursor);
        let (onto, _): (SetOntology<ArcStr>, _) =
            horned_owl::io::ofn::reader::read_with_build(reader, &build).unwrap();
        onto
    }

    #[test]
    fn p08_detects_missing_labels() {
        let onto = parse_ofn(
            "Ontology(\
               Declaration(Class(<http://example.org/Dog>))\
             )",
        );
        let results = check_p08(&onto);
        assert!(!results.is_empty());
        assert!(results.iter().any(|p| p.id == "P08-A"));
    }

    #[test]
    fn p08_no_pitfall_when_annotated() {
        let onto = parse_ofn(
            "Prefix(rdfs:=<http://www.w3.org/2000/01/rdf-schema#>)\
             Ontology(\
               Declaration(Class(<http://example.org/Dog>))\
               AnnotationAssertion(rdfs:label <http://example.org/Dog> \"Dog\")\
               AnnotationAssertion(rdfs:comment <http://example.org/Dog> \"A canine\")\
             )",
        );
        let results = check_p08(&onto);
        let p08a = results.iter().find(|p| p.id == "P08-A");
        assert!(p08a.is_none() || p08a.unwrap().affected_elements.is_empty());
    }

    #[test]
    fn p10_detects_missing_disjointness() {
        let onto = parse_ofn(
            "Ontology(\
               Declaration(Class(<http://example.org/Dog>))\
               Declaration(Class(<http://example.org/Cat>))\
             )",
        );
        let results = check_p10(&onto);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "P10");
    }

    #[test]
    fn p10_no_pitfall_when_disjoint() {
        let onto = parse_ofn(
            "Ontology(\
               Declaration(Class(<http://example.org/Dog>))\
               Declaration(Class(<http://example.org/Cat>))\
               DisjointClasses(<http://example.org/Dog> <http://example.org/Cat>)\
             )",
        );
        let results = check_p10(&onto);
        assert!(results.is_empty());
    }

    #[test]
    fn p11_detects_missing_domain_range() {
        let onto = parse_ofn(
            "Ontology(\
               Declaration(ObjectProperty(<http://example.org/hasPet>))\
             )",
        );
        let results = check_p11(&onto);
        assert_eq!(results.len(), 1);
        assert!(results[0]
            .affected_elements
            .contains(&"http://example.org/hasPet".to_string()));
    }

    #[test]
    fn p38_detects_missing_ontology_iri() {
        let onto = parse_ofn("Ontology()");
        let results = check_p38(&onto);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "P38");
    }

    #[test]
    fn p38_no_pitfall_with_iri() {
        let onto = parse_ofn("Ontology(<http://example.org/my-ontology>)");
        let results = check_p38(&onto);
        assert!(results.is_empty());
    }

    #[test]
    fn p41_detects_missing_license() {
        let onto = parse_ofn("Ontology(<http://example.org/onto>)");
        let results = check_p41(&onto);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "P41");
    }

    #[test]
    fn p02_detects_synonym_classes() {
        let onto = parse_ofn(
            "Ontology(\
               Declaration(Class(<http://example.org/Dog>))\
               Declaration(Class(<http://example.org/Canine>))\
               EquivalentClasses(<http://example.org/Dog> <http://example.org/Canine>)\
             )",
        );
        let results = check_p02(&onto);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].num_affected_elements, 2);
    }

    #[test]
    fn p03_detects_is_relationship() {
        let onto = parse_ofn(
            "Ontology(\
               Declaration(ObjectProperty(<http://example.org/is>))\
             )",
        );
        let results = check_p03(&onto);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn p03_no_pitfall_for_normal_property() {
        let onto = parse_ofn(
            "Ontology(\
               Declaration(ObjectProperty(<http://example.org/hasPart>))\
             )",
        );
        let results = check_p03(&onto);
        assert!(results.is_empty());
    }

    #[test]
    fn p06_detects_cycle() {
        let onto = parse_ofn(
            "Ontology(\
               Declaration(Class(<http://example.org/A>))\
               Declaration(Class(<http://example.org/B>))\
               SubClassOf(<http://example.org/A> <http://example.org/B>)\
               SubClassOf(<http://example.org/B> <http://example.org/A>)\
             )",
        );
        let results = check_p06(&onto);
        assert_eq!(results.len(), 1);
        assert!(results[0].num_affected_elements >= 2);
    }

    #[test]
    fn p07_detects_merged_concepts() {
        let onto = parse_ofn(
            "Ontology(\
               Declaration(Class(<http://example.org/PersonAndPlace>))\
             )",
        );
        let results = check_p07(&onto);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn p21_detects_misc_class() {
        let onto = parse_ofn(
            "Ontology(\
               Declaration(Class(<http://example.org/OtherThings>))\
             )",
        );
        let results = check_p21(&onto);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn p25_detects_self_inverse() {
        let onto = parse_ofn(
            "Ontology(\
               Declaration(ObjectProperty(<http://example.org/knows>))\
               InverseObjectProperties(<http://example.org/knows> <http://example.org/knows>)\
             )",
        );
        let results = check_p25(&onto);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn p39_detects_ambiguous_namespace() {
        let onto = parse_ofn("Ontology()");
        let results = check_p39(&onto);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "P39");
    }

    #[test]
    fn p39_no_pitfall_with_iri() {
        let onto = parse_ofn("Ontology(<http://example.org/onto>)");
        let results = check_p39(&onto);
        assert!(results.is_empty());
    }

    #[test]
    fn p13_splits_into_variants() {
        let onto = parse_ofn(
            "Ontology(\
               Declaration(ObjectProperty(<http://example.org/hasPet>))\
             )",
        );
        let results = check_p13(&onto);
        assert!(!results.is_empty());
        assert!(results.iter().all(|p| p.id.starts_with("P13-")));
    }

    #[test]
    fn scan_returns_report() {
        let onto = parse_ofn(
            "Ontology(\
               Declaration(Class(<http://example.org/Dog>))\
               Declaration(Class(<http://example.org/Cat>))\
             )",
        );
        let report = scan(&onto, None);
        assert_eq!(report.summary.num_classes, 2);
        assert!(!report.pitfalls.is_empty());
    }

    #[test]
    fn scan_with_filter() {
        let onto = parse_ofn(
            "Ontology(\
               Declaration(Class(<http://example.org/Dog>))\
               Declaration(Class(<http://example.org/Cat>))\
             )",
        );
        let filter: HashSet<String> = ["P38".to_string()].into();
        let report = scan(&onto, Some(&filter));
        assert!(report.pitfalls.iter().all(|p| p.id == "P38"));
    }
}
