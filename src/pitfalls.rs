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

// ── Main entry point ─────────────────────────────────────────────────────────

pub fn scan(ontology: &SetOntology<ArcStr>, filter: Option<&HashSet<String>>) -> PitfallReport {
    let should_check = |id: &str| -> bool { filter.is_none_or(|f| f.contains(id)) };

    let mut pitfalls = Vec::new();

    if should_check("P04") {
        pitfalls.extend(check_p04(ontology));
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
    if should_check("P13") {
        pitfalls.extend(check_p13(ontology));
    }
    if should_check("P19") {
        pitfalls.extend(check_p19(ontology));
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

// ── P04: Creating unconnected ontology elements ──────────────────────────────

fn check_p04(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut declared: HashSet<String> = HashSet::new();
    let mut referenced: HashSet<String> = HashSet::new();

    for ac in ontology.iter() {
        match &ac.component {
            Component::DeclareClass(dc) => {
                let iri = dc.0 .0.as_ref();
                if !is_standard(iri) {
                    declared.insert(iri.to_string());
                }
            }
            Component::SubClassOf(ax) => {
                referenced.extend(collect_class_iris(&ax.sub));
                referenced.extend(collect_class_iris(&ax.sup));
            }
            Component::EquivalentClasses(ax) => {
                for ce in &ax.0 {
                    referenced.extend(collect_class_iris(ce));
                }
            }
            Component::DisjointClasses(ax) => {
                for ce in &ax.0 {
                    referenced.extend(collect_class_iris(ce));
                }
            }
            Component::ObjectPropertyDomain(ax) => {
                referenced.extend(collect_class_iris(&ax.ce));
            }
            Component::ObjectPropertyRange(ax) => {
                referenced.extend(collect_class_iris(&ax.ce));
            }
            Component::DataPropertyDomain(ax) => {
                referenced.extend(collect_class_iris(&ax.ce));
            }
            Component::ClassAssertion(ax) => {
                referenced.extend(collect_class_iris(&ax.ce));
            }
            _ => {}
        }
    }

    let unconnected: Vec<String> = declared
        .iter()
        .filter(|iri| !referenced.contains(iri.as_str()))
        .cloned()
        .collect();

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

// ── P13: Missing inverse relationships ───────────────────────────────────────

fn check_p13(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut obj_props: HashSet<String> = HashSet::new();
    let mut has_inverse: HashSet<String> = HashSet::new();

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
            _ => {}
        }
    }

    let missing: Vec<String> = obj_props
        .iter()
        .filter(|p| !has_inverse.contains(p.as_str()))
        .cloned()
        .collect();

    if missing.is_empty() {
        return vec![];
    }

    vec![DetectedPitfall {
        id: "P13".to_string(),
        title: "Missing inverse relationships".to_string(),
        description: "Object properties that could potentially have inverse relationships \
                      are not explicitly declared. Consider whether inverse properties apply."
            .to_string(),
        importance: "Important".to_string(),
        num_affected_elements: missing.len(),
        affected_elements: missing,
    }]
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

// ── P22: Using different naming conventions ──────────────────────────────────

fn check_p22(ontology: &SetOntology<ArcStr>) -> Vec<DetectedPitfall> {
    let mut style_counts: HashMap<NamingStyle, Vec<String>> = HashMap::new();

    for ac in ontology.iter() {
        let iri = match &ac.component {
            Component::DeclareClass(dc) => Some(dc.0 .0.as_ref()),
            Component::DeclareObjectProperty(dop) => Some(dop.0 .0.as_ref()),
            Component::DeclareDataProperty(ddp) => Some(ddp.0 .0.as_ref()),
            _ => None,
        };

        if let Some(iri) = iri {
            if is_standard(iri) {
                continue;
            }
            let name = local_name(iri);
            let style = detect_naming_style(name);
            if style != NamingStyle::Unknown {
                style_counts.entry(style).or_default().push(iri.to_string());
            }
        }
    }

    let distinct_styles: Vec<&NamingStyle> = style_counts.keys().collect();
    if distinct_styles.len() <= 1 {
        return vec![];
    }

    let affected: Vec<String> = style_counts.values().flatten().cloned().collect();

    vec![DetectedPitfall {
        id: "P22".to_string(),
        title: "Using different naming conventions".to_string(),
        description: format!(
            "The ontology uses {} different naming conventions across its entities. \
             Consistent naming (e.g. all UpperCamelCase for classes, lowerCamelCase for properties) \
             improves readability and interoperability.",
            distinct_styles.len()
        ),
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
