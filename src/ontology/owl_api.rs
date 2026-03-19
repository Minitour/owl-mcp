use std::io::{BufReader, BufWriter, Cursor};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use horned_owl::curie::PrefixMapping;
use horned_owl::io::ofn::reader::read_with_build as ofn_read;
use horned_owl::io::ofn::writer::{write as ofn_write, AsFunctional};
use horned_owl::io::rdf::reader::read_with_build as rdf_read;
use horned_owl::io::rdf::writer::write as rdf_write;
use horned_owl::io::ParserConfiguration;
use horned_owl::model::{
    AnnotatedComponent, AnnotationAssertion, AnnotationSubject, AnnotationValue, ArcStr, Build,
    Component, Literal, MutableOntology, OntologyID,
};
use horned_owl::ontology::component_mapped::ComponentMappedOntology;
use horned_owl::ontology::set::SetOntology;
use regex::Regex;
use thiserror::Error;

const RDFS_LABEL: &str = "http://www.w3.org/2000/01/rdf-schema#label";

#[derive(Debug, Error)]
pub enum OwlApiError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("OWL parse error: {0}")]
    Parse(String),
    #[error("Ontology is read-only")]
    ReadOnly,
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
}

impl From<horned_owl::error::HornedError> for OwlApiError {
    fn from(e: horned_owl::error::HornedError) -> Self {
        OwlApiError::Parse(e.to_string())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OntologyFormat {
    Ofn,
    Rdf,
}

fn detect_format(path: &Path, content: &[u8]) -> OntologyFormat {
    match path.extension().and_then(|e| e.to_str()) {
        Some("ofn") => OntologyFormat::Ofn,
        Some("owx") => OntologyFormat::Ofn,
        Some("owl") | Some("rdf") => {
            let start = std::str::from_utf8(&content[..content.len().min(200)]).unwrap_or("");
            if start.trim_start().starts_with("<?xml") || start.contains("<rdf:RDF") {
                OntologyFormat::Rdf
            } else {
                OntologyFormat::Ofn
            }
        }
        _ => OntologyFormat::Ofn,
    }
}

pub struct OwlApi {
    pub path: PathBuf,
    pub ontology: SetOntology<ArcStr>,
    pub prefixes: PrefixMapping,
    pub build: Build<ArcStr>,
    pub readonly: bool,
    pub format: OntologyFormat,
    pub last_modified: Option<SystemTime>,
}

impl OwlApi {
    pub fn load(
        path: impl AsRef<Path>,
        readonly: bool,
        create_if_not_exists: bool,
    ) -> Result<Self, OwlApiError> {
        let path = path.as_ref().to_path_buf();
        let build = Build::new_arc();

        if !path.exists() {
            if create_if_not_exists {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let empty = "Ontology()\n";
                std::fs::write(&path, empty)?;
            } else {
                return Err(OwlApiError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("File not found: {}", path.display()),
                )));
            }
        }

        let content = std::fs::read(&path)?;
        let last_modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        let format = detect_format(&path, &content);
        let (ontology, prefixes) = parse_bytes(&content, &build, format)?;

        Ok(OwlApi {
            path,
            ontology,
            prefixes,
            build,
            readonly,
            format,
            last_modified,
        })
    }

    pub fn reload(&mut self) -> Result<(), OwlApiError> {
        let content = std::fs::read(&self.path)?;
        self.last_modified = std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .ok();
        let format = detect_format(&self.path, &content);
        let (ontology, prefixes) = parse_bytes(&content, &self.build, format)?;
        self.ontology = ontology;
        self.prefixes = prefixes;
        self.format = format;
        Ok(())
    }

    /// Check if the file was modified since last load and reload if so.
    pub fn check_and_reload_if_modified(&mut self) -> Result<bool, OwlApiError> {
        let current_mtime = std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .ok();
        if current_mtime != self.last_modified {
            self.reload()?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn add_axiom(&mut self, axiom_str: &str) -> Result<String, OwlApiError> {
        if self.readonly {
            return Err(OwlApiError::ReadOnly);
        }
        let components = self.parse_axiom_string(axiom_str)?;
        if components.is_empty() {
            return Ok(format!("Warning: no axiom parsed from: {}", axiom_str));
        }
        let count = components.len();
        for ac in components {
            self.ontology.insert(ac);
        }
        self.save()?;
        Ok(format!("Successfully added {} axiom(s)", count))
    }

    pub fn add_axioms(&mut self, axiom_strs: &[String]) -> Result<String, OwlApiError> {
        if self.readonly {
            return Err(OwlApiError::ReadOnly);
        }
        let mut parsed = Vec::new();
        for axiom_str in axiom_strs {
            let components = self.parse_axiom_string(axiom_str)?;
            parsed.extend(components);
        }
        let count = parsed.len();
        for ac in parsed {
            self.ontology.insert(ac);
        }
        self.save()?;
        Ok(format!("Successfully added {} axiom(s)", count))
    }

    pub fn remove_axiom(&mut self, axiom_str: &str) -> Result<String, OwlApiError> {
        if self.readonly {
            return Err(OwlApiError::ReadOnly);
        }
        let components = self.parse_axiom_string(axiom_str)?;
        let mut removed = 0usize;
        for ac in &components {
            if self.ontology.remove(ac) {
                removed += 1;
            }
        }
        if removed == 0 {
            return Ok(format!(
                "Warning: axiom not found in ontology: {}",
                axiom_str
            ));
        }
        self.save()?;
        Ok(format!("Successfully removed {} axiom(s)", removed))
    }

    pub fn find_axioms(
        &self,
        pattern: &str,
        limit: usize,
        include_labels: bool,
        annotation_property: Option<&str>,
    ) -> Result<Vec<String>, OwlApiError> {
        let re = Regex::new(pattern)?;
        let results = self
            .ontology
            .iter()
            .filter(|ac| {
                !matches!(
                    ac.component,
                    Component::OntologyID(_) | Component::OntologyAnnotation(_)
                )
            })
            .filter_map(|ac| {
                let s = self.component_to_string(ac);
                if re.is_match(&s) {
                    Some(if include_labels {
                        self.annotate_with_labels(&s, annotation_property)
                    } else {
                        s
                    })
                } else {
                    None
                }
            })
            .take(limit)
            .collect();
        Ok(results)
    }

    pub fn get_all_axioms(
        &self,
        limit: usize,
        include_labels: bool,
        annotation_property: Option<&str>,
    ) -> Vec<String> {
        self.ontology
            .iter()
            .filter(|ac| {
                !matches!(
                    ac.component,
                    Component::OntologyID(_) | Component::OntologyAnnotation(_)
                )
            })
            .map(|ac| {
                let s = self.component_to_string(ac);
                if include_labels {
                    self.annotate_with_labels(&s, annotation_property)
                } else {
                    s
                }
            })
            .take(limit)
            .collect()
    }

    pub fn ontology_metadata(&self) -> Vec<String> {
        self.ontology
            .iter()
            .filter(|ac| {
                matches!(
                    ac.component,
                    Component::OntologyAnnotation(_) | Component::OntologyID(_)
                )
            })
            .map(|ac| self.component_to_string(ac))
            .collect()
    }

    pub fn add_prefix(&mut self, prefix: &str, uri: &str) -> Result<String, OwlApiError> {
        if self.readonly {
            return Err(OwlApiError::ReadOnly);
        }
        // curie::PrefixMapping stores prefix names without trailing colon
        let prefix_name = prefix.trim_end_matches(':');
        if prefix_name.is_empty() {
            self.prefixes.set_default(uri);
        } else {
            self.prefixes
                .add_prefix(prefix_name, uri)
                .map_err(|e| OwlApiError::Parse(format!("Failed to add prefix: {:?}", e)))?;
        }
        self.save()?;
        Ok(format!("Added prefix {}=<{}>", prefix, uri))
    }

    pub fn set_ontology_iri(
        &mut self,
        iri: Option<&str>,
        version_iri: Option<&str>,
    ) -> Result<String, OwlApiError> {
        if self.readonly {
            return Err(OwlApiError::ReadOnly);
        }

        let old_ids: Vec<AnnotatedComponent<ArcStr>> = self
            .ontology
            .iter()
            .filter(|ac| matches!(ac.component, Component::OntologyID(_)))
            .cloned()
            .collect();
        for ac in &old_ids {
            self.ontology.remove(ac);
        }

        let new_iri = iri.map(|s| self.build.iri(self.expand_curie(s)));
        let new_viri = version_iri.map(|s| self.build.iri(self.expand_curie(s)));
        let oid = OntologyID {
            iri: new_iri,
            viri: new_viri,
        };
        self.ontology
            .insert(AnnotatedComponent::from(Component::OntologyID(oid)));
        self.save()?;

        let mut parts = Vec::new();
        if let Some(i) = iri {
            parts.push(format!("IRI set to <{}>", i));
        }
        if let Some(v) = version_iri {
            parts.push(format!("version IRI set to <{}>", v));
        }
        if parts.is_empty() {
            Ok("Ontology IRI cleared".to_string())
        } else {
            Ok(parts.join(", "))
        }
    }

    pub fn get_labels_for_iri(&self, iri: &str, annotation_property: Option<&str>) -> Vec<String> {
        let label_prop_iri = self.resolve_ann_prop_iri(annotation_property);
        let subject_iri = self.expand_curie(iri);

        self.ontology
            .iter()
            .filter_map(|ac| {
                if let Component::AnnotationAssertion(AnnotationAssertion {
                    subject: AnnotationSubject::IRI(s_iri),
                    ann,
                }) = &ac.component
                {
                    let s_str: &str = s_iri.as_ref();
                    let ann_prop_str: &str = ann.ap.0.as_ref();
                    if (s_str == subject_iri.as_str() || s_str == iri)
                        && ann_prop_str == label_prop_iri.as_str()
                    {
                        return Some(annotation_value_to_string(&ann.av));
                    }
                }
                None
            })
            .collect()
    }

    pub fn save(&self) -> Result<(), OwlApiError> {
        let cmo: ComponentMappedOntology<ArcStr, Arc<AnnotatedComponent<ArcStr>>> =
            self.ontology.clone().into();
        match self.format {
            OntologyFormat::Ofn => {
                let buf = ofn_write(Vec::new(), &cmo, Some(&self.prefixes))?;
                std::fs::write(&self.path, buf)?;
            }
            OntologyFormat::Rdf => {
                let file = std::fs::File::create(&self.path)?;
                let writer = BufWriter::new(file);
                rdf_write(writer, &cmo)?;
            }
        }
        Ok(())
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn parse_axiom_string(
        &self,
        axiom_str: &str,
    ) -> Result<Vec<AnnotatedComponent<ArcStr>>, OwlApiError> {
        let wrapper = self.build_wrapper_doc(axiom_str);
        let cursor = Cursor::new(wrapper.as_bytes());
        let reader = BufReader::new(cursor);
        let (temp_onto, _): (SetOntology<ArcStr>, _) = ofn_read(reader, &self.build)?;
        // Filter out the wrapper document's own OntologyID / OntologyAnnotation —
        // the caller supplied only the inner axiom, not the surrounding Ontology(...).
        Ok(temp_onto
            .into_iter()
            .filter(|ac| {
                !matches!(
                    ac.component,
                    Component::OntologyID(_) | Component::OntologyAnnotation(_)
                )
            })
            .collect())
    }

    fn build_wrapper_doc(&self, axiom_str: &str) -> String {
        let mut doc = String::new();
        // Standard prefixes first (so they can be overridden by user prefixes)
        doc.push_str("Prefix(owl:=<http://www.w3.org/2002/07/owl#>)\n");
        doc.push_str("Prefix(rdf:=<http://www.w3.org/1999/02/22-rdf-syntax-ns#>)\n");
        doc.push_str("Prefix(rdfs:=<http://www.w3.org/2000/01/rdf-schema#>)\n");
        doc.push_str("Prefix(xsd:=<http://www.w3.org/2001/XMLSchema#>)\n");
        // User-defined prefixes from current ontology
        for (prefix, iri) in self.prefixes.mappings() {
            // Prefix names stored without colon in curie; OFN format needs the colon
            if prefix.is_empty() {
                doc.push_str(&format!("Prefix(:=<{}>)\n", iri));
            } else {
                doc.push_str(&format!("Prefix({}:=<{}>)\n", prefix, iri));
            }
        }
        doc.push_str("Ontology(\n");
        doc.push_str(axiom_str);
        doc.push('\n');
        doc.push_str(")\n");
        doc
    }

    fn component_to_string(&self, ac: &AnnotatedComponent<ArcStr>) -> String {
        ac.component
            .as_functional_with_prefixes(&self.prefixes)
            .to_string()
    }

    fn annotate_with_labels(&self, axiom_str: &str, annotation_property: Option<&str>) -> String {
        let label_prop_iri = self.resolve_ann_prop_iri(annotation_property);
        let iri_re = Regex::new(r"<([^>]+)>").unwrap();
        let mut labels_found = Vec::new();
        for cap in iri_re.captures_iter(axiom_str) {
            let iri = &cap[1];
            let labels = self.get_labels_for_iri_raw(iri, &label_prop_iri);
            if !labels.is_empty() {
                labels_found.push(format!("<{}> # {}", iri, labels.join(", ")));
            }
        }
        if labels_found.is_empty() {
            axiom_str.to_string()
        } else {
            format!("{} ## {}", axiom_str, labels_found.join("; "))
        }
    }

    fn get_labels_for_iri_raw(&self, iri: &str, label_prop_iri: &str) -> Vec<String> {
        self.ontology
            .iter()
            .filter_map(|ac| {
                if let Component::AnnotationAssertion(AnnotationAssertion {
                    subject: AnnotationSubject::IRI(s_iri),
                    ann,
                }) = &ac.component
                {
                    let s_str: &str = s_iri.as_ref();
                    let ann_prop_str: &str = ann.ap.0.as_ref();
                    if s_str == iri && ann_prop_str == label_prop_iri {
                        return Some(annotation_value_to_string(&ann.av));
                    }
                }
                None
            })
            .collect()
    }

    fn resolve_ann_prop_iri(&self, ann_prop: Option<&str>) -> String {
        match ann_prop {
            None => RDFS_LABEL.to_string(),
            Some(s) => self.expand_curie(s),
        }
    }

    fn expand_curie(&self, curie: &str) -> String {
        if curie.starts_with('<') && curie.ends_with('>') {
            return curie[1..curie.len() - 1].to_string();
        }
        // Try curie::PrefixMapping expansion
        match self.prefixes.expand_curie_string(curie) {
            Ok(expanded) => expanded,
            Err(_) => curie.to_string(),
        }
    }
}

fn parse_bytes(
    content: &[u8],
    build: &Build<ArcStr>,
    format: OntologyFormat,
) -> Result<(SetOntology<ArcStr>, PrefixMapping), OwlApiError> {
    match format {
        OntologyFormat::Ofn => {
            let cursor = Cursor::new(content);
            let reader = BufReader::new(cursor);
            let (onto, pm): (SetOntology<ArcStr>, _) = ofn_read(reader, build)?;
            Ok((onto, pm))
        }
        OntologyFormat::Rdf => {
            let cursor = Cursor::new(content);
            let mut reader = BufReader::new(cursor);
            let (rdf_onto, _incomplete): (
                horned_owl::io::rdf::reader::ConcreteRDFOntology<
                    ArcStr,
                    Arc<AnnotatedComponent<ArcStr>>,
                >,
                _,
            ) = rdf_read(&mut reader, build, ParserConfiguration::default())?;
            let onto: SetOntology<ArcStr> = rdf_onto.into();
            Ok((onto, PrefixMapping::default()))
        }
    }
}

fn annotation_value_to_string(av: &AnnotationValue<ArcStr>) -> String {
    match av {
        AnnotationValue::Literal(lit) => match lit {
            Literal::Simple { literal } => literal.clone(),
            Literal::Language { literal, lang } => {
                format!("{}@{}", literal, lang)
            }
            Literal::Datatype { literal, .. } => literal.clone(),
        },
        AnnotationValue::IRI(iri) => {
            let s: &str = iri.as_ref();
            s.to_string()
        }
        AnnotationValue::AnonymousIndividual(ai) => {
            let s: &str = ai.0.as_ref();
            s.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    /// Write a minimal OFN file to a NamedTempFile and return it.
    fn empty_ofn() -> NamedTempFile {
        // NamedTempFile keeps the file alive as long as the handle exists.
        // We write through the handle then return it.
        let f = NamedTempFile::with_suffix(".ofn").unwrap();
        fs::write(f.path(), "Ontology()\n").unwrap();
        f
    }

    fn ofn_with_content(content: &str) -> NamedTempFile {
        let f = NamedTempFile::with_suffix(".ofn").unwrap();
        fs::write(f.path(), content).unwrap();
        f
    }

    // ── detect_format ────────────────────────────────────────────────────────

    #[test]
    fn detect_format_by_extension_ofn() {
        use std::path::Path;
        let content = b"Ontology()";
        assert_eq!(
            detect_format(Path::new("x.ofn"), content),
            OntologyFormat::Ofn
        );
    }

    #[test]
    fn detect_format_by_extension_owl_xml_sniff() {
        use std::path::Path;
        let content = b"<?xml version=\"1.0\"?><rdf:RDF>";
        assert_eq!(
            detect_format(Path::new("x.owl"), content),
            OntologyFormat::Rdf
        );
    }

    #[test]
    fn detect_format_owl_extension_ofn_content() {
        use std::path::Path;
        let content = b"Ontology()";
        assert_eq!(
            detect_format(Path::new("x.owl"), content),
            OntologyFormat::Ofn
        );
    }

    // ── load ─────────────────────────────────────────────────────────────────

    #[test]
    fn load_creates_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new.ofn");
        assert!(!path.exists());
        let api = OwlApi::load(&path, false, true).unwrap();
        assert!(path.exists());
        assert_eq!(api.format, OntologyFormat::Ofn);
    }

    #[test]
    fn load_returns_error_when_missing_and_no_create() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.ofn");
        let result = OwlApi::load(&path, false, false);
        assert!(result.is_err());
    }

    #[test]
    fn load_existing_file() {
        let f = empty_ofn();
        let api = OwlApi::load(f.path(), false, false).unwrap();
        assert_eq!(api.format, OntologyFormat::Ofn);
        assert_eq!(api.get_all_axioms(100, false, None).len(), 0);
    }

    // ── add_axiom ────────────────────────────────────────────────────────────

    #[test]
    fn add_axiom_class_declaration() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        let msg = api
            .add_axiom("Declaration(Class(<http://example.org/Dog>))")
            .unwrap();
        assert!(msg.contains("Successfully"));
        let axioms = api.get_all_axioms(100, false, None);
        assert_eq!(axioms.len(), 1);
        assert!(axioms[0].contains("Dog"));
    }

    #[test]
    fn add_axiom_subclass_of() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        api.add_axiom("Declaration(Class(<http://example.org/Animal>))")
            .unwrap();
        api.add_axiom("Declaration(Class(<http://example.org/Dog>))")
            .unwrap();
        api.add_axiom("SubClassOf(<http://example.org/Dog> <http://example.org/Animal>)")
            .unwrap();
        let axioms = api.get_all_axioms(100, false, None);
        assert_eq!(axioms.len(), 3);
        assert!(axioms.iter().any(|s| s.contains("SubClassOf")));
    }

    #[test]
    fn add_axiom_readonly_returns_error() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), true, false).unwrap();
        let result = api.add_axiom("Declaration(Class(<http://example.org/Dog>))");
        assert!(matches!(result, Err(OwlApiError::ReadOnly)));
    }

    // ── add_axioms ───────────────────────────────────────────────────────────

    #[test]
    fn add_axioms_batch() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        let strs = vec![
            "Declaration(Class(<http://example.org/A>))".to_string(),
            "Declaration(Class(<http://example.org/B>))".to_string(),
            "Declaration(Class(<http://example.org/C>))".to_string(),
        ];
        let msg = api.add_axioms(&strs).unwrap();
        assert!(msg.contains("3"));
        assert_eq!(api.get_all_axioms(100, false, None).len(), 3);
    }

    // ── remove_axiom ─────────────────────────────────────────────────────────

    #[test]
    fn remove_axiom_existing() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        api.add_axiom("Declaration(Class(<http://example.org/Dog>))")
            .unwrap();
        assert_eq!(api.get_all_axioms(100, false, None).len(), 1);
        let msg = api
            .remove_axiom("Declaration(Class(<http://example.org/Dog>))")
            .unwrap();
        assert!(msg.contains("Successfully removed"));
        assert_eq!(api.get_all_axioms(100, false, None).len(), 0);
    }

    #[test]
    fn remove_axiom_not_found_returns_warning() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        let msg = api
            .remove_axiom("Declaration(Class(<http://example.org/Ghost>))")
            .unwrap();
        assert!(msg.contains("Warning"));
    }

    #[test]
    fn remove_axiom_readonly_returns_error() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), true, false).unwrap();
        let result = api.remove_axiom("Declaration(Class(<http://example.org/Dog>))");
        assert!(matches!(result, Err(OwlApiError::ReadOnly)));
    }

    // ── find_axioms ──────────────────────────────────────────────────────────

    #[test]
    fn find_axioms_by_regex() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        api.add_axiom("Declaration(Class(<http://example.org/Dog>))")
            .unwrap();
        api.add_axiom("Declaration(Class(<http://example.org/Cat>))")
            .unwrap();
        api.add_axiom("SubClassOf(<http://example.org/Dog> <http://example.org/Animal>)")
            .unwrap();

        let results = api.find_axioms("SubClassOf", 100, false, None).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].contains("SubClassOf"));
    }

    #[test]
    fn find_axioms_respects_limit() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        for i in 0..10 {
            api.add_axiom(&format!("Declaration(Class(<http://example.org/C{}>))", i))
                .unwrap();
        }
        let results = api.find_axioms("Declaration", 3, false, None).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn find_axioms_invalid_regex_returns_error() {
        let f = empty_ofn();
        let api = OwlApi::load(f.path(), false, false).unwrap();
        let result = api.find_axioms("[invalid(regex", 100, false, None);
        assert!(matches!(result, Err(OwlApiError::Regex(_))));
    }

    // ── get_all_axioms ───────────────────────────────────────────────────────

    #[test]
    fn get_all_axioms_empty_ontology() {
        let f = empty_ofn();
        let api = OwlApi::load(f.path(), false, false).unwrap();
        assert!(api.get_all_axioms(100, false, None).is_empty());
    }

    #[test]
    fn get_all_axioms_limit() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        for i in 0..5 {
            api.add_axiom(&format!("Declaration(Class(<http://example.org/X{}>))", i))
                .unwrap();
        }
        assert_eq!(api.get_all_axioms(2, false, None).len(), 2);
        assert_eq!(api.get_all_axioms(100, false, None).len(), 5);
    }

    // ── add_prefix ───────────────────────────────────────────────────────────

    #[test]
    fn add_prefix_and_use_in_axiom() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        api.add_prefix("ex:", "http://example.org/").unwrap();
        // The ontology now has an "ex" prefix; axioms should round-trip with it
        api.add_axiom("Declaration(Class(<http://example.org/Dog>))")
            .unwrap();
        let axioms = api.get_all_axioms(100, false, None);
        assert!(!axioms.is_empty());
    }

    #[test]
    fn add_prefix_strips_trailing_colon() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        // Both "ex:" and "ex" should be accepted
        api.add_prefix("ex:", "http://example.org/").unwrap();
        api.add_prefix("ex2", "http://example2.org/").unwrap();
    }

    // ── get_labels_for_iri ───────────────────────────────────────────────────

    #[test]
    fn get_labels_for_iri_returns_labels() {
        let content = r#"Prefix(rdfs:=<http://www.w3.org/2000/01/rdf-schema#>)
Prefix(xsd:=<http://www.w3.org/2001/XMLSchema#>)
Ontology(
  Declaration(Class(<http://example.org/Dog>))
  AnnotationAssertion(rdfs:label <http://example.org/Dog> "dog")
)
"#;
        let f = ofn_with_content(content);
        let api = OwlApi::load(f.path(), false, false).unwrap();
        let labels = api.get_labels_for_iri("http://example.org/Dog", None);
        assert_eq!(labels, vec!["dog"]);
    }

    #[test]
    fn get_labels_for_iri_no_labels_returns_empty() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        api.add_axiom("Declaration(Class(<http://example.org/Dog>))")
            .unwrap();
        let labels = api.get_labels_for_iri("http://example.org/Dog", None);
        assert!(labels.is_empty());
    }

    // ── persistence (save + reload) ──────────────────────────────────────────

    #[test]
    fn axioms_persist_across_reload() {
        let f = empty_ofn();
        {
            let mut api = OwlApi::load(f.path(), false, false).unwrap();
            api.add_axiom("Declaration(Class(<http://example.org/Dog>))")
                .unwrap();
            api.add_axiom("SubClassOf(<http://example.org/Dog> <http://example.org/Animal>)")
                .unwrap();
        }
        // Load fresh from disk
        let api2 = OwlApi::load(f.path(), false, false).unwrap();
        let axioms = api2.get_all_axioms(100, false, None);
        assert_eq!(axioms.len(), 2);
        assert!(axioms.iter().any(|s| s.contains("SubClassOf")));
    }

    #[test]
    fn remove_persists_across_reload() {
        let f = empty_ofn();
        {
            let mut api = OwlApi::load(f.path(), false, false).unwrap();
            api.add_axiom("Declaration(Class(<http://example.org/Dog>))")
                .unwrap();
            api.add_axiom("Declaration(Class(<http://example.org/Cat>))")
                .unwrap();
            api.remove_axiom("Declaration(Class(<http://example.org/Cat>))")
                .unwrap();
        }
        let api2 = OwlApi::load(f.path(), false, false).unwrap();
        let axioms = api2.get_all_axioms(100, false, None);
        assert_eq!(axioms.len(), 1);
        assert!(!axioms.iter().any(|s| s.contains("Cat")));
    }

    // ── include_labels annotation ─────────────────────────────────────────────

    #[test]
    fn get_all_axioms_include_labels_appends_comment() {
        let content = r#"Prefix(rdfs:=<http://www.w3.org/2000/01/rdf-schema#>)
Prefix(xsd:=<http://www.w3.org/2001/XMLSchema#>)
Ontology(
  Declaration(Class(<http://example.org/Dog>))
  AnnotationAssertion(rdfs:label <http://example.org/Dog> "dog")
)
"#;
        let f = ofn_with_content(content);
        let api = OwlApi::load(f.path(), false, false).unwrap();
        let axioms = api.get_all_axioms(100, true, None);
        // At least the SubClassOf-like axiom should have a label comment
        let has_label_comment = axioms.iter().any(|s| s.contains("##") || s.contains("dog"));
        assert!(has_label_comment);
    }

    // ── set_ontology_iri ──────────────────────────────────────────────────────

    #[test]
    fn set_ontology_iri_on_empty_ontology() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        let msg = api
            .set_ontology_iri(Some("http://example.org/my-onto"), None)
            .unwrap();
        assert!(msg.contains("IRI set to"));
        let meta = api.ontology_metadata();
        let joined = meta.join(" ");
        assert!(joined.contains("http://example.org/my-onto"));
    }

    #[test]
    fn set_ontology_iri_with_version() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        let msg = api
            .set_ontology_iri(
                Some("http://example.org/onto"),
                Some("http://example.org/onto/1.0"),
            )
            .unwrap();
        assert!(msg.contains("IRI set to"));
        assert!(msg.contains("version IRI set to"));
        let meta = api.ontology_metadata();
        let joined = meta.join(" ");
        assert!(joined.contains("http://example.org/onto"));
        assert!(joined.contains("http://example.org/onto/1.0"));
    }

    #[test]
    fn set_ontology_iri_replaces_existing() {
        let content = "Ontology(<http://old.example.org/onto>)\n";
        let f = ofn_with_content(content);
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        let meta_before = api.ontology_metadata();
        assert!(meta_before.iter().any(|s| s.contains("old.example.org")));

        api.set_ontology_iri(Some("http://new.example.org/onto"), None)
            .unwrap();
        let meta_after = api.ontology_metadata();
        assert!(!meta_after.iter().any(|s| s.contains("old.example.org")));
        assert!(meta_after.iter().any(|s| s.contains("new.example.org")));
    }

    #[test]
    fn set_ontology_iri_persists_across_reload() {
        let f = empty_ofn();
        {
            let mut api = OwlApi::load(f.path(), false, false).unwrap();
            api.set_ontology_iri(Some("http://example.org/persisted"), None)
                .unwrap();
        }
        let api2 = OwlApi::load(f.path(), false, false).unwrap();
        let meta = api2.ontology_metadata();
        assert!(meta
            .iter()
            .any(|s| s.contains("http://example.org/persisted")));
    }

    #[test]
    fn set_ontology_iri_clear() {
        let content = "Ontology(<http://example.org/will-clear>)\n";
        let f = ofn_with_content(content);
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        let msg = api.set_ontology_iri(None, None).unwrap();
        assert!(msg.contains("cleared"));
        let meta = api.ontology_metadata();
        assert!(!meta.iter().any(|s| s.contains("will-clear")));
    }

    #[test]
    fn set_ontology_iri_readonly_returns_error() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), true, false).unwrap();
        let result = api.set_ontology_iri(Some("http://example.org/x"), None);
        assert!(matches!(result, Err(OwlApiError::ReadOnly)));
    }

    // ── expand_curie ─────────────────────────────────────────────────────────

    #[test]
    fn expand_curie_full_iri_passthrough() {
        let f = empty_ofn();
        let api = OwlApi::load(f.path(), false, false).unwrap();
        let expanded = api.expand_curie("<http://example.org/Dog>");
        assert_eq!(expanded, "http://example.org/Dog");
    }

    #[test]
    fn expand_curie_unknown_returns_as_is() {
        let f = empty_ofn();
        let api = OwlApi::load(f.path(), false, false).unwrap();
        let expanded = api.expand_curie("unknown:Dog");
        assert_eq!(expanded, "unknown:Dog");
    }
}
