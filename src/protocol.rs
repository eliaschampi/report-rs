//! Protocolo v1: un manifest JSON entra por stdin, una respuesta JSON sale por stdout.
//! Contrato espejo del patrón omr-rs: `deny_unknown_fields`, echo de `id`,
//! orden preservado, errores por ítem que nunca tumban el lote.

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_BATCH_DOCUMENTS: usize = 16;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub protocol_version: u32,
    /// Directorio con las fuentes (LiberationMono-{Regular,Bold}.ttf) y assets.
    pub assets_dir: PathBuf,
    pub documents: Vec<DocumentSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentSpec {
    /// Opaco para el motor: se hace eco en la respuesta tal cual llegó.
    pub id: String,
    pub kind: DocumentKind,
    /// El PDF se escribe aquí; el caller es dueño del ciclo de vida del archivo.
    pub out_path: PathBuf,
    pub payload: TicketPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    PaymentTicket,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TicketPayload {
    pub rows: Vec<TicketRow>,
    #[serde(default)]
    pub meta_title: Option<String>,
    #[serde(default)]
    pub meta_author: Option<String>,
    #[serde(default)]
    pub meta_subject: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TicketRow {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub align: Align,
    #[serde(default)]
    pub separator: bool,
    #[serde(default)]
    pub gap_after: f64,
}

#[derive(Debug, Serialize)]
pub struct BatchResponse {
    pub protocol_version: u32,
    pub results: Vec<ItemResult>,
}

#[derive(Debug, Serialize)]
pub struct ItemResult {
    pub id: String,
    #[serde(flatten)]
    pub outcome: Outcome,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Outcome {
    Ok { bytes: u64, pages: u32 },
    Error { error: ItemError },
}

#[derive(Debug, Serialize)]
pub struct ItemError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    PayloadInvalid,
    RenderFailed,
    WriteFailed,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = serde_json::to_value(self).ok();
        write!(
            f,
            "{}",
            name.and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_else(|| "UNKNOWN".into())
        )
    }
}

/// Errores de manifest: abortan el proceso (exit 2), sin escribir nada en stdout.
#[derive(Debug)]
pub struct ManifestError(pub String);

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn parse_manifest(input: &str) -> Result<Manifest, ManifestError> {
    let manifest: Manifest = serde_json::from_str(input)
        .map_err(|e| ManifestError(format!("manifest JSON inválido: {e}")))?;

    if manifest.protocol_version != PROTOCOL_VERSION {
        return Err(ManifestError(format!(
            "protocol_version no soportado: {} (esperado {PROTOCOL_VERSION})",
            manifest.protocol_version
        )));
    }

    let n = manifest.documents.len();
    if n == 0 || n > MAX_BATCH_DOCUMENTS {
        return Err(ManifestError(format!(
            "documents fuera de rango: {n} (esperado 1..={MAX_BATCH_DOCUMENTS})"
        )));
    }

    for (index, document) in manifest.documents.iter().enumerate() {
        if document.id.trim().is_empty() {
            return Err(ManifestError(format!("documents[{index}].id vacío")));
        }
        if document.out_path.as_os_str().is_empty() {
            return Err(ManifestError(format!(
                "documents[{index}].out_path vacío (id={})",
                document.id
            )));
        }
        if manifest.documents[..index]
            .iter()
            .any(|d| d.id == document.id)
        {
            return Err(ManifestError(format!(
                "documents[{index}].id duplicado: {}",
                document.id
            )));
        }
    }

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_json(documents: &str) -> String {
        format!(r#"{{"protocol_version":1,"assets_dir":"assets","documents":[{documents}]}}"#)
    }

    fn ticket_document(id: &str, out: &str) -> String {
        format!(
            r#"{{"id":"{id}","kind":"payment_ticket","out_path":"{out}","payload":{{"rows":[{{"text":"X"}}]}}}}"#
        )
    }

    #[test]
    fn rejects_unknown_fields() {
        let raw = r#"{"protocol_version":1,"assets_dir":"a","documents":[],"extra":1}"#;
        assert!(parse_manifest(raw).is_err());
    }

    #[test]
    fn rejects_wrong_version_and_empty_batch() {
        let raw = r#"{"protocol_version":2,"assets_dir":"a","documents":[{"id":"x","kind":"payment_ticket","out_path":"x.pdf","payload":{"rows":[]}}]}"#;
        assert!(parse_manifest(raw).is_err());
        assert!(parse_manifest(&manifest_json("")).is_err());
    }

    #[test]
    fn rejects_duplicate_ids() {
        let doc = ticket_document("a", "a.pdf");
        assert!(parse_manifest(&manifest_json(&format!("{doc},{doc}"))).is_err());
    }

    #[test]
    fn accepts_valid_manifest() {
        let doc = ticket_document("a", "a.pdf");
        let manifest = parse_manifest(&manifest_json(&doc)).unwrap();
        assert_eq!(manifest.documents.len(), 1);
        assert_eq!(manifest.documents[0].kind, DocumentKind::PaymentTicket);
    }

    #[test]
    fn error_code_serializes_screaming_snake_case() {
        let item = ItemResult {
            id: "doc-1".into(),
            outcome: Outcome::Error {
                error: ItemError {
                    code: ErrorCode::PayloadInvalid,
                    message: "rows[3]: texto vacío".into(),
                },
            },
        };
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["status"], "error");
        assert_eq!(json["error"]["code"], "PAYLOAD_INVALID");
    }
}
