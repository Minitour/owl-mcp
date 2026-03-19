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

    /// Add an axiom to a configured ontology by its registered name
    AddAxiomByName {
        /// Name of a configured ontology
        #[arg(long)]
        name: String,
        /// Axiom in OWL functional syntax
        #[arg(long)]
        axiom: String,
    },

    /// Remove an axiom from a configured ontology by its registered name
    RemoveAxiomByName {
        /// Name of a configured ontology
        #[arg(long)]
        name: String,
        /// Axiom in OWL functional syntax to remove
        #[arg(long)]
        axiom: String,
    },

    /// Search axioms in a configured ontology by name using a regex pattern
    FindAxiomsByName {
        /// Name of a configured ontology
        #[arg(long)]
        name: String,
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

    /// Add a prefix mapping to a configured ontology by name
    AddPrefixByName {
        /// Name of a configured ontology
        #[arg(long)]
        name: String,
        /// Prefix name including colon, e.g. 'ex:'
        #[arg(long)]
        prefix: String,
        /// The full IRI the prefix expands to
        #[arg(long)]
        uri: String,
    },

    /// Return label values for an IRI in a configured ontology by name
    GetLabelsForIriByName {
        /// Name of a configured ontology
        #[arg(long)]
        name: String,
        /// Full IRI or CURIE
        #[arg(long)]
        iri: String,
        /// IRI or CURIE of the annotation property (default: rdfs:label)
        #[arg(long)]
        annotation_property: Option<String>,
    },

    /// List all ontologies registered in the configuration
    ListConfiguredOntologies,

    /// Add or update a named ontology in the configuration
    ConfigureOntology {
        /// Unique name for this ontology
        #[arg(long)]
        name: String,
        /// Absolute path to the OWL file
        #[arg(long)]
        path: String,
        /// Metadata axioms to add when loading (one per --metadata-axiom flag)
        #[arg(long = "metadata-axiom")]
        metadata_axioms: Vec<String>,
        /// Mark the ontology as read-only
        #[arg(long)]
        readonly: bool,
        /// Human-readable description
        #[arg(long)]
        description: Option<String>,
        /// Preferred serialization format (e.g. 'ofn', 'rdf')
        #[arg(long)]
        preferred_serialization: Option<String>,
        /// IRI or CURIE of the annotation property for labels
        #[arg(long)]
        annotation_property: Option<String>,
    },

    /// Remove an ontology from the configuration
    RemoveOntologyConfig {
        /// Name of the configured ontology to remove
        #[arg(long)]
        name: String,
    },

    /// Retrieve the configuration for a specific named ontology
    GetOntologyConfig {
        /// Name of the configured ontology
        #[arg(long)]
        name: String,
    },

    /// Register an OWL file in the configuration by name
    RegisterOntologyInConfig {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
        /// Name to register under (defaults to the file stem)
        #[arg(long)]
        name: Option<String>,
        /// Mark the ontology as read-only
        #[arg(long)]
        readonly: bool,
        /// Human-readable description
        #[arg(long)]
        description: Option<String>,
        /// Preferred serialization format
        #[arg(long)]
        preferred_serialization: Option<String>,
        /// IRI or CURIE of the annotation property for labels
        #[arg(long)]
        annotation_property: Option<String>,
    },

    /// Load (or create) an OWL file, optionally add metadata, and register it
    LoadAndRegisterOntology {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: String,
        /// Name to register under (defaults to the file stem)
        #[arg(long)]
        name: Option<String>,
        /// Mark the ontology as read-only
        #[arg(long)]
        readonly: bool,
        /// Do NOT create the file if it does not exist (by default, the file is created)
        #[arg(long)]
        no_create: bool,
        /// Human-readable description
        #[arg(long)]
        description: Option<String>,
        /// Preferred serialization format
        #[arg(long)]
        preferred_serialization: Option<String>,
        /// Metadata axioms to add when loading (one per --metadata-axiom flag)
        #[arg(long = "metadata-axiom")]
        metadata_axioms: Vec<String>,
        /// IRI or CURIE of the annotation property for labels
        #[arg(long)]
        annotation_property: Option<String>,
    },

    /// Scan an OWL ontology for common modeling pitfalls
    TestPitfalls {
        /// Absolute path to the OWL file
        #[arg(long)]
        file: Option<String>,
        /// Name of a configured ontology
        #[arg(long)]
        name: Option<String>,
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
        CliCommand::AddAxiomByName { name, axiom } => {
            tools::AddAxiomByName::run_tool(
                tools::AddAxiomByName {
                    ontology_name: name,
                    axiom_str: axiom,
                },
                &manager,
            )
            .await
        }
        CliCommand::RemoveAxiomByName { name, axiom } => {
            tools::RemoveAxiomByName::run_tool(
                tools::RemoveAxiomByName {
                    ontology_name: name,
                    axiom_str: axiom,
                },
                &manager,
            )
            .await
        }
        CliCommand::FindAxiomsByName {
            name,
            pattern,
            limit,
            include_labels,
            annotation_property,
        } => {
            tools::FindAxiomsByName::run_tool(
                tools::FindAxiomsByName {
                    ontology_name: name,
                    pattern,
                    limit,
                    include_labels,
                    annotation_property,
                },
                &manager,
            )
            .await
        }
        CliCommand::AddPrefixByName { name, prefix, uri } => {
            tools::AddPrefixByName::run_tool(
                tools::AddPrefixByName {
                    ontology_name: name,
                    prefix,
                    uri,
                },
                &manager,
            )
            .await
        }
        CliCommand::GetLabelsForIriByName {
            name,
            iri,
            annotation_property,
        } => {
            tools::GetLabelsForIriByName::run_tool(
                tools::GetLabelsForIriByName {
                    ontology_name: name,
                    iri,
                    annotation_property,
                },
                &manager,
            )
            .await
        }
        CliCommand::ListConfiguredOntologies => {
            tools::ListConfiguredOntologies::run_tool(tools::ListConfiguredOntologies {}, &manager)
                .await
        }
        CliCommand::ConfigureOntology {
            name,
            path,
            metadata_axioms,
            readonly,
            description,
            preferred_serialization,
            annotation_property,
        } => {
            let meta = if metadata_axioms.is_empty() {
                None
            } else {
                Some(metadata_axioms)
            };
            tools::ConfigureOntology::run_tool(
                tools::ConfigureOntology {
                    name,
                    path,
                    metadata_axioms: meta,
                    readonly,
                    description,
                    preferred_serialization,
                    annotation_property,
                },
                &manager,
            )
            .await
        }
        CliCommand::RemoveOntologyConfig { name } => {
            tools::RemoveOntologyConfig::run_tool(tools::RemoveOntologyConfig { name }, &manager)
                .await
        }
        CliCommand::GetOntologyConfig { name } => {
            tools::GetOntologyConfig::run_tool(tools::GetOntologyConfig { name }, &manager).await
        }
        CliCommand::RegisterOntologyInConfig {
            file,
            name,
            readonly,
            description,
            preferred_serialization,
            annotation_property,
        } => {
            let ro = if readonly { Some(true) } else { None };
            tools::RegisterOntologyInConfig::run_tool(
                tools::RegisterOntologyInConfig {
                    owl_file_path: file,
                    name,
                    readonly: ro,
                    description,
                    preferred_serialization,
                    annotation_property,
                },
                &manager,
            )
            .await
        }
        CliCommand::LoadAndRegisterOntology {
            file,
            name,
            readonly,
            no_create,
            description,
            preferred_serialization,
            metadata_axioms,
            annotation_property,
        } => {
            let meta = if metadata_axioms.is_empty() {
                None
            } else {
                Some(metadata_axioms)
            };
            tools::LoadAndRegisterOntology::run_tool(
                tools::LoadAndRegisterOntology {
                    owl_file_path: file,
                    name,
                    readonly,
                    create_if_not_exists: !no_create,
                    description,
                    preferred_serialization,
                    metadata_axioms: meta,
                    annotation_property,
                },
                &manager,
            )
            .await
        }
        CliCommand::TestPitfalls {
            file,
            name,
            pitfalls,
        } => {
            tools::TestPitfalls::run_tool(
                tools::TestPitfalls {
                    owl_file_path: file,
                    ontology_name: name,
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
