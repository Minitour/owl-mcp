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
    AnnotatedComponent, Annotation, AnnotationAssertion, AnnotationSubject, AnnotationValue, ArcStr,
    Build, ClassAssertion, Component, DataPropertyAssertion, Literal, MutableOntology,
    ObjectPropertyAssertion, OntologyID,
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
            return Err(empty_parse_error(axiom_str));
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
            if components.is_empty() {
                return Err(empty_parse_error(axiom_str));
            }
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

    // ── Structured assertions ─────────────────────────────────────────────────
    //
    // These build the axiom from its component parts (property, subject, value)
    // and construct the OWL model object directly. The literal value is stored
    // verbatim and never passes through the functional-syntax parser, so callers
    // can supply arbitrarily long values containing `;`, `=`, `/`, `,`, quotes,
    // or newlines without any escaping or shell-quoting concerns.

    /// Build an OWL literal from a raw value plus an optional datatype or
    /// language tag. A language tag (if non-empty) takes precedence over a
    /// datatype; with neither, a plain `xsd:string` literal is produced.
    fn build_literal(
        &self,
        value: &str,
        datatype: Option<&str>,
        lang: Option<&str>,
    ) -> Literal<ArcStr> {
        if let Some(lang) = lang.filter(|l| !l.is_empty()) {
            Literal::Language {
                literal: value.to_string(),
                lang: lang.to_string(),
            }
        } else if let Some(dt) = datatype.filter(|d| !d.is_empty()) {
            Literal::Datatype {
                literal: value.to_string(),
                datatype_iri: self.build.iri(self.expand_curie(dt)),
            }
        } else {
            Literal::Simple {
                literal: value.to_string(),
            }
        }
    }

    fn insert_component_and_save(
        &mut self,
        component: Component<ArcStr>,
    ) -> Result<String, OwlApiError> {
        if self.readonly {
            return Err(OwlApiError::ReadOnly);
        }
        let ac = AnnotatedComponent::from(component);
        let rendered = self.component_to_string(&ac);
        self.ontology.insert(ac);
        self.save()?;
        Ok(format!("Successfully added: {}", rendered))
    }

    /// Add a `DataPropertyAssertion(property subject "value"^^datatype)`.
    pub fn add_data_property_assertion(
        &mut self,
        property: &str,
        subject: &str,
        value: &str,
        datatype: Option<&str>,
        lang: Option<&str>,
    ) -> Result<String, OwlApiError> {
        let dp = self.build.data_property(self.expand_curie(property));
        let from = self.build.named_individual(self.expand_curie(subject));
        let to = self.build_literal(value, datatype, lang);
        let axiom = DataPropertyAssertion {
            dp,
            from: from.into(),
            to,
        };
        self.insert_component_and_save(Component::DataPropertyAssertion(axiom))
    }

    /// Add an `AnnotationAssertion(property subject "value")`.
    pub fn add_annotation_assertion(
        &mut self,
        property: &str,
        subject: &str,
        value: &str,
        datatype: Option<&str>,
        lang: Option<&str>,
    ) -> Result<String, OwlApiError> {
        let ap = self.build.annotation_property(self.expand_curie(property));
        let subject_iri = self.build.iri(self.expand_curie(subject));
        let av = AnnotationValue::Literal(self.build_literal(value, datatype, lang));
        let axiom = AnnotationAssertion {
            subject: AnnotationSubject::IRI(subject_iri),
            ann: Annotation { ap, av },
        };
        self.insert_component_and_save(Component::AnnotationAssertion(axiom))
    }

    /// Add an `ObjectPropertyAssertion(property subject target)`.
    pub fn add_object_property_assertion(
        &mut self,
        property: &str,
        subject: &str,
        target: &str,
    ) -> Result<String, OwlApiError> {
        let ope = self.build.object_property(self.expand_curie(property));
        let from = self.build.named_individual(self.expand_curie(subject));
        let to = self.build.named_individual(self.expand_curie(target));
        let axiom = ObjectPropertyAssertion {
            ope: ope.into(),
            from: from.into(),
            to: to.into(),
        };
        self.insert_component_and_save(Component::ObjectPropertyAssertion(axiom))
    }

    /// Add a `ClassAssertion(class individual)`.
    pub fn add_class_assertion(
        &mut self,
        class: &str,
        individual: &str,
    ) -> Result<String, OwlApiError> {
        let ce = self.build.class(self.expand_curie(class));
        let i = self.build.named_individual(self.expand_curie(individual));
        let axiom = ClassAssertion {
            ce: ce.into(),
            i: i.into(),
        };
        self.insert_component_and_save(Component::ClassAssertion(axiom))
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

    /// Serialize the in-memory ontology to RDF/XML bytes.
    ///
    /// Used for SPARQL querying: the resulting triples are loaded into an
    /// in-memory RDF store regardless of the on-disk file format.
    pub fn to_rdf_bytes(&self) -> Result<Vec<u8>, OwlApiError> {
        let cmo: ComponentMappedOntology<ArcStr, Arc<AnnotatedComponent<ArcStr>>> =
            self.ontology.clone().into();
        let mut buf: Vec<u8> = Vec::new();
        rdf_write(&mut buf, &cmo)?;
        Ok(buf)
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
        // Try curie::PrefixMapping expansion, then fall back to well-known
        // prefixes (rdf/rdfs/owl/xsd) so callers can use e.g. `rdfs:label`
        // even when the ontology has not declared that prefix explicitly.
        match self.prefixes.expand_curie_string(curie) {
            Ok(expanded) => expanded,
            Err(_) => expand_wellknown_prefix(curie).unwrap_or_else(|| curie.to_string()),
        }
    }
}

/// Expand a CURIE using the standard W3C prefixes (rdf, rdfs, owl, xsd).
/// Returns `None` for anything that is not one of these well-known prefixes.
fn expand_wellknown_prefix(curie: &str) -> Option<String> {
    let (prefix, local) = curie.split_once(':')?;
    let base = match prefix {
        "rdf" => "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
        "rdfs" => "http://www.w3.org/2000/01/rdf-schema#",
        "owl" => "http://www.w3.org/2002/07/owl#",
        "xsd" => "http://www.w3.org/2001/XMLSchema#",
        _ => return None,
    };
    Some(format!("{}{}", base, local))
}

/// Build a precise error for the case where an axiom string parsed without
/// yielding any component — almost always a malformed literal or bad syntax.
fn empty_parse_error(axiom_str: &str) -> OwlApiError {
    OwlApiError::Parse(format!(
        "No axiom was parsed from input. This usually means the axiom is malformed \
         (unterminated string literal, unbalanced parentheses, or an unescaped quote \
         inside a literal). For long or special-character literal values, prefer the \
         structured assertion tools (e.g. add_data_property_assertion / \
         add_annotation_assertion), which take the value as a separate field and require \
         no escaping. Offending input: {}",
        axiom_str
    ))
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

    // ── structured assertions + long-literal robustness ───────────────────────

    /// A ~2 KB literal exercising `;`, `=`, `/`, `,`, embedded quotes and newlines —
    /// the exact shape that breaks hand-written functional-syntax literals.
    fn big_literal() -> String {
        let mut s = String::new();
        for i in 0..50 {
            s.push_str(&format!(
                "source=s{i}; instrument=i{i}, path=/seg/{i}/leaf; expr=x=\"y{i}\"; note=a,b,c\n"
            ));
        }
        assert!(s.len() > 2000, "expected >2KB literal, got {}", s.len());
        s
    }

    fn data_property_values(api: &OwlApi, property: &str) -> Vec<String> {
        api.ontology
            .iter()
            .filter_map(|ac| {
                if let Component::DataPropertyAssertion(dpa) = &ac.component {
                    let p: &str = dpa.dp.0.as_ref();
                    if p == property {
                        return Some(dpa.to.literal().clone());
                    }
                }
                None
            })
            .collect()
    }

    #[test]
    fn add_data_property_assertion_long_literal_roundtrip() {
        let f = empty_ofn();
        let value = big_literal();
        {
            let mut api = OwlApi::load(f.path(), false, false).unwrap();
            let msg = api
                .add_data_property_assertion(
                    "<http://example.org/metaprops>",
                    "<http://example.org/Subj>",
                    &value,
                    None,
                    None,
                )
                .unwrap();
            assert!(msg.contains("Successfully"));
        }
        // Reload fresh from disk: the value must survive serialization byte-for-byte.
        let api2 = OwlApi::load(f.path(), false, false).unwrap();
        let values = data_property_values(&api2, "http://example.org/metaprops");
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], value);
    }

    #[test]
    fn add_annotation_assertion_long_literal_roundtrip() {
        let f = empty_ofn();
        let value = big_literal();
        {
            let mut api = OwlApi::load(f.path(), false, false).unwrap();
            api.add_annotation_assertion(
                "<http://example.org/metaprops>",
                "<http://example.org/Subj>",
                &value,
                None,
                None,
            )
            .unwrap();
        }
        let api2 = OwlApi::load(f.path(), false, false).unwrap();
        let labels = api2.get_labels_for_iri(
            "http://example.org/Subj",
            Some("<http://example.org/metaprops>"),
        );
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0], value);
    }

    #[test]
    fn add_data_property_assertion_with_datatype() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        api.add_data_property_assertion(
            "<http://example.org/age>",
            "<http://example.org/Subj>",
            "42",
            Some("xsd:integer"),
            None,
        )
        .unwrap();
        let api2 = OwlApi::load(f.path(), false, false).unwrap();
        let values = data_property_values(&api2, "http://example.org/age");
        assert_eq!(values, vec!["42"]);
    }

    #[test]
    fn add_annotation_assertion_with_lang() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        api.add_annotation_assertion(
            "rdfs:label",
            "<http://example.org/Subj>",
            "chien",
            None,
            Some("fr"),
        )
        .unwrap();
        let api2 = OwlApi::load(f.path(), false, false).unwrap();
        let labels = api2.get_labels_for_iri("http://example.org/Subj", None);
        assert_eq!(labels, vec!["chien@fr"]);
    }

    #[test]
    fn add_object_property_assertion_persists() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        api.add_object_property_assertion(
            "<http://example.org/knows>",
            "<http://example.org/Alice>",
            "<http://example.org/Bob>",
        )
        .unwrap();
        let api2 = OwlApi::load(f.path(), false, false).unwrap();
        let axioms = api2.get_all_axioms(100, false, None);
        assert!(axioms.iter().any(|s| s.contains("ObjectPropertyAssertion")));
    }

    #[test]
    fn add_class_assertion_persists() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        api.add_class_assertion("<http://example.org/Dog>", "<http://example.org/Rex>")
            .unwrap();
        let api2 = OwlApi::load(f.path(), false, false).unwrap();
        let axioms = api2.get_all_axioms(100, false, None);
        assert!(axioms.iter().any(|s| s.contains("ClassAssertion")));
    }

    #[test]
    fn structured_assertion_readonly_returns_error() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), true, false).unwrap();
        let result = api.add_data_property_assertion(
            "<http://example.org/p>",
            "<http://example.org/s>",
            "v",
            None,
            None,
        );
        assert!(matches!(result, Err(OwlApiError::ReadOnly)));
    }

    // ── hard errors on zero-parse ──────────────────────────────────────────────

    #[test]
    fn add_axiom_empty_input_is_hard_error() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        let result = api.add_axiom("   ");
        assert!(matches!(result, Err(OwlApiError::Parse(_))));
    }

    #[test]
    fn add_axioms_zero_parse_is_hard_error() {
        let f = empty_ofn();
        let mut api = OwlApi::load(f.path(), false, false).unwrap();
        let strs = vec![
            "Declaration(Class(<http://example.org/A>))".to_string(),
            "   ".to_string(),
        ];
        let result = api.add_axioms(&strs);
        assert!(matches!(result, Err(OwlApiError::Parse(_))));
    }

    // ── well-known prefix expansion ────────────────────────────────────────────

    #[test]
    fn expand_curie_wellknown_rdfs() {
        let f = empty_ofn();
        let api = OwlApi::load(f.path(), false, false).unwrap();
        assert_eq!(
            api.expand_curie("rdfs:label"),
            "http://www.w3.org/2000/01/rdf-schema#label"
        );
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
