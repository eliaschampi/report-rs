//! report-rs — motor PDF determinista (protocolo v1, patrón omr-rs).
//!
//! Frontera de proceso: stdin recibe UN manifest JSON, stdout emite UNA
//! respuesta JSON (nada más), stderr solo diagnósticos. Exit 0 = el lote
//! corrió (errores por ítem permitidos), exit 2 = invocación/manifest
//! inservible.

mod protocol;
mod render;
mod text;

use std::io::{Read, Write};
use std::process::ExitCode;
use std::time::Instant;

use protocol::{
    BatchResponse, ErrorCode, ItemError, ItemResult, ManifestError, Outcome, PROTOCOL_VERSION,
};
use text::{Fonts, ImageCache};

const MAX_STDIN_BYTES: usize = 16 * 1024 * 1024;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("report-rs: {}", error.0);
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, ManifestError> {
    let stdin = std::io::stdin().lock();
    let mut input = String::new();
    stdin
        .take(MAX_STDIN_BYTES as u64)
        .read_to_string(&mut input)
        .map_err(|e| ManifestError(format!("no se pudo leer stdin: {e}")))?;

    let manifest = protocol::parse_manifest(&input)?;
    let fonts = Fonts::load(&manifest.assets_dir)?;
    let mut images = ImageCache::new(manifest.assets_dir.clone());

    let mut results = Vec::with_capacity(manifest.documents.len());
    for document in &manifest.documents {
        let started = Instant::now();
        let outcome = render_document(document, &fonts, &mut images);
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        match &outcome {
            Outcome::Ok { bytes, .. } => eprintln!(
                "{}: ok {} bytes in {:.2} ms",
                document.id, bytes, elapsed_ms
            ),
            Outcome::Error { error } => {
                eprintln!("{}: {} ({:.2} ms)", document.id, error.code, elapsed_ms)
            }
        }
        results.push(ItemResult {
            id: document.id.clone(),
            outcome,
        });
    }

    emit_response(&BatchResponse {
        protocol_version: PROTOCOL_VERSION,
        results,
    })
}

fn render_document(
    document: &protocol::DocumentSpec,
    fonts: &Fonts,
    images: &mut ImageCache,
) -> Outcome {
    use protocol::DocumentKind;

    let rendered = match document.kind {
        DocumentKind::PaymentTicket => match document.ticket_payload() {
            Ok(payload) => render::ticket::render(&payload, &fonts.mono),
            Err(error) => Err(error),
        },
        DocumentKind::AttendanceDailyReport => match document.report_payload() {
            Ok(payload) => {
                let letterhead = payload
                    .letterhead
                    .as_deref()
                    .and_then(|file| images.get(file));
                render::attendance_daily::render(&payload, fonts, letterhead.as_ref())
            }
            Err(error) => Err(error),
        },
        DocumentKind::StudentCard => match document.card_payload() {
            Ok(payload) => {
                let template = payload
                    .template
                    .as_deref()
                    .and_then(|file| images.get(file));
                render::card::render(&payload, fonts, template.as_ref())
            }
            Err(error) => Err(error),
        },
    };
    let (bytes, pages) = match rendered {
        Ok(rendered) => rendered,
        Err((code, message)) => {
            return Outcome::Error {
                error: ItemError { code, message },
            };
        }
    };

    match std::fs::write(&document.out_path, &bytes) {
        Ok(()) => Outcome::Ok {
            bytes: bytes.len() as u64,
            pages,
        },
        Err(e) => Outcome::Error {
            error: ItemError {
                code: ErrorCode::WriteFailed,
                message: format!("no se pudo escribir {}: {e}", document.out_path.display()),
            },
        },
    }
}

fn emit_response(response: &BatchResponse) -> Result<ExitCode, ManifestError> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, response)
        .map_err(|e| ManifestError(format!("no se pudo serializar respuesta: {e}")))?;
    stdout
        .write_all(b"\n")
        .and_then(|()| stdout.flush())
        .map_err(|e| ManifestError(format!("no se pudo escribir stdout: {e}")))?;
    Ok(ExitCode::SUCCESS)
}
