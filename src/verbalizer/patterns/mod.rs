mod disjoint;
mod first_rest;
mod restriction;

use oxigraph::model::{NamedNode, Term};
use oxigraph::store::Store;

use super::vocabulary::Vocabulary;
use super::{next_step, VerbalizationNode};
use crate::ontology::owl_api::OwlApiError;

pub use disjoint::OwlDisjointWith;
pub use first_rest::OwlFirstRestPattern;
pub use restriction::OwlRestrictionPattern;

pub trait Pattern {
    fn check(&self, results: &[(NamedNode, Term)]) -> bool;
    fn normalize(
        &self,
        node: &mut VerbalizationNode,
        triples: &mut Vec<(Term, NamedNode, Term)>,
        store: &Store,
        vocab: &Vocabulary,
    ) -> Result<Vec<(NamedNode, Term)>, OwlApiError>;
    fn guarded_iris(&self) -> &'static [&str];
}

pub const PATTERNS: &[&dyn Pattern] = &[
    &OwlRestrictionPattern,
    &OwlFirstRestPattern,
    &OwlDisjointWith,
];

pub(crate) fn outgoing(
    store: &Store,
    node: &VerbalizationNode,
) -> Result<Vec<(NamedNode, Term)>, OwlApiError> {
    next_step(store, &node.concept)
}

pub(crate) fn child_pairs(node: &VerbalizationNode) -> Vec<(NamedNode, Term)> {
    node.references
        .iter()
        .map(|r| (r.relationship.clone(), r.node.concept.clone()))
        .collect()
}
