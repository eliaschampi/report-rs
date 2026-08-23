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
    /// Directorio con fuentes (assets/fonts) e imágenes de marca (membrete,
    /// templates).
    pub assets_dir: PathBuf,
    /// Directorio con archivos de entrada del job (p. ej. la foto del
    /// carnet): el caller lo crea con modo 0700 y lo limpia en `finally`.
    #[serde(default)]
    pub input_dir: Option<PathBuf>,
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
    /// Payload sin tipar aquí: se deserializa al tipo de `kind` con
    /// `deny_unknown_fields` en cada acceso (`ticket_payload` etc.).
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    PaymentTicket,
    AttendanceDailyReport,
    StudentCard,
    EmployeeCard,
}

impl DocumentSpec {
    fn typed_payload<T: serde::de::DeserializeOwned>(
        &self,
        expected: DocumentKind,
    ) -> Result<T, (ErrorCode, String)> {
        if self.kind != expected {
            return Err((
                ErrorCode::PayloadInvalid,
                format!(
                    "kind {} no corresponde a este payload (esperado {})",
                    self.kind.as_str(),
                    expected.as_str()
                ),
            ));
        }
        serde_json::from_value(self.payload.clone()).map_err(|e| {
            (
                ErrorCode::PayloadInvalid,
                format!("payload no cumple el contrato: {e}"),
            )
        })
    }

    pub fn ticket_payload(&self) -> Result<TicketPayload, (ErrorCode, String)> {
        self.typed_payload(DocumentKind::PaymentTicket)
    }

    pub fn report_payload(&self) -> Result<AttendanceDailyPayload, (ErrorCode, String)> {
        self.typed_payload(DocumentKind::AttendanceDailyReport)
    }

    /// Carnet de identidad: alumno y personal comparten payload/renderizador.
    pub fn card_payload(&self) -> Result<IdentityCardPayload, (ErrorCode, String)> {
        if !matches!(
            self.kind,
            DocumentKind::StudentCard | DocumentKind::EmployeeCard
        ) {
            return Err((
                ErrorCode::PayloadInvalid,
                format!(
                    "kind {} no corresponde a un carnet de identidad",
                    self.kind.as_str()
                ),
            ));
        }
        self.typed_payload(self.kind)
    }
}

impl DocumentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocumentKind::PaymentTicket => "payment_ticket",
            DocumentKind::AttendanceDailyReport => "attendance_daily_report",
            DocumentKind::StudentCard => "student_card",
            DocumentKind::EmployeeCard => "employee_card",
        }
    }
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

/// Paleta cerrada del membrete de marca (los colores no viajan por el
/// manifest: son constantes de la identidad, no contenido).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaletteColor {
    #[default]
    Text,
    Muted,
    Success,
    Warning,
    Danger,
    Info,
    Accent,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttendanceDailyPayload {
    pub title: String,
    pub subtitle: String,
    pub summary: Vec<SummaryItem>,
    pub columns: Vec<ColumnSpec>,
    #[serde(default)]
    pub rows: Vec<Vec<Cell>>,
    /// Estado vacío obligatorio si `rows` viene vacío.
    pub empty: Option<EmptyState>,
    /// PNG dentro de assets_dir usado como fondo de página (opcional).
    #[serde(default)]
    pub letterhead: Option<String>,
    #[serde(default)]
    pub meta_title: Option<String>,
    #[serde(default)]
    pub meta_author: Option<String>,
    #[serde(default)]
    pub meta_subject: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryItem {
    pub label: String,
    pub value: String,
    #[serde(default)]
    pub color: PaletteColor,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnSpec {
    pub label: String,
    pub width: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    pub text: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub color: PaletteColor,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyState {
    pub title: String,
    pub subtitle: String,
}

/// Carnet CR80 de identidad: alumno (`student_card`) y personal
/// (`employee_card`) comparten renderizador; el kind distingue el semántico.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityCardPayload {
    /// PNG template CR80 dentro de assets_dir (opcional: sin él, fondo blanco).
    #[serde(default)]
    pub template: Option<String>,
    /// Foto PNG/JPEG dentro de input_dir (opcional: sin ella, panel de respaldo).
    #[serde(default)]
    pub photo: Option<String>,
    pub full_name: String,
    pub qr_text: String,
    pub code_text: String,
    /// 13 para roll codes de alumno, 5.5 para números de personal.
    pub code_font_size: f64,
    #[serde(default)]
    pub space_code_characters: bool,
    #[serde(default)]
    pub details: Vec<String>,
    #[serde(default)]
    pub meta_title: Option<String>,
    #[serde(default)]
    pub meta_author: Option<String>,
    #[serde(default)]
    pub meta_subject: Option<String>,
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

    fn report_document(id: &str, row: &str) -> String {
        let columns = r#"{"label":"Alumno","width":100}"#;
        format!(
            r#"{{"id":"{id}","kind":"attendance_daily_report","out_path":"o.pdf","payload":{{"title":"T","subtitle":"S","summary":[{{"label":"A","value":"B"}}],"columns":[{columns}],"rows":[{row}]}}}}"#
        )
    }

    #[test]
    fn rejects_unknown_fields() {
        let raw = r#"{"protocol_version":1,"assets_dir":"a","documents":[],"extra":1}"#;
        assert!(parse_manifest(raw).is_err());
    }

    #[test]
    fn rejects_wrong_version_and_empty_batch() {
        let raw = r#"{"protocol_version":2,"assets_dir":"a","documents":[]}"#;
        assert!(parse_manifest(raw).is_err());
        assert!(parse_manifest(&manifest_json("")).is_err());
    }

    #[test]
    fn rejects_duplicate_ids() {
        let doc = ticket_document("a", "a.pdf");
        assert!(parse_manifest(&manifest_json(&format!("{doc},{doc}"))).is_err());
    }

    #[test]
    fn accepts_valid_manifests() {
        let manifest = parse_manifest(&manifest_json(&ticket_document("a", "a.pdf"))).unwrap();
        assert_eq!(manifest.documents[0].kind, DocumentKind::PaymentTicket);
        assert!(manifest.documents[0].ticket_payload().is_ok());

        let row = r#"[{"text":"Valeria","bold":true}]"#;
        let manifest = parse_manifest(&manifest_json(&report_document("r", row))).unwrap();
        assert_eq!(
            manifest.documents[0].kind,
            DocumentKind::AttendanceDailyReport
        );
        assert_eq!(
            manifest.documents[0].report_payload().unwrap().rows.len(),
            1
        );
    }

    #[test]
    fn rejects_payload_unknown_fields_and_wrong_kind() {
        let raw = r#"{"id":"x","kind":"payment_ticket","out_path":"o.pdf","payload":{"rows":[],"extra":1}}"#;
        let manifest = parse_manifest(&manifest_json(raw)).unwrap();
        assert!(manifest.documents[0].ticket_payload().is_err());

        // payload de reporte bajo kind de ticket: tipos cruzados → error
        let row = r#"{"text":"V"}"#;
        let manifest = parse_manifest(&manifest_json(&report_document("x", row))).unwrap();
        assert!(manifest.documents[0].ticket_payload().is_err());
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
