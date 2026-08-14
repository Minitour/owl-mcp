use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use oxigraph::model::{NamedNode, NamedOrBlankNode, Term};
use oxigraph::store::Store;
use regex::Regex;

use super::ns;
use crate::ontology::owl_api::OwlApiError;

pub enum Label {
    Ignore,
    Text(String),
}

pub fn default_ignore() -> HashSet<String> {
    [
        ns::OWL_ON_DATATYPE,
        "http://www.w3.org/2000/01/rdf-schema#seeAlso",
        ns::RDFS_LABEL,
        "http://www.w3.org/2000/01/rdf-schema#comment",
        ns::RDF_TYPE,
        "http://www.w3.org/2000/01/rdf-schema#isDefinedBy",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

pub fn default_rephrased() -> HashMap<String, String> {
    [
        (
            "http://www.w3.org/2002/07/owl#equivalentClass",
            "is same as",
        ),
        (
            "http://www.w3.org/2000/01/rdf-schema#subClassOf",
            "is a type of",
        ),
        ("http://www.w3.org/2002/07/owl#intersectionOf", "all of"),
        ("http://www.w3.org/2002/07/owl#unionOf", "any of"),
        (ns::OWL_DISJOINT_WITH, "is different from"),
        ("http://www.w3.org/2002/07/owl#withRestrictions", "must be"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

pub struct Vocabulary {
    relationship_labels: HashMap<String, String>,
    object_labels: HashMap<String, String>,
    rephrased: HashMap<String, String>,
    ignore: HashSet<String>,
    guard: HashSet<String>,
}

impl Vocabulary {
    pub fn new(store: &Store) -> Result<Self, OwlApiError> {
        Ok(Self {
            relationship_labels: load_relationship_labels(store)?,
            object_labels: load_object_labels(store)?,
            rephrased: default_rephrased(),
            ignore: default_ignore(),
            guard: [ns::RDF_TYPE, ns::RDFS_LABEL, ns::OWL_ON_DATATYPE]
                .into_iter()
                .map(String::from)
                .collect(),
        })
    }

    pub fn should_keep(&self, pred: &NamedNode) -> bool {
        self.guard.contains(pred.as_str())
    }

    pub fn get_relationship_label(&self, pred: &NamedNode) -> Label {
        self.lookup(&self.relationship_labels, pred.as_str())
    }

    pub fn get_class_label(&self, val: &Term) -> Label {
        match val {
            Term::NamedNode(n) => self.lookup(&self.object_labels, n.as_str()),
            Term::BlankNode(b) => Label::Text(b.as_str().to_string()),
            Term::Literal(l) => Label::Text(l.value().to_string()),
        }
    }

    fn lookup(&self, dictionary: &HashMap<String, String>, key: &str) -> Label {
        if self.ignore.contains(key) {
            return Label::Ignore;
        }
        if let Some(r) = self.rephrased.get(key) {
            return Label::Text(r.clone());
        }
        if let Some(r) = dictionary.get(key) {
            return Label::Text(r.clone());
        }
        Label::Text(from_uri_to_text(key))
    }
}

fn load_relationship_labels(store: &Store) -> Result<HashMap<String, String>, OwlApiError> {
    let rdfs_label = ns::named(ns::RDFS_LABEL);
    let mut explicit = HashMap::new();
    for quad in store.quads_for_pattern(None, Some(rdfs_label.as_ref()), None, None) {
        let quad = quad.map_err(|e| OwlApiError::Verbalize(e.to_string()))?;
        let NamedOrBlankNode::NamedNode(s) = quad.subject else {
            continue;
        };
        let Term::Literal(lit) = quad.object else {
            continue;
        };
        explicit.insert(s.as_str().to_string(), lit.value().to_string());
    }

    let mut map = HashMap::new();
    for quad in store.quads_for_pattern(None, None, None, None) {
        let quad = quad.map_err(|e| OwlApiError::Verbalize(e.to_string()))?;
        let iri = quad.predicate.as_str().to_string();
        if map.contains_key(&iri) {
            continue;
        }
        let mut label = explicit
            .get(&iri)
            .cloned()
            .unwrap_or_else(|| from_uri_to_text(&iri));
        if label.starts_with("http") {
            label = from_uri_to_text(&label);
        }
        map.insert(iri, strip_non_alnum(&label));
    }
    Ok(map)
}

fn load_object_labels(store: &Store) -> Result<HashMap<String, String>, OwlApiError> {
    let rdfs_label = ns::named(ns::RDFS_LABEL);
    let rdf_type = ns::named(ns::RDF_TYPE);
    let owl_class = Term::NamedNode(ns::named(ns::OWL_CLASS));

    let mut map = HashMap::new();
    for quad in store.quads_for_pattern(None, Some(rdfs_label.as_ref()), None, None) {
        let quad = quad.map_err(|e| OwlApiError::Verbalize(e.to_string()))?;
        let NamedOrBlankNode::NamedNode(s) = quad.subject else {
            continue;
        };
        let Term::Literal(lit) = quad.object else {
            continue;
        };
        insert_object_label(&mut map, s.as_str(), lit.value());
    }

    for quad in store.quads_for_pattern(
        None,
        Some(rdf_type.as_ref()),
        Some((&owl_class).into()),
        None,
    ) {
        let quad = quad.map_err(|e| OwlApiError::Verbalize(e.to_string()))?;
        let NamedOrBlankNode::NamedNode(s) = quad.subject else {
            continue;
        };
        map.entry(s.as_str().to_string())
            .or_insert_with(|| strip_non_alnum(&camel_to_snake(&from_uri_to_text(s.as_str()))));
    }
    Ok(map)
}

fn insert_object_label(map: &mut HashMap<String, String>, iri: &str, label: &str) {
    if iri == label && !label.starts_with("http") {
        return;
    }
    let mut label = label.to_string();
    if label.starts_with("http") {
        label = from_uri_to_text(&label);
    }
    label = camel_to_snake(&label);
    map.insert(iri.to_string(), strip_non_alnum(&label));
}

pub fn list_typed(store: &Store, ty: &str) -> Result<Vec<Term>, OwlApiError> {
    let rdf_type = ns::named(ns::RDF_TYPE);
    let ty_term = Term::NamedNode(ns::named(ty));
    let deprecated = ns::named(ns::OWL_DEPRECATED);
    let mut iris = Vec::new();
    for quad in
        store.quads_for_pattern(None, Some(rdf_type.as_ref()), Some((&ty_term).into()), None)
    {
        let quad = quad.map_err(|e| OwlApiError::Verbalize(e.to_string()))?;
        let NamedOrBlankNode::NamedNode(s) = quad.subject else {
            continue;
        };
        if is_deprecated(store, s.as_ref(), deprecated.as_ref())? {
            continue;
        }
        iris.push(Term::NamedNode(s));
    }
    Ok(iris)
}

fn is_deprecated(
    store: &Store,
    subject: oxigraph::model::NamedNodeRef<'_>,
    deprecated: oxigraph::model::NamedNodeRef<'_>,
) -> Result<bool, OwlApiError> {
    for quad in store.quads_for_pattern(Some(subject.into()), Some(deprecated), None, None) {
        let quad = quad.map_err(|e| OwlApiError::Verbalize(e.to_string()))?;
        if let Term::Literal(lit) = quad.object {
            if lit.value().eq_ignore_ascii_case("true") {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn from_uri_to_text(uri: &str) -> String {
    let text = if let Some((_, rest)) = uri.rsplit_once('#') {
        rest
    } else {
        uri.rsplit('/').next().unwrap_or(uri)
    };
    camel_to_snake(text).replace('_', " ")
}

fn camel_to_snake(name: &str) -> String {
    static RE1: OnceLock<Regex> = OnceLock::new();
    static RE2: OnceLock<Regex> = OnceLock::new();
    let re1 = RE1.get_or_init(|| Regex::new(r"(.)([A-Z][a-z]+)").unwrap());
    let re2 = RE2.get_or_init(|| Regex::new(r"([a-z0-9])([A-Z])").unwrap());
    let s = re1.replace_all(name, "${1}_${2}");
    re2.replace_all(&s, "${1}_${2}").to_lowercase()
}

fn strip_non_alnum(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
