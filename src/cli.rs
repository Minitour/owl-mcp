use std::sync::Arc;

use clap::Subcommand;
use rust_mcp_sdk::schema::{CallToolResult, ContentBlock};
use tokio::sync::Mutex;

use crate::ontology::manager::OntologyManager;
use crate::tools;

type Manager = Arc<Mutex<OntologyManager>>;

#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// Add a single OWL axiom in functional syntax to an ontology file
    AddAxiom {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
        /// Axiom in OWL functional syntax, e.g. SubClassOf(:Dog :Animal)
        #[arg(long)]
        axiom: String,
    },

    /// Add multiple OWL axioms in functional syntax (stops on first failure)
    AddAxioms {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
        /// Axioms in OWL functional syntax (one per --axiom flag)
        #[arg(long = "axiom")]
        axioms: Vec<String>,
        /// Read axioms from a file instead of --axiom, bypassing shell quoting.
        /// Content is a JSON array of strings, or NUL/newline-delimited axioms.
        /// Use '-' to read from stdin. (Newline-delimited cannot contain
        /// multi-line literals; use JSON or NUL for those.)
        #[arg(long)]
        axioms_file: Option<String>,
    },

    /// Add a data property assertion; the value is a separate field (no escaping needed)
    AddDataPropertyAssertion {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
        /// Data property IRI or CURIE
        #[arg(long)]
        property: String,
        /// Subject individual IRI or CURIE
        #[arg(long)]
        subject: String,
        /// The literal value (stored verbatim). One of --value or --value-file is required.
        #[arg(long)]
        value: Option<String>,
        /// Read the literal value verbatim from a file (use '-' for stdin), bypassing shell quoting
        #[arg(long)]
        value_file: Option<String>,
        /// Optional datatype IRI or CURIE (e.g. 'xsd:string')
        #[arg(long)]
        datatype: Option<String>,
        /// Optional language tag (e.g. 'en')
        #[arg(long)]
        lang: Option<String>,
    },

    /// Add an annotation assertion; the value is a separate field (no escaping needed)
    AddAnnotationAssertion {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
        /// Annotation property IRI or CURIE (e.g. 'rdfs:label')
        #[arg(long)]
        property: String,
        /// Subject IRI or CURIE
        #[arg(long)]
        subject: String,
        /// The literal value (stored verbatim). One of --value or --value-file is required.
        #[arg(long)]
        value: Option<String>,
        /// Read the literal value verbatim from a file (use '-' for stdin), bypassing shell quoting
        #[arg(long)]
        value_file: Option<String>,
        /// Optional datatype IRI or CURIE (e.g. 'xsd:string')
        #[arg(long)]
        datatype: Option<String>,
        /// Optional language tag (e.g. 'en')
        #[arg(long)]
        lang: Option<String>,
    },

    /// Add an object property assertion linking two individuals
    AddObjectPropertyAssertion {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
        /// Object property IRI or CURIE
        #[arg(long)]
        property: String,
        /// Subject (from) individual IRI or CURIE
        #[arg(long)]
        subject: String,
        /// Target (to) individual IRI or CURIE
        #[arg(long)]
        target: String,
    },

    /// Add a class assertion (individual is an instance of class)
    AddClassAssertion {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
        /// Class IRI or CURIE
        #[arg(long)]
        class: String,
        /// Individual IRI or CURIE
        #[arg(long)]
        individual: String,
    },

    /// Remove a single OWL axiom from an ontology file
    RemoveAxiom {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
        /// Axiom in OWL functional syntax to remove
        #[arg(long)]
        axiom: String,
    },

    /// Search axioms in an OWL file using a regex pattern
    FindAxioms {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
        /// Regex pattern to match against functional-syntax axiom strings
        #[arg(long)]
        pattern: String,
        /// Maximum number of results to return
        #[arg(long, default_value_t = 100)]
        limit: u64,
        /// Append human-readable labels after ## comments
        #[arg(long)]
        include_labels: bool,
        /// IRI or CURIE of the annotation property to use for labels
        #[arg(long)]
        annotation_property: Option<String>,
    },

    /// Return all axioms in an OWL file (up to limit)
    GetAllAxioms {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
        /// Maximum number of results to return
        #[arg(long, default_value_t = 100)]
        limit: u64,
        /// Append human-readable labels after ## comments
        #[arg(long)]
        include_labels: bool,
        /// IRI or CURIE of the annotation property to use for labels
        #[arg(long)]
        annotation_property: Option<String>,
    },

    /// Add a prefix mapping to an ontology file
    AddPrefix {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
        /// Prefix name including colon, e.g. 'ex:'
        #[arg(long)]
        prefix: String,
        /// The full IRI the prefix expands to
        #[arg(long)]
        uri: String,
    },

    /// Return ontology-level annotation axioms (metadata header)
    OntologyMetadata {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
    },

    /// Return all label values for a given IRI or CURIE
    GetLabelsForIri {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
        /// Full IRI or CURIE (e.g. 'ex:Dog' or '<http://example.org/Dog>')
        #[arg(long)]
        iri: String,
        /// IRI or CURIE of the annotation property (default: rdfs:label)
        #[arg(long)]
        annotation_property: Option<String>,
    },

    /// Set or update the ontology IRI (and optional version IRI)
    SetOntologyIri {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
        /// The ontology IRI to set
        #[arg(long)]
        iri: Option<String>,
        /// Optional version IRI
        #[arg(long)]
        version_iri: Option<String>,
    },

    /// Evaluate ontology quality using the OQuaRE framework
    TestQuality {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
    },

    /// Scan an OWL ontology for common modeling pitfalls
    TestPitfalls {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
        /// Comma-separated list of pitfall IDs to check (e.g. "P04,P08,P11")
        #[arg(long)]
        pitfalls: Option<String>,
    },

    /// Run a SPARQL query over one or more OWL files (merged into one RDF graph)
    Sparql {
        /// Absolute path to an OWL file. Repeat --file to merge several (e.g. schema + ABox).
        #[arg(long = "file", required = true)]
        files: Vec<String>,
        /// SPARQL query string (SELECT, ASK, CONSTRUCT, or DESCRIBE)
        #[arg(long)]
        query: Option<String>,
        /// Read the SPARQL query from a file instead of --query (use '-' for stdin), bypassing shell quoting
        #[arg(long)]
        query_file: Option<String>,
    },
}

/// Read the full contents of a file, or stdin when `path` is "-".
fn read_file_or_stdin(path: &str) -> String {
    if path == "-" {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut s).unwrap_or_else(|e| {
            eprintln!("Error reading stdin: {}", e);
            std::process::exit(1);
        });
        s
    } else {
        std::fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("Error reading {}: {}", path, e);
            std::process::exit(1);
        })
    }
}

/// Strip a single trailing newline (and a preceding CR) — files written by
/// editors/`Set-Content` commonly append one, which is rarely intended as
/// part of a literal value.
fn strip_one_trailing_newline(mut s: String) -> String {
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    }
    s
}

/// Resolve a literal value from either an inline `--value` or a `--value-file`.
fn resolve_value(value: Option<String>, value_file: Option<String>) -> String {
    match (value, value_file) {
        (Some(v), _) => v,
        (None, Some(path)) => strip_one_trailing_newline(read_file_or_stdin(&path)),
        (None, None) => {
            eprintln!("Error: one of --value or --value-file is required");
            std::process::exit(1);
        }
    }
}

/// Parse a batch of axioms from raw file/stdin content: a JSON array of
/// strings, or NUL-delimited, or (fallback) newline-delimited.
fn parse_axiom_list(content: &str) -> Vec<String> {
    let trimmed = content.trim_start();
    if trimmed.starts_with('[') {
        return serde_json::from_str::<Vec<String>>(trimmed).unwrap_or_else(|e| {
            eprintln!("Error parsing JSON axioms array: {}", e);
            std::process::exit(1);
        });
    }
    let delimiter = if content.contains('\0') { '\0' } else { '\n' };
    content
        .split(delimiter)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Resolve the axiom list from inline `--axiom` flags or `--axioms-file`/stdin.
fn resolve_axioms(axioms: Vec<String>, axioms_file: Option<String>) -> Vec<String> {
    if !axioms.is_empty() {
        return axioms;
    }
    let content = read_file_or_stdin(axioms_file.as_deref().unwrap_or("-"));
    let parsed = parse_axiom_list(&content);
    if parsed.is_empty() {
        eprintln!("Error: no axioms provided (via --axiom, --axioms-file, or stdin)");
        std::process::exit(1);
    }
    parsed
}

fn print_result(result: CallToolResult) {
    for block in result.content {
        if let ContentBlock::TextContent(tc) = block {
            println!("{}", tc.text);
        }
    }
}

pub async fn dispatch(cmd: CliCommand, manager: Manager) {
    let result = match cmd {
        CliCommand::AddAxiom { file, axiom } => {
            tools::AddAxiom::run_tool(
                tools::AddAxiom {
                    owl_file_path: file,
                    axiom_str: axiom,
                },
                &manager,
            )
            .await
        }
        CliCommand::AddAxioms {
            file,
            axioms,
            axioms_file,
        } => {
            let axiom_strs = resolve_axioms(axioms, axioms_file);
            tools::AddAxioms::run_tool(
                tools::AddAxioms {
                    owl_file_path: file,
                    axiom_strs,
                },
                &manager,
            )
            .await
        }
        CliCommand::AddDataPropertyAssertion {
            file,
            property,
            subject,
            value,
            value_file,
            datatype,
            lang,
        } => {
            let value = resolve_value(value, value_file);
            tools::AddDataPropertyAssertion::run_tool(
                tools::AddDataPropertyAssertion {
                    owl_file_path: file,
                    property,
                    subject,
                    value,
                    datatype,
                    lang,
                },
                &manager,
            )
            .await
        }
        CliCommand::AddAnnotationAssertion {
            file,
            property,
            subject,
            value,
            value_file,
            datatype,
            lang,
        } => {
            let value = resolve_value(value, value_file);
            tools::AddAnnotationAssertion::run_tool(
                tools::AddAnnotationAssertion {
                    owl_file_path: file,
                    property,
                    subject,
                    value,
                    datatype,
                    lang,
                },
                &manager,
            )
            .await
        }
        CliCommand::AddObjectPropertyAssertion {
            file,
            property,
            subject,
            target,
        } => {
            tools::AddObjectPropertyAssertion::run_tool(
                tools::AddObjectPropertyAssertion {
                    owl_file_path: file,
                    property,
                    subject,
                    target,
                },
                &manager,
            )
            .await
        }
        CliCommand::AddClassAssertion {
            file,
            class,
            individual,
        } => {
            tools::AddClassAssertion::run_tool(
                tools::AddClassAssertion {
                    owl_file_path: file,
                    class,
                    individual,
                },
                &manager,
            )
            .await
        }
        CliCommand::RemoveAxiom { file, axiom } => {
            tools::RemoveAxiom::run_tool(
                tools::RemoveAxiom {
                    owl_file_path: file,
                    axiom_str: axiom,
                },
                &manager,
            )
            .await
        }
        CliCommand::FindAxioms {
            file,
            pattern,
            limit,
            include_labels,
            annotation_property,
        } => {
            tools::FindAxioms::run_tool(
                tools::FindAxioms {
                    owl_file_path: file,
                    pattern,
                    limit,
                    include_labels,
                    annotation_property,
                },
                &manager,
            )
            .await
        }
        CliCommand::GetAllAxioms {
            file,
            limit,
            include_labels,
            annotation_property,
        } => {
            tools::GetAllAxioms::run_tool(
                tools::GetAllAxioms {
                    owl_file_path: file,
                    limit,
                    include_labels,
                    annotation_property,
                },
                &manager,
            )
            .await
        }
        CliCommand::AddPrefix { file, prefix, uri } => {
            tools::AddPrefix::run_tool(
                tools::AddPrefix {
                    owl_file_path: file,
                    prefix,
                    uri,
                },
                &manager,
            )
            .await
        }
        CliCommand::OntologyMetadata { file } => {
            tools::OntologyMetadata::run_tool(
                tools::OntologyMetadata {
                    owl_file_path: file,
                },
                &manager,
            )
            .await
        }
        CliCommand::GetLabelsForIri {
            file,
            iri,
            annotation_property,
        } => {
            tools::GetLabelsForIri::run_tool(
                tools::GetLabelsForIri {
                    owl_file_path: file,
                    iri,
                    annotation_property,
                },
                &manager,
            )
            .await
        }
        CliCommand::SetOntologyIri {
            file,
            iri,
            version_iri,
        } => {
            tools::SetOntologyIri::run_tool(
                tools::SetOntologyIri {
                    owl_file_path: file,
                    iri,
                    version_iri,
                },
                &manager,
            )
            .await
        }
        CliCommand::TestQuality { file } => {
            tools::TestQuality::run_tool(
                tools::TestQuality {
                    owl_file_path: file,
                },
                &manager,
            )
            .await
        }
        CliCommand::TestPitfalls { file, pitfalls } => {
            tools::TestPitfalls::run_tool(
                tools::TestPitfalls {
                    owl_file_path: file,
                    pitfalls,
                },
                &manager,
            )
            .await
        }
        CliCommand::Sparql {
            files,
            query,
            query_file,
        } => {
            let query = match (query, query_file) {
                (Some(q), _) => q,
                (None, Some(path)) => read_file_or_stdin(&path),
                (None, None) => {
                    eprintln!("Error: one of --query or --query-file is required");
                    std::process::exit(1);
                }
            };
            tools::SparqlQuery::run_tool(
                tools::SparqlQuery {
                    owl_file_paths: files,
                    query,
                },
                &manager,
            )
            .await
        }
    };

    match result {
        Ok(r) => print_result(r),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_axiom_list_json_array() {
        let content = r#"["Declaration(Class(<http://example.org/A>))", "SubClassOf(<a> <b>)"]"#;
        let v = parse_axiom_list(content);
        assert_eq!(v.len(), 2);
        assert!(v[0].contains("Declaration"));
    }

    #[test]
    fn parse_axiom_list_json_preserves_special_chars() {
        // A literal containing the chars that break newline/NUL splitting.
        let content =
            "[\"AnnotationAssertion(<p> <s> \\\"a;b=c,/\\nd\\\")\", \"Declaration(Class(<x>))\"]";
        let v = parse_axiom_list(content);
        assert_eq!(v.len(), 2);
        assert!(v[0].contains("a;b=c,/"));
        assert!(v[0].contains('\n'));
    }

    #[test]
    fn parse_axiom_list_newline_delimited() {
        let content = "Declaration(Class(<a>))\n\nDeclaration(Class(<b>))\n";
        let v = parse_axiom_list(content);
        assert_eq!(
            v,
            vec!["Declaration(Class(<a>))", "Declaration(Class(<b>))"]
        );
    }

    #[test]
    fn parse_axiom_list_nul_delimited() {
        let content = "Declaration(Class(<a>))\0Declaration(Class(<b>))";
        let v = parse_axiom_list(content);
        assert_eq!(
            v,
            vec!["Declaration(Class(<a>))", "Declaration(Class(<b>))"]
        );
    }

    #[test]
    fn strip_one_trailing_newline_variants() {
        assert_eq!(strip_one_trailing_newline("abc\n".to_string()), "abc");
        assert_eq!(strip_one_trailing_newline("abc\r\n".to_string()), "abc");
        assert_eq!(strip_one_trailing_newline("abc".to_string()), "abc");
        assert_eq!(strip_one_trailing_newline("a\nb\n".to_string()), "a\nb");
    }

    #[test]
    fn resolve_value_prefers_inline() {
        let v = resolve_value(Some("inline".to_string()), None);
        assert_eq!(v, "inline");
    }
}
