use std::collections::HashSet;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ontology::manager::OntologyManager;
use crate::ontology::owl_api::{OwlApi, OwlApiError};

pub type Manager = Arc<OntologyManager>;

fn text(s: impl Into<String>) -> Vec<String> {
    vec![s.into()]
}

async fn with_api<R>(
    manager: &Manager,
    path: &str,
    readonly: bool,
    create: bool,
    f: impl FnOnce(&mut OwlApi) -> Result<R, OwlApiError>,
) -> Result<R, OwlApiError> {
    let handle = manager.get_or_load(path, readonly, create).await?;
    let mut api = handle.lock().await;
    f(&mut api)
}

// ── Axiom operations ──────────────────────────────────────────────────────────

/// Add a single OWL axiom in functional syntax to the ontology file. E.g. SubClassOf(:Dog :Animal)
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct AddAxiom {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Axiom in OWL functional syntax, e.g. SubClassOf(:Dog :Animal)
    pub axiom_str: String,
}

impl AddAxiom {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        with_api(manager, &params.owl_file_path, false, true, |api| {
            api.add_axiom(&params.axiom_str).map(text)
        })
        .await
    }
}

/// Add multiple OWL axioms in functional syntax to the ontology file. Stops on the first failure.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct AddAxioms {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// List of axioms in OWL functional syntax
    pub axiom_strs: Vec<String>,
}

impl AddAxioms {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        with_api(manager, &params.owl_file_path, false, true, |api| {
            api.add_axioms(&params.axiom_strs).map(text)
        })
        .await
    }
}

/// Add a data property assertion (DataPropertyAssertion) where the literal VALUE is
/// supplied as a separate field. Use this instead of add_axiom for long or special-character
/// values (containing ; = / , quotes or newlines): the server constructs the axiom directly, so
/// no escaping or shell-quoting is needed. property/subject accept an IRI, CURIE, or <full-iri>.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct AddDataPropertyAssertion {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Data property IRI or CURIE, e.g. 'pf:metaprops' or '<http://example.org/metaprops>'
    pub property: String,
    /// Subject individual IRI or CURIE
    pub subject: String,
    /// The literal value, stored verbatim (no escaping required)
    pub value: String,
    /// Optional datatype IRI or CURIE (e.g. 'xsd:string'). Ignored if `lang` is set.
    pub datatype: Option<String>,
    /// Optional language tag (e.g. 'en'); produces a language-tagged literal.
    pub lang: Option<String>,
}

impl AddDataPropertyAssertion {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        with_api(manager, &params.owl_file_path, false, true, |api| {
            api.add_data_property_assertion(
                &params.property,
                &params.subject,
                &params.value,
                params.datatype.as_deref(),
                params.lang.as_deref(),
            )
            .map(text)
        })
        .await
    }
}

/// Add an annotation assertion (AnnotationAssertion) where the literal VALUE is
/// supplied as a separate field. Use this instead of add_axiom for long or special-character
/// values (containing ; = / , quotes or newlines): the server constructs the axiom directly, so
/// no escaping or shell-quoting is needed. property/subject accept an IRI, CURIE, or <full-iri>.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct AddAnnotationAssertion {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Annotation property IRI or CURIE, e.g. 'rdfs:label'
    pub property: String,
    /// Subject IRI or CURIE the annotation applies to
    pub subject: String,
    /// The literal value, stored verbatim (no escaping required)
    pub value: String,
    /// Optional datatype IRI or CURIE (e.g. 'xsd:string'). Ignored if `lang` is set.
    pub datatype: Option<String>,
    /// Optional language tag (e.g. 'en'); produces a language-tagged literal.
    pub lang: Option<String>,
}

impl AddAnnotationAssertion {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        with_api(manager, &params.owl_file_path, false, true, |api| {
            api.add_annotation_assertion(
                &params.property,
                &params.subject,
                &params.value,
                params.datatype.as_deref(),
                params.lang.as_deref(),
            )
            .map(text)
        })
        .await
    }
}

/// Add an object property assertion (ObjectPropertyAssertion) linking a subject
/// individual to a target individual via an object property. property/subject/target accept an
/// IRI, CURIE, or <full-iri>.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct AddObjectPropertyAssertion {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Object property IRI or CURIE
    pub property: String,
    /// Subject individual IRI or CURIE (the `from` individual)
    pub subject: String,
    /// Target individual IRI or CURIE (the `to` individual)
    pub target: String,
}

impl AddObjectPropertyAssertion {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        with_api(manager, &params.owl_file_path, false, true, |api| {
            api.add_object_property_assertion(&params.property, &params.subject, &params.target)
                .map(text)
        })
        .await
    }
}

/// Add a class assertion (ClassAssertion) stating that an individual is an instance
/// of a class. class/individual accept an IRI, CURIE, or <full-iri>.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct AddClassAssertion {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Class IRI or CURIE
    pub class: String,
    /// Individual IRI or CURIE
    pub individual: String,
}

impl AddClassAssertion {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        with_api(manager, &params.owl_file_path, false, true, |api| {
            api.add_class_assertion(&params.class, &params.individual)
                .map(text)
        })
        .await
    }
}

/// Remove a single OWL axiom (given in functional syntax) from the ontology file.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct RemoveAxiom {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Axiom in OWL functional syntax to remove
    pub axiom_str: String,
}

impl RemoveAxiom {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        with_api(manager, &params.owl_file_path, false, false, |api| {
            api.remove_axiom(&params.axiom_str).map(text)
        })
        .await
    }
}

/// Search axioms in an OWL file using a regex pattern. Returns matching axioms (up to limit).
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct FindAxioms {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Regex pattern to match against functional-syntax axiom strings
    pub pattern: String,
    /// Maximum number of results to return (default: 100)
    #[serde(default = "default_limit")]
    pub limit: u64,
    /// If true, append human-readable labels after ## comments
    #[serde(default)]
    pub include_labels: bool,
    /// IRI or CURIE of the annotation property to use for labels (default: rdfs:label)
    pub annotation_property: Option<String>,
}

impl FindAxioms {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        with_api(manager, &params.owl_file_path, false, false, |api| {
            api.find_axioms(
                &params.pattern,
                params.limit as usize,
                params.include_labels,
                params.annotation_property.as_deref(),
            )
        })
        .await
    }
}

/// Return all axioms in the OWL file (up to limit).
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct GetAllAxioms {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Maximum number of results to return (default: 100)
    #[serde(default = "default_limit")]
    pub limit: u64,
    /// If true, append human-readable labels after ## comments
    #[serde(default)]
    pub include_labels: bool,
    /// IRI or CURIE of the annotation property to use for labels (default: rdfs:label)
    pub annotation_property: Option<String>,
}

impl GetAllAxioms {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        with_api(manager, &params.owl_file_path, false, false, |api| {
            Ok(api.get_all_axioms(
                params.limit as usize,
                params.include_labels,
                params.annotation_property.as_deref(),
            ))
        })
        .await
    }
}

// ── Metadata operations ───────────────────────────────────────────────────────

/// Add a prefix mapping (e.g. prefix='ex:' uri='http://example.org/') to the ontology file.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct AddPrefix {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Prefix name including colon, e.g. 'ex:'
    pub prefix: String,
    /// The full IRI the prefix expands to, e.g. 'http://example.org/'
    pub uri: String,
}

impl AddPrefix {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        with_api(manager, &params.owl_file_path, false, true, |api| {
            api.add_prefix(&params.prefix, &params.uri).map(text)
        })
        .await
    }
}

/// Return the ontology-level annotation axioms (metadata header) for the given OWL file.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct OntologyMetadata {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
}

impl OntologyMetadata {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        with_api(manager, &params.owl_file_path, false, false, |api| {
            Ok(api.ontology_metadata())
        })
        .await
    }
}

/// Return all label values for a given IRI or CURIE in the ontology file. Defaults to rdfs:label.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct GetLabelsForIri {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Full IRI or CURIE (e.g. 'ex:Dog' or '<http://example.org/Dog>')
    pub iri: String,
    /// IRI or CURIE of the annotation property (default: rdfs:label)
    pub annotation_property: Option<String>,
}

impl GetLabelsForIri {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        with_api(manager, &params.owl_file_path, false, false, |api| {
            Ok(api.get_labels_for_iri(&params.iri, params.annotation_property.as_deref()))
        })
        .await
    }
}

/// Set or update the ontology IRI (and optional version IRI) for an OWL file.
/// Pass iri=null to clear the ontology IRI.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct SetOntologyIri {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// The ontology IRI to set (e.g. 'http://example.org/my-ontology')
    pub iri: Option<String>,
    /// Optional version IRI (e.g. 'http://example.org/my-ontology/1.0')
    pub version_iri: Option<String>,
}

impl SetOntologyIri {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        with_api(manager, &params.owl_file_path, false, true, |api| {
            api.set_ontology_iri(params.iri.as_deref(), params.version_iri.as_deref())
                .map(text)
        })
        .await
    }
}

/// Evaluate the quality of an OWL ontology using the OQuaRE framework (based on ISO/IEC 25000 SQuaRE).
/// Uses the whelk OWL EL reasoner for inferred class hierarchy.
/// Returns a JSON report with: basic metrics (class/property counts, hierarchy stats),
/// 19 raw + scaled metrics (ANOnto, AROnto, CBOOnto, CROnto, DITOnto, INROnto, LCOMOnto,
/// NACOnto, NOCOnto, NOMOnto, RFCOnto, RROnto, TMOnto, WMCOnto, plus variants),
/// 22 subcharacteristics, 7 quality characteristics (Structural, Functional Adequacy,
/// Maintainability, Operability, Reliability, Transferability, Compatibility),
/// and an overall OQuaRE score (1-5 scale, where 1=not acceptable, 3=minimally acceptable, 5=exceeds requirements).
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct TestQuality {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
}

impl TestQuality {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        with_api(manager, &params.owl_file_path, false, false, |api| {
            let report = crate::quality::evaluate(&api.ontology);
            let json = serde_json::to_string_pretty(&report)
                .map_err(|e| OwlApiError::Parse(e.to_string()))?;
            Ok(text(json))
        })
        .await
    }
}

/// Run a SPARQL query over one or more OWL files. Each file is serialized to RDF and
/// loaded together into an in-memory store (pass several paths to merge a schema with its ABox or
/// imports before querying), then the query is evaluated against the merged graph.
/// Returns SPARQL 1.1 JSON results for SELECT/ASK, and a list of N-Triples for CONSTRUCT/DESCRIBE.
/// By default queries run over asserted triples only. Set with_reasoning=true to materialize
/// OWL 2 EL entailments (via whelk) before querying so inferred subclass relationships are visible.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct SparqlQuery {
    /// Absolute paths to the OWL files to load and merge into one RDF graph before querying
    pub owl_file_paths: Vec<String>,
    /// The SPARQL query string (SELECT, ASK, CONSTRUCT, or DESCRIBE)
    pub query: String,
    /// When true, materialize OWL 2 EL inferred SubClassOf axioms (via whelk) before querying.
    /// Default false (asserted triples only).
    #[serde(default)]
    pub with_reasoning: bool,
}

impl SparqlQuery {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        if params.owl_file_paths.is_empty() {
            return Err(OwlApiError::Parse(
                "owl_file_paths must contain at least one path".to_string(),
            ));
        }

        let json = if params.with_reasoning {
            let mut ontologies = Vec::with_capacity(params.owl_file_paths.len());
            for path in &params.owl_file_paths {
                let handle = manager.get_or_load(path, false, false).await?;
                let api = handle.lock().await;
                ontologies.push(api.ontology.clone());
            }

            let refs: Vec<_> = ontologies.iter().collect();
            let merged = crate::reasoning::merge_ontologies(&refs);
            let reasoned = crate::reasoning::reason_and_materialize(&merged);
            let bytes = crate::ontology::owl_api::ontology_to_rdf_bytes(&reasoned)?;
            crate::sparql::query(&[bytes], &params.query)?
        } else {
            let mut rdf_docs: Vec<Vec<u8>> = Vec::with_capacity(params.owl_file_paths.len());
            for path in &params.owl_file_paths {
                let handle = manager.get_or_load(path, false, false).await?;
                let api = handle.lock().await;
                rdf_docs.push(api.to_rdf_bytes()?);
            }
            crate::sparql::query(&rdf_docs, &params.query)?
        };

        Ok(text(json))
    }
}

/// Run an OWL 2 EL reasoner (whelk; alias elk) over one or more OWL files and report
/// logical consistency. Multiple paths are loaded and merged (same semantics as sparql_query), so a
/// schema + ABox can be reasoned together. Returns JSON with: consistent (bool),
/// unsatisfiable_classes (IRIs equivalent to owl:Nothing), optional inferred_axioms_count, and
/// reasoner. Optionally write a materialized ontology (asserted + inferred SubClassOf axioms) to
/// output_path. IMPORTANT: reasoning is limited to the OWL 2 EL profile — full OWL 2 DL
/// inconsistency (cardinality restrictions, complex disjointness outside EL, etc.) is NOT detected.
/// This matches robot reason --reasoner ELK.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct CheckConsistency {
    /// Absolute paths to the OWL files to load and merge before reasoning
    pub owl_file_paths: Vec<String>,
    /// Reasoner id: "whelk" (default) or "elk" (synonym). Only OWL 2 EL is supported.
    pub reasoner: Option<String>,
    /// If set, write the reasoned ontology (asserted + inferred SubClassOf) to this path
    pub output_path: Option<String>,
}

impl CheckConsistency {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        if params.owl_file_paths.is_empty() {
            return Err(OwlApiError::Parse(
                "owl_file_paths must contain at least one path".to_string(),
            ));
        }

        crate::reasoning::normalize_reasoner_id(params.reasoner.as_deref())
            .map_err(OwlApiError::Parse)?;

        let mut ontologies = Vec::with_capacity(params.owl_file_paths.len());
        let mut prefixes = horned_owl::curie::PrefixMapping::default();
        for (i, path) in params.owl_file_paths.iter().enumerate() {
            let handle = manager.get_or_load(path, false, false).await?;
            let api = handle.lock().await;
            if i == 0 {
                prefixes = api.prefixes.clone();
            }
            ontologies.push(api.ontology.clone());
        }

        let refs: Vec<_> = ontologies.iter().collect();
        let merged = crate::reasoning::merge_ontologies(&refs);

        let report = if let Some(ref out) = params.output_path {
            let (report, _) = crate::reasoning::check_and_maybe_write(
                &merged,
                &prefixes,
                Some(std::path::Path::new(out)),
                true,
            )?;
            report
        } else {
            crate::reasoning::check(&merged, false)
        };

        let json =
            serde_json::to_string_pretty(&report).map_err(|e| OwlApiError::Parse(e.to_string()))?;
        Ok(text(json))
    }
}

/// Scan an OWL ontology for common modeling pitfalls (inspired by OOPS! - OntOlogy Pitfall Scanner).
/// Returns a JSON report listing detected issues, their severity, and affected elements.
/// 31 checks: P02 (synonym classes), P03 ("is" relationship), P04 (unconnected elements),
/// P05 (wrong inverses), P06 (class hierarchy cycles), P07 (merged concepts),
/// P08 (missing annotations), P10 (missing disjointness), P11 (missing domain/range),
/// P12 (undeclared equivalent properties), P13 (missing inverses, with sub-variants Y/N/S),
/// P19 (multiple domains/ranges), P20 (misused annotations), P21 (miscellaneous class),
/// P22 (inconsistent naming), P24 (recursive definitions), P25 (self-inverse),
/// P26 (inverse of symmetric), P27 (wrong equivalent properties), P28 (wrong symmetric),
/// P29 (wrong transitive), P30 (undeclared equivalent classes), P31 (wrong equivalent classes),
/// P32 (duplicate labels), P33 (single-property chain), P34 (untyped class),
/// P35 (untyped property), P36 (URI file extension), P38 (no ontology declaration),
/// P39 (ambiguous namespace), P41 (no license).
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct TestPitfalls {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Comma-separated list of pitfall IDs to check (e.g. "P04,P08,P11"). If omitted, all checks run.
    pub pitfalls: Option<String>,
}

impl TestPitfalls {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        with_api(manager, &params.owl_file_path, false, false, |api| {
            let filter = params.pitfalls.as_ref().map(|s| {
                s.split(',')
                    .map(|p| p.trim().to_uppercase())
                    .collect::<HashSet<_>>()
            });

            let report = crate::pitfalls::scan(&api.ontology, filter.as_ref());
            let json = serde_json::to_string_pretty(&report)
                .map_err(|e| OwlApiError::Parse(e.to_string()))?;
            Ok(text(json))
        })
        .await
    }
}

pub fn default_limit() -> u64 {
    100
}

/// Convert OWL axioms into Controlled Natural Language (pseudo-text) plus a Turtle fragment.
/// If `iri` is omitted, verbalizes owl:Class and owl:NamedIndividual entities (up to limit).
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct Verbalize {
    /// Absolute path to the OWL file
    pub owl_file_path: String,
    /// Optional IRI or CURIE of a single class or individual to verbalize
    pub iri: Option<String>,
    /// Maximum number of entities to verbalize when `iri` is omitted (default: 100)
    #[serde(default = "default_limit")]
    pub limit: u64,
}

impl Verbalize {
    pub async fn run(params: Self, manager: &Manager) -> Result<Vec<String>, OwlApiError> {
        let handle = manager
            .get_or_load(&params.owl_file_path, false, false)
            .await?;
        let api = handle.lock().await;
        let rdf = api.to_rdf_bytes()?;
        let starting = params.iri.as_ref().map(|iri| vec![api.expand_curie(iri)]);
        drop(api);

        let entries =
            crate::verbalizer::verbalize(&rdf, starting.as_deref(), params.limit as usize)?;
        let json = serde_json::to_string_pretty(&entries)
            .map_err(|e| OwlApiError::Parse(e.to_string()))?;
        Ok(text(json))
    }
}

#[cfg(test)]
mod verbalize_tests {
    use super::*;
    use crate::verbalizer::VerbalizeEntry;

    const FIXTURE: &str = include_str!("../tests/fixtures/pizza.ofn");
    const GOLDEN_VEGETARIAN: &str =
        include_str!("../tests/fixtures/golden/vegetarian_pizza.cnl");
    const GOLDEN_MARGHERITA: &str = include_str!("../tests/fixtures/golden/margherita.cnl");

    #[tokio::test]
    async fn verbalize_tool_with_iri_matches_golden() {
        let tmp = tempfile::NamedTempFile::with_suffix(".ofn").unwrap();
        std::fs::write(tmp.path(), FIXTURE).unwrap();
        let manager = Arc::new(OntologyManager::new());
        let lines = Verbalize::run(
            Verbalize {
                owl_file_path: tmp.path().to_string_lossy().into_owned(),
                iri: Some("http://example.org/pizza#VegetarianPizza".into()),
                limit: 10,
            },
            &manager,
        )
        .await
        .unwrap();
        let entries: Vec<VerbalizeEntry> = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text, GOLDEN_VEGETARIAN.trim_end());
    }

    #[tokio::test]
    async fn verbalize_tool_without_iri_includes_margherita_golden() {
        let tmp = tempfile::NamedTempFile::with_suffix(".ofn").unwrap();
        std::fs::write(tmp.path(), FIXTURE).unwrap();
        let manager = Arc::new(OntologyManager::new());
        let lines = Verbalize::run(
            Verbalize {
                owl_file_path: tmp.path().to_string_lossy().into_owned(),
                iri: None,
                limit: 20,
            },
            &manager,
        )
        .await
        .unwrap();
        let entries: Vec<VerbalizeEntry> = serde_json::from_str(&lines[0]).unwrap();
        let marg = entries
            .iter()
            .find(|e| e.root.contains("Margherita"))
            .expect("Margherita entry");
        assert_eq!(marg.text, GOLDEN_MARGHERITA.trim_end());
    }
}
