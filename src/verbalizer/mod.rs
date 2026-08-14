//! OWL ontology verbalizer: RDF graph → Controlled Natural Language.
//!
//! Port of [ontology-verbalizer](https://github.com/Minitour/ontology-verbalizer) (CNL only).

mod cnl;
mod fragment;
mod ns;
mod patterns;
mod vocabulary;

use std::collections::{HashMap, HashSet};

use oxigraph::io::RdfFormat;
use oxigraph::model::{NamedNode, NamedOrBlankNode, Term};
use oxigraph::store::Store;
use serde::{Deserialize, Serialize};

use crate::ontology::owl_api::OwlApiError;

use cnl::{collapse_ws, join_clauses, with_article};
use fragment::generate_fragment;
use patterns::PATTERNS;
use vocabulary::{list_typed, Label, Vocabulary};

/// One verbalized concept (class or individual).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbalizeEntry {
    pub root: String,
    pub fragment: String,
    pub text: String,
    pub statements: usize,
    pub unique_concepts: usize,
    pub unique_relationships: usize,
}

/// Verbalize `starting` IRIs (or every class + named individual when `starting` is empty).
pub fn verbalize(
    rdf_ntriples: &[u8],
    starting: Option<&[String]>,
    limit: usize,
) -> Result<Vec<VerbalizeEntry>, OwlApiError> {
    let store = Store::new().map_err(|e| OwlApiError::Verbalize(e.to_string()))?;
    store
        .load_from_slice(RdfFormat::NTriples, rdf_ntriples)
        .map_err(|e| OwlApiError::Verbalize(format!("RDF load error: {e}")))?;

    let vocab = Vocabulary::new(&store)?;
    let verbalizer = Verbalizer::new(&store, vocab);

    let roots: Vec<Term> = if let Some(iris) = starting {
        iris.iter()
            .map(|s| {
                let s = s
                    .strip_prefix('<')
                    .and_then(|t| t.strip_suffix('>'))
                    .unwrap_or(s);
                NamedNode::new(s.to_string())
                    .map(Term::NamedNode)
                    .map_err(|e| OwlApiError::Verbalize(e.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let mut r = list_typed(&store, ns::OWL_CLASS)?;
        r.extend(list_typed(&store, ns::OWL_NAMED_INDIVIDUAL)?);
        r
    };

    let mut out = Vec::new();
    for root in roots {
        if out.len() >= limit {
            break;
        }
        if let Some(entry) = verbalizer.verbalize(&root)? {
            out.push(entry);
        }
    }
    Ok(out)
}

pub(crate) struct VerbalizationEdge {
    pub(crate) relationship: NamedNode,
    pub(crate) node: VerbalizationNode,
    pub(crate) display: Option<String>,
}

impl VerbalizationEdge {
    fn verbalize(&self) -> String {
        let mut display = self.display.clone().unwrap_or_default();
        if display.starts_with('#') {
            display.clear();
        }
        let node = self.node.verbalize().trim().to_string();
        if node.is_empty() {
            display
        } else {
            format!("{display} {node}")
        }
    }
}

pub(crate) struct VerbalizationNode {
    pub(crate) concept: Term,
    pub(crate) references: Vec<VerbalizationEdge>,
    pub(crate) display: Option<String>,
}

impl VerbalizationNode {
    pub(crate) fn new(concept: Term) -> Self {
        Self {
            concept,
            references: Vec::new(),
            display: None,
        }
    }

    fn set_display(&mut self, value: impl Into<String>) {
        if self.display.is_none() {
            self.display = Some(value.into());
        }
    }

    fn get_next_node_mut(
        &mut self,
        relationship: &NamedNode,
        concept: &Term,
    ) -> Option<&mut VerbalizationNode> {
        self.references.iter_mut().find_map(|e| {
            if &e.relationship == relationship && &e.node.concept == concept {
                Some(&mut e.node)
            } else {
                None
            }
        })
    }

    fn verbalize(&self) -> String {
        let parts: Vec<String> = self
            .references
            .iter()
            .map(|e| e.verbalize().trim().to_string())
            .collect();
        let mut display = self.display.clone().unwrap_or_default();
        if matches!(self.concept, Term::BlankNode(_)) {
            display.clear();
        }
        let next_text = join_clauses(&parts);
        format!("{}{next_text}", with_article(&display))
    }
}

struct Verbalizer<'a> {
    store: &'a Store,
    vocab: Vocabulary,
}

impl<'a> Verbalizer<'a> {
    fn new(store: &'a Store, vocab: Vocabulary) -> Self {
        Self { store, vocab }
    }

    fn verbalize(&self, starting: &Term) -> Result<Option<VerbalizeEntry>, OwlApiError> {
        let mut triples: Vec<(Term, NamedNode, Term)> = Vec::new();
        let mut node = VerbalizationNode::new(starting.clone());
        self.expand(&mut node, &mut triples)?;

        let mut sentences: Vec<String> = node
            .references
            .iter()
            .map(|r| {
                let raw = format!(
                    "{} {}.",
                    node.display.clone().unwrap_or_default(),
                    r.verbalize().trim()
                );
                collapse_ws(&raw)
            })
            .collect();
        sentences.sort();
        sentences.dedup();
        let text = sentences.join("\n");
        if sentences.is_empty() {
            return Ok(None);
        }

        let mut relationship_counter: HashMap<String, usize> = HashMap::new();
        let mut concepts: HashSet<String> = HashSet::new();
        for (s, p, o) in &triples {
            *relationship_counter
                .entry(p.as_str().to_string())
                .or_insert(0) += 1;
            if let Term::NamedNode(n) = s {
                concepts.insert(n.as_str().to_string());
            }
            if let Term::NamedNode(n) = o {
                concepts.insert(n.as_str().to_string());
            }
        }

        Ok(Some(VerbalizeEntry {
            root: term_root(starting),
            fragment: generate_fragment(&self.vocab, &triples)?,
            text,
            statements: sentences.len(),
            unique_concepts: concepts.len(),
            unique_relationships: relationship_counter.len(),
        }))
    }

    fn expand(
        &self,
        node: &mut VerbalizationNode,
        triples: &mut Vec<(Term, NamedNode, Term)>,
    ) -> Result<(), OwlApiError> {
        let po = next_step(self.store, &node.concept)?;

        for pattern in PATTERNS {
            if !pattern.check(&po) {
                continue;
            }
            let children = pattern.normalize(node, triples, self.store, &self.vocab)?;
            node.set_display(label_text(&self.vocab.get_class_label(&node.concept)));
            self.finish_children(node, triples, &children, pattern.guarded_iris())?;
            return Ok(());
        }

        node.set_display(label_text(&self.vocab.get_class_label(&node.concept)));
        for (relation, obj) in po {
            self.add_plain_edge(node, triples, relation, obj)?;
        }
        Ok(())
    }

    fn finish_children(
        &self,
        node: &mut VerbalizationNode,
        triples: &mut Vec<(Term, NamedNode, Term)>,
        children: &[(NamedNode, Term)],
        required_iris: &[&str],
    ) -> Result<(), OwlApiError> {
        for (relation, obj) in children {
            let relation_display = self.vocab.get_relationship_label(relation);
            if required_iris.contains(&relation.as_str())
                && matches!(relation_display, Label::Ignore)
            {
                return Err(OwlApiError::Verbalize(format!(
                    "Cannot perform verbalization because required IRI {} is ignored.",
                    relation.as_str()
                )));
            }
            if let Some(next) = node.get_next_node_mut(relation, obj) {
                next.set_display(object_display(&self.vocab, obj));
            }
            if is_bnode(obj) {
                if let Some(next) = node.get_next_node_mut(relation, obj) {
                    self.expand(next, triples)?;
                }
            }
        }
        Ok(())
    }

    fn add_plain_edge(
        &self,
        node: &mut VerbalizationNode,
        triples: &mut Vec<(Term, NamedNode, Term)>,
        relation: NamedNode,
        obj: Term,
    ) -> Result<(), OwlApiError> {
        let relation_display = self.vocab.get_relationship_label(&relation);
        if matches!(relation_display, Label::Ignore) {
            if self.vocab.should_keep(&relation) {
                triples.push((node.concept.clone(), relation, obj));
            }
            return Ok(());
        }

        let mut next_node = VerbalizationNode::new(obj.clone());
        next_node.set_display(object_display(&self.vocab, &obj));
        let mut edge = VerbalizationEdge {
            relationship: relation.clone(),
            node: next_node,
            display: None,
        };
        if let Label::Text(s) = relation_display {
            edge.display = Some(s);
        }
        let recurse = is_bnode(&edge.node.concept);
        triples.push((node.concept.clone(), relation, edge.node.concept.clone()));
        node.references.push(edge);
        if recurse {
            if let Some(next) = node.references.last_mut() {
                self.expand(&mut next.node, triples)?;
            }
        }
        Ok(())
    }
}

fn object_display(vocab: &Vocabulary, obj: &Term) -> String {
    match obj {
        Term::Literal(l) => l.value().to_string(),
        _ => label_text(&vocab.get_class_label(obj)),
    }
}

fn label_text(label: &Label) -> String {
    match label {
        Label::Ignore => String::new(),
        Label::Text(s) => s.clone(),
    }
}

fn is_bnode(term: &Term) -> bool {
    matches!(term, Term::BlankNode(_))
}

fn term_root(term: &Term) -> String {
    match term {
        Term::NamedNode(n) => n.as_str().to_string(),
        Term::BlankNode(b) => b.as_str().to_string(),
        Term::Literal(l) => l.value().to_string(),
    }
}

/// Outgoing `(predicate, object)` pairs from a subject term.
pub(crate) fn next_step(
    store: &Store,
    concept: &Term,
) -> Result<Vec<(NamedNode, Term)>, OwlApiError> {
    let subject: NamedOrBlankNode = match concept {
        Term::NamedNode(n) => n.clone().into(),
        Term::BlankNode(b) => b.clone().into(),
        Term::Literal(_) => return Ok(Vec::new()),
    };
    let mut rows = Vec::new();
    for quad in store.quads_for_pattern(Some(subject.as_ref()), None, None, None) {
        let quad = quad.map_err(|e| OwlApiError::Verbalize(e.to_string()))?;
        rows.push((quad.predicate, quad.object));
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ontology::owl_api::OwlApi;
    use tempfile::NamedTempFile;

    const FIXTURE: &str = include_str!("../../tests/fixtures/pizza.ofn");
    const GOLDEN_VEGETARIAN: &str =
        include_str!("../../tests/fixtures/golden/vegetarian_pizza.cnl");
    const GOLDEN_MARGHERITA: &str = include_str!("../../tests/fixtures/golden/margherita.cnl");

    fn verbalize_fixture() -> Vec<VerbalizeEntry> {
        let tmp = NamedTempFile::with_suffix(".ofn").unwrap();
        std::fs::write(tmp.path(), FIXTURE).unwrap();
        let api = OwlApi::load(tmp.path(), false, false).unwrap();
        let rdf = api.to_rdf_bytes().unwrap();
        verbalize(&rdf, None, 20).unwrap()
    }

    fn entry<'a>(entries: &'a [VerbalizeEntry], needle: &str) -> &'a VerbalizeEntry {
        entries
            .iter()
            .find(|e| e.root.contains(needle))
            .unwrap_or_else(|| panic!("missing entry containing {needle}, got {entries:?}"))
    }

    #[test]
    fn verbalizes_subclass_and_disjoint() {
        let entries = verbalize_fixture();
        let veg = entry(&entries, "VegetarianPizza");
        assert_eq!(veg.text, GOLDEN_VEGETARIAN.trim_end());
        assert!(!veg.fragment.is_empty());
        assert!(veg.fragment.contains("vegetarian_pizza"));
        assert_eq!(veg.statements, 2);
    }

    #[test]
    fn verbalizes_restriction_union() {
        let entries = verbalize_fixture();
        let marg = entry(&entries, "Margherita");
        assert_eq!(marg.text, GOLDEN_MARGHERITA.trim_end());
        assert!(marg.unique_concepts >= 2);
        assert!(marg.unique_relationships >= 1);
        assert!(!marg.fragment.is_empty());
    }

    #[test]
    fn verbalize_single_iri() {
        let tmp = NamedTempFile::with_suffix(".ofn").unwrap();
        std::fs::write(tmp.path(), FIXTURE).unwrap();
        let api = OwlApi::load(tmp.path(), false, false).unwrap();
        let rdf = api.to_rdf_bytes().unwrap();
        let entries = verbalize(
            &rdf,
            Some(&["http://example.org/pizza#VegetarianPizza".to_string()]),
            10,
        )
        .unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].root.contains("VegetarianPizza"));
        assert_eq!(entries[0].text, GOLDEN_VEGETARIAN.trim_end());
    }

    #[test]
    fn default_ignore_covers_upstream_metadata_iris() {
        let ignore = vocabulary::default_ignore();
        assert!(ignore.contains("http://www.w3.org/2003/06/sw-vocab-status/ns#term_status"));
        assert!(ignore.contains("http://www.w3.org/2000/01/rdf-schema#Class"));
    }
}
