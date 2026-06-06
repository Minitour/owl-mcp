//! SPARQL querying over OWL ontologies.
//!
//! One or more ontologies are serialized to RDF/XML and loaded together into an
//! in-memory [`oxigraph`] store, then a SPARQL query is evaluated against the
//! merged graph. Results are returned in the standard SPARQL 1.1 JSON results
//! format for `SELECT`/`ASK`, and as a list of N-Triples for `CONSTRUCT`/`DESCRIBE`.

use oxigraph::io::RdfFormat;
use oxigraph::model::Term;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use serde_json::json;

use crate::ontology::owl_api::OwlApiError;

const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// Evaluate `query` over the RDF/XML documents in `rdf_docs` (merged into one graph).
///
/// Returns a pretty-printed JSON string. `SELECT`/`ASK` use the W3C SPARQL JSON
/// results format; `CONSTRUCT`/`DESCRIBE` return `{ "triples": [ ... ] }`.
pub fn query(rdf_docs: &[Vec<u8>], query: &str) -> Result<String, OwlApiError> {
    let store = Store::new().map_err(|e| OwlApiError::Parse(e.to_string()))?;

    for doc in rdf_docs {
        store
            .load_from_slice(RdfFormat::RdfXml, doc.as_slice())
            .map_err(|e| OwlApiError::Parse(format!("RDF load error: {e}")))?;
    }

    let results = SparqlEvaluator::new()
        .parse_query(query)
        .map_err(|e| OwlApiError::Parse(format!("SPARQL parse error: {e}")))?
        .on_store(&store)
        .execute()
        .map_err(|e| OwlApiError::Parse(format!("SPARQL evaluation error: {e}")))?;

    results_to_json(results)
}

fn results_to_json(results: QueryResults) -> Result<String, OwlApiError> {
    let value = match results {
        QueryResults::Solutions(solutions) => {
            let vars: Vec<String> = solutions
                .variables()
                .iter()
                .map(|v| v.as_str().to_string())
                .collect();

            let mut bindings = Vec::new();
            for solution in solutions {
                let solution = solution.map_err(|e| OwlApiError::Parse(e.to_string()))?;
                let mut obj = serde_json::Map::new();
                for (var, term) in solution.iter() {
                    obj.insert(var.as_str().to_string(), term_to_json(term));
                }
                bindings.push(serde_json::Value::Object(obj));
            }

            json!({
                "head": { "vars": vars },
                "results": { "bindings": bindings }
            })
        }
        QueryResults::Boolean(value) => json!({ "head": {}, "boolean": value }),
        QueryResults::Graph(triples) => {
            let mut lines = Vec::new();
            for triple in triples {
                let triple = triple.map_err(|e| OwlApiError::Parse(e.to_string()))?;
                lines.push(triple.to_string());
            }
            json!({ "head": {}, "triples": lines })
        }
    };

    serde_json::to_string_pretty(&value).map_err(|e| OwlApiError::Parse(e.to_string()))
}

fn term_to_json(term: &Term) -> serde_json::Value {
    match term {
        Term::NamedNode(n) => json!({ "type": "uri", "value": n.as_str() }),
        Term::BlankNode(b) => json!({ "type": "bnode", "value": b.as_str() }),
        Term::Literal(l) => {
            let mut m = serde_json::Map::new();
            m.insert("type".to_string(), json!("literal"));
            m.insert("value".to_string(), json!(l.value()));
            if let Some(lang) = l.language() {
                m.insert("xml:lang".to_string(), json!(lang));
            } else {
                let dt = l.datatype();
                if dt.as_str() != XSD_STRING {
                    m.insert("datatype".to_string(), json!(dt.as_str()));
                }
            }
            serde_json::Value::Object(m)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal RDF/XML document: A rdfs:subClassOf B, and an instance a1 of A.
    const RDFXML: &str = r#"<?xml version="1.0"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
         xmlns:rdfs="http://www.w3.org/2000/01/rdf-schema#"
         xmlns:owl="http://www.w3.org/2002/07/owl#"
         xmlns:ex="http://example.org/">
  <owl:Class rdf:about="http://example.org/A">
    <rdfs:subClassOf rdf:resource="http://example.org/B"/>
    <rdfs:label>Class A</rdfs:label>
  </owl:Class>
  <owl:Class rdf:about="http://example.org/B"/>
  <owl:NamedIndividual rdf:about="http://example.org/a1">
    <rdf:type rdf:resource="http://example.org/A"/>
  </owl:NamedIndividual>
</rdf:RDF>
"#;

    fn docs() -> Vec<Vec<u8>> {
        vec![RDFXML.as_bytes().to_vec()]
    }

    #[test]
    fn select_returns_solutions() {
        let q = r#"PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
            SELECT ?sub ?sup WHERE { ?sub rdfs:subClassOf ?sup }"#;
        let out = query(&docs(), q).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let bindings = v["results"]["bindings"].as_array().unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(
            bindings[0]["sub"]["value"].as_str().unwrap(),
            "http://example.org/A"
        );
        assert_eq!(
            bindings[0]["sup"]["value"].as_str().unwrap(),
            "http://example.org/B"
        );
    }

    #[test]
    fn ask_returns_boolean() {
        let q = r#"PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
            ASK { <http://example.org/a1> rdf:type <http://example.org/A> }"#;
        let out = query(&docs(), q).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v["boolean"].as_bool().unwrap());
    }

    #[test]
    fn literal_label_has_value() {
        let q = r#"PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
            SELECT ?label WHERE { <http://example.org/A> rdfs:label ?label }"#;
        let out = query(&docs(), q).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let bindings = v["results"]["bindings"].as_array().unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0]["label"]["type"].as_str().unwrap(), "literal");
        assert_eq!(bindings[0]["label"]["value"].as_str().unwrap(), "Class A");
    }

    #[test]
    fn merges_multiple_documents() {
        // Schema in one doc, ABox in another — query relies on both.
        let schema = r#"<?xml version="1.0"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
         xmlns:owl="http://www.w3.org/2002/07/owl#">
  <owl:Class rdf:about="http://example.org/A"/>
</rdf:RDF>"#;
        let abox = r#"<?xml version="1.0"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="http://example.org/a1">
    <rdf:type rdf:resource="http://example.org/A"/>
  </rdf:Description>
</rdf:RDF>"#;
        let docs = vec![schema.as_bytes().to_vec(), abox.as_bytes().to_vec()];
        let q = r#"PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
            SELECT ?i WHERE { ?i rdf:type <http://example.org/A> }"#;
        let out = query(&docs, q).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let bindings = v["results"]["bindings"].as_array().unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(
            bindings[0]["i"]["value"].as_str().unwrap(),
            "http://example.org/a1"
        );
    }

    #[test]
    fn invalid_query_errors() {
        let err = query(&docs(), "NOT SPARQL").unwrap_err();
        assert!(err.to_string().to_lowercase().contains("parse"));
    }
}
