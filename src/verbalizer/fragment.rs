use oxigraph::io::{RdfFormat, RdfSerializer};
use oxigraph::model::{GraphName, GraphNameRef, NamedNode, NamedOrBlankNode, Quad, Term};
use oxigraph::store::Store;

use super::ns;
use super::vocabulary::{Label, Vocabulary};
use crate::ontology::owl_api::OwlApiError;

pub fn generate_fragment(
    vocab: &Vocabulary,
    triples: &[(Term, NamedNode, Term)],
) -> Result<String, OwlApiError> {
    let store = Store::new().map_err(|e| OwlApiError::Verbalize(e.to_string()))?;
    for (subject, predicate, obj) in triples {
        let Some(s) = rewrite_subject(vocab, subject) else {
            continue;
        };
        let Some(p) = rewrite_predicate(vocab, predicate) else {
            continue;
        };
        let Some(o) = rewrite_object(vocab, obj) else {
            continue;
        };
        store
            .insert(&Quad::new(s, p, o, GraphName::DefaultGraph))
            .map_err(|e| OwlApiError::Verbalize(e.to_string()))?;
    }

    let serializer = RdfSerializer::from_format(RdfFormat::Turtle)
        .with_prefix("", ns::FRAGMENT_NS)
        .map_err(|e| OwlApiError::Verbalize(e.to_string()))?;
    let mut buf = Vec::new();
    store
        .dump_graph_to_writer(GraphNameRef::DefaultGraph, serializer, &mut buf)
        .map_err(|e| OwlApiError::Verbalize(e.to_string()))?;
    String::from_utf8(buf).map_err(|e| OwlApiError::Verbalize(e.to_string()))
}

fn rewrite_subject(vocab: &Vocabulary, subject: &Term) -> Option<NamedOrBlankNode> {
    match subject {
        Term::BlankNode(b) => Some(NamedOrBlankNode::BlankNode(b.clone())),
        Term::NamedNode(_) => match vocab.get_class_label(subject) {
            Label::Ignore => None,
            Label::Text(rep) => Some(NamedOrBlankNode::NamedNode(display_to_named(&rep)?)),
        },
        Term::Literal(_) => None,
    }
}

fn rewrite_predicate(vocab: &Vocabulary, predicate: &NamedNode) -> Option<NamedNode> {
    if ns::is_vocab_ns(predicate.as_str()) {
        return Some(predicate.clone());
    }
    match vocab.get_relationship_label(predicate) {
        Label::Ignore => None,
        Label::Text(label) => display_to_named(&label),
    }
}

fn rewrite_object(vocab: &Vocabulary, obj: &Term) -> Option<Term> {
    match obj {
        Term::BlankNode(b) => Some(Term::BlankNode(b.clone())),
        Term::Literal(l) => Some(Term::Literal(l.clone())),
        Term::NamedNode(n) if n.as_str().starts_with(ns::OWL_NS) => Some(obj.clone()),
        Term::NamedNode(_) => match vocab.get_class_label(obj) {
            Label::Ignore => None,
            Label::Text(rep) => Some(Term::NamedNode(display_to_named(&rep)?)),
        },
    }
}

fn display_to_named(display: &str) -> Option<NamedNode> {
    if display.starts_with("http") {
        return NamedNode::new(display.to_string()).ok();
    }
    let ident: String = display
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == ' ' || c == '\n' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .to_lowercase()
        .replace(' ', "_");
    NamedNode::new(format!("{}{ident}", ns::FRAGMENT_NS)).ok()
}
