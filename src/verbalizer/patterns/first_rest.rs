use std::collections::HashSet;

use oxigraph::model::{NamedNode, Term};
use oxigraph::store::Store;

use super::{child_pairs, outgoing, Pattern};
use crate::ontology::owl_api::OwlApiError;
use crate::verbalizer::ns;
use crate::verbalizer::vocabulary::Vocabulary;
use crate::verbalizer::{VerbalizationEdge, VerbalizationNode};

pub struct OwlFirstRestPattern;

impl Pattern for OwlFirstRestPattern {
    fn check(&self, results: &[(NamedNode, Term)]) -> bool {
        let actual: HashSet<&str> = results.iter().map(|(p, _)| p.as_str()).collect();
        actual.len() == 2 && actual.contains(ns::RDF_FIRST) && actual.contains(ns::RDF_REST)
    }

    fn normalize(
        &self,
        node: &mut VerbalizationNode,
        triples: &mut Vec<(Term, NamedNode, Term)>,
        store: &Store,
        _vocab: &Vocabulary,
    ) -> Result<Vec<(NamedNode, Term)>, OwlApiError> {
        let collection = ns::named(ns::RDF_COLLECTION);
        let mut current = node.concept.clone();
        loop {
            if matches!(&current, Term::NamedNode(n) if n.as_str() == ns::RDF_NIL) {
                break;
            }
            let tmp = VerbalizationNode::new(current.clone());
            let rows = outgoing(store, &tmp)?;
            let mut rest: Option<Term> = None;
            for (relation, obj) in rows {
                if relation.as_str() == ns::RDF_FIRST {
                    node.references.push(VerbalizationEdge {
                        relationship: collection.clone(),
                        node: VerbalizationNode::new(obj.clone()),
                        display: Some("#collection".to_string()),
                    });
                } else if relation.as_str() == ns::RDF_REST {
                    rest = Some(obj.clone());
                }
                triples.push((current.clone(), relation, obj));
            }
            match rest {
                Some(obj) => current = obj,
                None => break,
            }
        }
        Ok(child_pairs(node))
    }

    fn guarded_iris(&self) -> &'static [&str] {
        &[]
    }
}
