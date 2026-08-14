use oxigraph::model::{NamedNode, Term};
use oxigraph::store::Store;

use super::{child_pairs, outgoing, Pattern};
use crate::ontology::owl_api::OwlApiError;
use crate::verbalizer::ns;
use crate::verbalizer::vocabulary::{Label, Vocabulary};
use crate::verbalizer::{VerbalizationEdge, VerbalizationNode};

pub struct OwlDisjointWith;

impl Pattern for OwlDisjointWith {
    fn check(&self, results: &[(NamedNode, Term)]) -> bool {
        results
            .iter()
            .any(|(p, _)| p.as_str() == ns::OWL_DISJOINT_WITH)
    }

    fn normalize(
        &self,
        node: &mut VerbalizationNode,
        triples: &mut Vec<(Term, NamedNode, Term)>,
        store: &Store,
        vocab: &Vocabulary,
    ) -> Result<Vec<(NamedNode, Term)>, OwlApiError> {
        let rows = outgoing(store, node)?;
        let disjoint = ns::named(ns::OWL_DISJOINT_WITH);
        let collection = ns::named(ns::RDF_COLLECTION);

        let mut intermediate = VerbalizationNode::new(Term::Literal(
            oxigraph::model::Literal::new_simple_literal(""),
        ));
        intermediate.display = Some(String::new());
        let mut intermediate_edge = VerbalizationEdge {
            relationship: disjoint.clone(),
            node: intermediate,
            display: None,
        };
        if let Label::Text(s) = vocab.get_relationship_label(&disjoint) {
            intermediate_edge.display = Some(s);
        }

        for (relation, obj) in &rows {
            if relation.as_str() != ns::OWL_DISJOINT_WITH {
                continue;
            }
            let mut next_node = VerbalizationNode::new(obj.clone());
            next_node.display = Some(match vocab.get_class_label(obj) {
                Label::Ignore => String::new(),
                Label::Text(s) => s,
            });
            intermediate_edge.node.references.push(VerbalizationEdge {
                relationship: collection.clone(),
                node: next_node,
                display: Some("#collection".to_string()),
            });
            triples.push((node.concept.clone(), relation.clone(), obj.clone()));
        }
        node.references.push(intermediate_edge);

        for (relation, obj) in &rows {
            if relation.as_str() == ns::OWL_DISJOINT_WITH {
                continue;
            }
            match vocab.get_relationship_label(relation) {
                Label::Ignore => {
                    if vocab.should_keep(relation) {
                        triples.push((node.concept.clone(), relation.clone(), obj.clone()));
                    }
                    continue;
                }
                Label::Text(display) => {
                    node.references.push(VerbalizationEdge {
                        relationship: relation.clone(),
                        node: VerbalizationNode::new(obj.clone()),
                        display: Some(display),
                    });
                    triples.push((node.concept.clone(), relation.clone(), obj.clone()));
                }
            }
        }

        Ok(child_pairs(node))
    }

    fn guarded_iris(&self) -> &'static [&str] {
        &[ns::OWL_DISJOINT_WITH]
    }
}
