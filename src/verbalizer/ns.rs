//! OWL/RDF IRI constants used by the verbalizer.

use oxigraph::model::NamedNode;

pub const OWL_NS: &str = "http://www.w3.org/2002/07/owl#";
pub const RDF_NS: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
pub const RDFS_NS: &str = "http://www.w3.org/2000/01/rdf-schema#";
pub const FRAGMENT_NS: &str = "https://zaitoun.dev/onto/";

pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
pub const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
pub const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
pub const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";
pub const RDF_COLLECTION: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#collection";
pub const RDFS_LABEL: &str = "http://www.w3.org/2000/01/rdf-schema#label";

pub const OWL_CLASS: &str = "http://www.w3.org/2002/07/owl#Class";
pub const OWL_NAMED_INDIVIDUAL: &str = "http://www.w3.org/2002/07/owl#NamedIndividual";
pub const OWL_DEPRECATED: &str = "http://www.w3.org/2002/07/owl#deprecated";
pub const OWL_ON_PROPERTY: &str = "http://www.w3.org/2002/07/owl#onProperty";
pub const OWL_SOME_VALUES_FROM: &str = "http://www.w3.org/2002/07/owl#someValuesFrom";
pub const OWL_ALL_VALUES_FROM: &str = "http://www.w3.org/2002/07/owl#allValuesFrom";
pub const OWL_HAS_VALUE: &str = "http://www.w3.org/2002/07/owl#hasValue";
pub const OWL_CARDINALITY: &str = "http://www.w3.org/2002/07/owl#cardinality";
pub const OWL_MIN_CARDINALITY: &str = "http://www.w3.org/2002/07/owl#minCardinality";
pub const OWL_MAX_CARDINALITY: &str = "http://www.w3.org/2002/07/owl#maxCardinality";
pub const OWL_QUALIFIED_CARDINALITY: &str = "http://www.w3.org/2002/07/owl#qualifiedCardinality";
pub const OWL_MIN_QUALIFIED_CARDINALITY: &str =
    "http://www.w3.org/2002/07/owl#minQualifiedCardinality";
pub const OWL_MAX_QUALIFIED_CARDINALITY: &str =
    "http://www.w3.org/2002/07/owl#maxQualifiedCardinality";
pub const OWL_ON_CLASS: &str = "http://www.w3.org/2002/07/owl#onClass";
pub const OWL_DISJOINT_WITH: &str = "http://www.w3.org/2002/07/owl#disjointWith";
pub const OWL_ON_DATATYPE: &str = "http://www.w3.org/2002/07/owl#onDatatype";

pub fn named(iri: &str) -> NamedNode {
    NamedNode::new_unchecked(iri)
}

pub fn is_vocab_ns(iri: &str) -> bool {
    iri.starts_with(OWL_NS) || iri.starts_with(RDF_NS) || iri.starts_with(RDFS_NS)
}
