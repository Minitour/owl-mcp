# owl-mcp

A high-performance [Model Context Protocol (MCP)](https://modelcontextprotocol.io) server for OWL ontology management, written in Rust.

Built as a drop-in replacement for [ai4curation/owl-mcp](https://github.com/ai4curation/owl-mcp), designed to eliminate the crashes and timeouts inherent to the Python implementation. Axioms are expressed in [OWL Functional Syntax](https://www.w3.org/TR/owl2-syntax/).

## Features

- **19 MCP tools** — add, remove, search, and inspect axioms; manage prefixes and labels; register and configure ontologies
- **2 transport modes** — `stdio` (default, for Cursor/Claude Desktop) and `http` (Streamable HTTP + SSE)
- **Persistent configuration** — register ontologies by name in `~/.owl-mcp/config.yaml`
- **Live file watching** — automatically reloads ontology files modified externally
- **OFN and RDF/XML support** — reads and writes both formats; format is auto-detected from file extension and content
- **Never crashes** — errors are returned as MCP tool failures, not panics

## Installation

### via npx (recommended)

```bash
npx owl-mcp
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

```
owl-mcp [OPTIONS]

Options:
  --transport <stdio|http>   Transport to use [default: stdio] [env: OWL_MCP_TRANSPORT]
  --host <HOST>              Host to bind (HTTP only) [default: 127.0.0.1] [env: OWL_MCP_HOST]
  --port <PORT>              Port to bind (HTTP only) [default: 8080] [env: OWL_MCP_PORT]
  --sse-support              Enable legacy SSE endpoint alongside Streamable HTTP [env: OWL_MCP_SSE_SUPPORT]
  -h, --help                 Print help
  -V, --version              Print version
```

## Cursor / Claude Desktop integration

Add the server to your MCP client configuration.

**Cursor** (`~/.cursor/mcp.json` or `.cursor/mcp.json` in your project):

```json
{
  "mcpServers": {
    "owl-mcp": {
      "command": "npx",
      "args": ["-y", "owl-mcp"]
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
owl-mcp --transport http --port 8080
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

### Configuration

| Tool | Description |
|---|---|
| `list_configured_ontologies` | List all registered ontologies |
| `configure_ontology` | Add or update a named ontology entry |
| `remove_ontology_config` | Remove a named ontology entry |
| `get_ontology_config` | Retrieve configuration for a named ontology |
| `register_ontology_in_config` | Register an existing file by name |
| `load_and_register_ontology` | Load (or create) a file and register it |

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

## Releasing

Push a version tag to trigger the [release workflow](.github/workflows/release.yml), which will:

1. Build native binaries for Linux (x64, arm64), macOS (x64, arm64), and Windows (x64)
2. Publish platform-specific npm packages (`@owl-mcp/owl-mcp-<platform>`)
3. Publish the main `owl-mcp` npm package
4. Create a GitHub Release with all binaries attached

```bash
git tag v1.2.0
git push --tags
```

The `NPM_TOKEN` secret must be configured in the repository settings before the first release.

## License

MIT — Antonio Zaitoun &lt;antonio@zaitoun.dev&gt;
