# OWL MCP

[![npm version](https://img.shields.io/npm/v/owl-mcp.svg)](https://www.npmjs.com/package/owl-mcp)
[![CI](https://github.com/Minitour/owl-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/Minitour/owl-mcp/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Node](https://img.shields.io/badge/node-%3E%3D18-green.svg)](https://nodejs.org/)

A high-performance [Model Context Protocol (MCP)](https://modelcontextprotocol.io) server **and CLI** for OWL ontology management, written in Rust.

Built as a drop-in replacement for [ai4curation/owl-mcp](https://github.com/ai4curation/owl-mcp), designed to eliminate the crashes and timeouts inherent to the Python implementation. Axioms are expressed in [OWL Functional Syntax](https://www.w3.org/TR/owl2-syntax/).

## Features

- **22 MCP tools** — add, remove, search, and inspect axioms; manage prefixes, labels, and ontology IRI; register and configure ontologies; scan for pitfalls
- **CLI mode** — every tool is also available as a direct CLI subcommand (`owl-mcp find-axioms ...`)
- **2 transport modes** — `stdio` (default, for Cursor/Claude Desktop) and `http` (Streamable HTTP + SSE)
- **Persistent configuration** — register ontologies by name in `~/.owl-mcp/config.yaml`
- **Live file watching** — automatically reloads ontology files modified externally
- **OFN and RDF/XML support** — reads and writes both formats; format is auto-detected from file extension and content
- **Never crashes** — errors are returned as MCP tool failures, not panics

## Installation

### via npx (recommended)

```bash
npx owl-mcp serve
```

### via npm (global install)

```bash
npm install -g owl-mcp
owl-mcp --help
```

### Build from source

Requires [Rust](https://rustup.rs) 1.75+.

```bash
git clone https://github.com/Minitour/owl-mcp
cd owl-mcp
cargo build --release
./target/release/owl-mcp --help
```

## Usage

owl-mcp has two modes: **serve** (MCP server) and **CLI** (direct commands).

### MCP server mode

```bash
owl-mcp serve [OPTIONS]

Options:
  --transport <stdio|http>   Transport to use [default: stdio]
  --host <HOST>              Host to bind (HTTP only) [default: 127.0.0.1]
  --port <PORT>              Port to bind (HTTP only) [default: 8080]
  --sse-support              Enable legacy SSE endpoint [default: true]
```

### CLI mode

Every MCP tool is available as a subcommand:

```bash
owl-mcp add-axiom --file ontology.owl --axiom "SubClassOf(:Dog :Animal)"
owl-mcp find-axioms --file ontology.owl --pattern "Dog" --limit 50
owl-mcp get-all-axioms --file ontology.owl --include-labels
owl-mcp test-pitfalls --file ontology.owl
owl-mcp list-configured-ontologies
owl-mcp find-axioms-by-name --name pizza --pattern "Topping"
```

Run `owl-mcp --help` for a full list of commands, or `owl-mcp <command> --help` for details on a specific command.

## Cursor / Claude Desktop integration

Add the server to your MCP client configuration.

**Cursor** (`~/.cursor/mcp.json` or `.cursor/mcp.json` in your project):

```json
{
  "mcpServers": {
    "owl-mcp": {
      "command": "npx",
      "args": ["-y", "owl-mcp", "serve"]
    }
  }
}
```

**HTTP transport** (useful for remote/shared setups):

```json
{
  "mcpServers": {
    "owl-mcp": {
      "url": "http://localhost:8080/mcp"
    }
  }
}
```

Start the server with:

```bash
owl-mcp serve --transport http --port 8080
```

## Tools

### Axiom operations (by file path)

| Tool | Description |
|---|---|
| `add_axiom` | Add a single axiom in OWL Functional Syntax |
| `add_axioms` | Add multiple axioms in one call |
| `remove_axiom` | Remove an axiom |
| `find_axioms` | Search axioms with a regex pattern |
| `get_all_axioms` | List all axioms (up to a limit) |

### Axiom operations (by registered name)

| Tool | Description |
|---|---|
| `add_axiom_by_name` | Add an axiom to a configured ontology |
| `remove_axiom_by_name` | Remove an axiom from a configured ontology |
| `find_axioms_by_name` | Search axioms in a configured ontology |

### Metadata and labels

| Tool | Description |
|---|---|
| `add_prefix` | Add a prefix mapping (`ex:` → `http://example.org/`) |
| `add_prefix_by_name` | Same, for a configured ontology |
| `ontology_metadata` | Return ontology-level annotation axioms |
| `get_labels_for_iri` | Look up `rdfs:label` (or custom property) values for an IRI |
| `get_labels_for_iri_by_name` | Same, for a configured ontology |
| `set_ontology_iri` | Set or update the ontology IRI and version IRI |
| `set_ontology_iri_by_name` | Same, for a configured ontology |

### Configuration

| Tool | Description |
|---|---|
| `list_configured_ontologies` | List all registered ontologies |
| `configure_ontology` | Add or update a named ontology entry |
| `remove_ontology_config` | Remove a named ontology entry |
| `get_ontology_config` | Retrieve configuration for a named ontology |
| `register_ontology_in_config` | Register an existing file by name |
| `load_and_register_ontology` | Load (or create) a file and register it |

### Quality checks

| Tool | Description |
|---|---|
| `test_pitfalls` | Scan an ontology for common modeling pitfalls (31 checks) |

All `find_axioms` and `get_all_axioms` tools accept `include_labels: true` to annotate each axiom with human-readable labels appended as `## <IRI> # label` comments.

## Configuration file

Registered ontologies are stored at `~/.owl-mcp/config.yaml`:

```yaml
ontologies:
  pizza:
    name: pizza
    path: /data/pizza.ofn
    readonly: false
    description: "Pizza ontology tutorial"
    annotation_property: null
    preferred_serialization: null
    metadata_axioms: []
```

You can manage this file through the MCP tools or edit it directly — changes are picked up on the next tool call.

## Development

```bash
# Run tests
cargo test

# Check formatting and lints
cargo fmt --check
cargo clippy --all-targets -- -D warnings

# Build release binary
cargo build --release
```

## License

MIT
