use std::collections::HashSet;

use oxigraph::model::{NamedNode, Term};
use oxigraph::store::Store;

use super::{child_pairs, outgoing, Pattern};
use crate::ontology::owl_api::OwlApiError;
use crate::verbalizer::ns;
use crate::verbalizer::vocabulary::{Label, Vocabulary};
use crate::verbalizer::{VerbalizationEdge, VerbalizationNode};

pub struct OwlRestrictionPattern;

const RESTRICTION_PREDS: &[&str] = &[
    ns::OWL_ON_PROPERTY,
    ns::OWL_SOME_VALUES_FROM,
    ns::OWL_ALL_VALUES_FROM,
    ns::OWL_HAS_VALUE,
    ns::OWL_CARDINALITY,
    ns::OWL_MIN_CARDINALITY,
    ns::OWL_MAX_CARDINALITY,
    ns::OWL_QUALIFIED_CARDINALITY,
    ns::OWL_MIN_QUALIFIED_CARDINALITY,
    ns::OWL_MAX_QUALIFIED_CARDINALITY,
    ns::OWL_ON_CLASS,
];

impl Pattern for OwlRestrictionPattern {
    fn check(&self, results: &[(NamedNode, Term)]) -> bool {
        let actual: HashSet<&str> = results.iter().map(|(p, _)| p.as_str()).collect();
        RESTRICTION_PREDS
            .iter()
            .filter(|p| actual.contains(*p))
            .count()
            >= 2
    }

    fn normalize(
        &self,
        node: &mut VerbalizationNode,
        triples: &mut Vec<(Term, NamedNode, Term)>,
        store: &Store,
        vocab: &Vocabulary,
    ) -> Result<Vec<(NamedNode, Term)>, OwlApiError> {
        let rows = outgoing(store, node)?;

        let mut next_concept: Option<Term> = None;
        let mut quantifier: Option<NamedNode> = None;
        let mut property: Option<Term> = None;
        let mut literal: Option<Term> = None;
        let mut on_class: Option<Term> = None;

        for (relation, obj) in &rows {
            if matches!(vocab.get_relationship_label(relation), Label::Ignore) {
                triples.push((node.concept.clone(), relation.clone(), obj.clone()));
                continue;
            }

            let iri = relation.as_str();
            if iri == ns::OWL_ON_PROPERTY {
                property = Some(obj.clone());
            }
            if matches!(
                iri,
                ns::OWL_SOME_VALUES_FROM | ns::OWL_ALL_VALUES_FROM | ns::OWL_HAS_VALUE
            ) {
                quantifier = Some(relation.clone());
                next_concept = Some(obj.clone());
            }
            if iri.ends_with("cardinality") || iri.ends_with("Cardinality") {
                quantifier = Some(relation.clone());
                literal = Some(obj.clone());
                if next_concept.is_none() {
                    next_concept = Some(empty_term());
                }
            }
            if iri == ns::OWL_ON_CLASS {
                on_class = Some(obj.clone());
                next_concept = Some(obj.clone());
            }
            triples.push((node.concept.clone(), relation.clone(), obj.clone()));
        }

        let Some(quantifier) = quantifier else {
            return Ok(Vec::new());
        };
        let Some(property) = property else {
            return Ok(Vec::new());
        };
        let concept = next_concept.unwrap_or_else(empty_term);

        let mut next_node = VerbalizationNode::new(concept);
        if is_empty_term(&next_node.concept) {
            next_node.display = Some(String::new());
        }

        let combo_iri = format!(
            "{}{}",
            quantifier.as_str(),
            match &property {
                Term::NamedNode(n) => n.as_str(),
                _ => "",
            }
        );
        // Internal tree key only (never serialized). Concatenating two IRIs is
        // not a valid IRI (`#` twice); Python's URIRef accepted it anyway.
        let combo = NamedNode::new_unchecked(combo_iri);
        let mut edge = VerbalizationEdge {
            relationship: combo,
            node: next_node,
            display: None,
        };

        let q_iri = quantifier.as_str();
        let prop_label = match vocab.get_class_label(&property) {
            Label::Ignore => String::new(),
            Label::Text(s) => s,
        };
        edge.display = Some(if q_iri.ends_with("someValuesFrom") {
            format!("at least {prop_label} some")
        } else if q_iri.ends_with("allValuesFrom") {
            format!("only {prop_label}")
        } else if q_iri.ends_with("hasValue") {
            format!("must {prop_label}")
        } else if q_iri.to_lowercase().ends_with("cardinality") {
            cardinality_label(vocab, q_iri, &property, literal.as_ref(), on_class.as_ref())
        } else {
            let q_label = match vocab.get_relationship_label(&quantifier) {
                Label::Ignore => String::new(),
                Label::Text(s) => s,
            };
            format!("{prop_label} {q_label}")
        });

        node.references.push(edge);
        Ok(child_pairs(node))
    }

    fn guarded_iris(&self) -> &'static [&str] {
        &[
            ns::OWL_SOME_VALUES_FROM,
            ns::OWL_ALL_VALUES_FROM,
            ns::OWL_HAS_VALUE,
            ns::OWL_CARDINALITY,
            ns::OWL_MIN_CARDINALITY,
            ns::OWL_MAX_CARDINALITY,
            ns::OWL_QUALIFIED_CARDINALITY,
            ns::OWL_MIN_QUALIFIED_CARDINALITY,
            ns::OWL_MAX_QUALIFIED_CARDINALITY,
            ns::OWL_ON_CLASS,
        ]
    }
}

fn empty_term() -> Term {
    Term::Literal(oxigraph::model::Literal::new_simple_literal(""))
}

fn is_empty_term(term: &Term) -> bool {
    matches!(term, Term::Literal(l) if l.value().is_empty())
}

fn cardinality_label(
    vocab: &Vocabulary,
    quantifier: &str,
    property: &Term,
    literal: Option<&Term>,
    on_class: Option<&Term>,
) -> String {
    let mut prop_label = match vocab.get_class_label(property) {
        Label::Ignore => String::new(),
        Label::Text(s) => s,
    };
    let literal_value = match literal {
        Some(Term::Literal(l)) => l.value().to_string(),
        Some(t) => t.to_string(),
        None => "0".to_string(),
    };
    let n: f64 = literal_value.parse().unwrap_or(0.0);
    let mut on_class_label = " ".to_string();
    let plural = if n > 1.0 && !prop_label.ends_with('s') {
        "s"
    } else {
        ""
    };
    if prop_label.starts_with("has") {
        prop_label = prop_label.replacen("has ", "", 1);
    }
    if let Some(c) = on_class {
        match vocab.get_class_label(c) {
            Label::Ignore => {}
            Label::Text(s) => on_class_label = format!(" {s} "),
        }
    }
    let q = quantifier.to_lowercase();
    if q.ends_with("cardinality")
        && n == 0.0
        && !q.contains("min")
        && !q.contains("max")
        && !q.contains("qualified")
    {
        format!("has zero {on_class_label} {prop_label}s")
    } else if q.ends_with("qualifiedcardinality")
        || (q.ends_with("cardinality") && !q.contains("min") && !q.contains("max"))
    {
        format!("has exactly {literal_value}{on_class_label}{prop_label}{plural}")
    } else if q.ends_with("mincardinality") || q.ends_with("minqualifiedcardinality") {
        format!("has at least {literal_value}{on_class_label}{prop_label}{plural}")
    } else if q.ends_with("maxcardinality") || q.ends_with("maxqualifiedcardinality") {
        format!("has at most {literal_value}{on_class_label}{prop_label}{plural}")
    } else {
        format!("has {literal_value}{on_class_label}{prop_label}{plural}")
    }
}
