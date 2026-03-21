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
        #[arg(long = "axiom", required = true)]
        axioms: Vec<String>,
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

    /// Scan an OWL ontology for common modeling pitfalls
    TestPitfalls {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
        /// Comma-separated list of pitfall IDs to check (e.g. "P04,P08,P11")
        #[arg(long)]
        pitfalls: Option<String>,
    },
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
        CliCommand::AddAxioms { file, axioms } => {
            tools::AddAxioms::run_tool(
                tools::AddAxioms {
                    owl_file_path: file,
                    axiom_strs: axioms,
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
    };

    match result {
        Ok(r) => print_result(r),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
