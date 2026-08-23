//! `attendance_daily_report`: reporte A4 de asistencia diaria. Puerto de
//! attendance-daily-report.service.ts sobre el kernel `a4`.

use krilla::Document;
use krilla::image::Image;
use krilla::metadata::Metadata;

use crate::protocol::{AttendanceDailyPayload, ErrorCode, PaletteColor};
use crate::render::a4::{self, PreparedCell};
use crate::text::{Fonts, wrap};

const CELL_PADDING: f64 = 12.0;
const FALLBACK: &str = "—";

pub fn render(
    payload: &AttendanceDailyPayload,
    fonts: &Fonts,
    letterhead: Option<&Image>,
) -> Result<(Vec<u8>, u32), (ErrorCode, String)> {
    validate(payload).map_err(|message| (ErrorCode::PayloadInvalid, message))?;

    let specs: Vec<(String, f64)> = payload
        .columns
        .iter()
        .map(|column| (column.label.clone(), column.width))
        .collect();
    let columns = a4::build_columns(&specs);
    let sans = &fonts.sans;

    let mut document = Document::new();
    let mut metadata = Metadata::new();
    if let Some(title) = &payload.meta_title {
        metadata = metadata.title(title.clone());
    }
    if let Some(subject) = &payload.meta_subject {
        metadata = metadata.description(subject.clone());
    }
    if let Some(author) = &payload.meta_author {
        metadata = metadata.creator(author.clone());
    }
    document.set_metadata(metadata);

    // Primera página: fondo + tarjeta resumen + header de tabla.
    let mut page = document.start_page_with(a4::page_settings());
    let mut surface = page.surface();
    a4::draw_background(&mut surface, letterhead);
    let summary_bottom = a4::draw_summary(
        &mut surface,
        &payload.title,
        &payload.subtitle,
        &payload.summary,
        sans,
    );
    let mut cursor = a4::draw_table_header(&mut surface, &columns, summary_bottom - 14.0, sans);
    let mut pages = 1u32;

    if payload.rows.is_empty() {
        // validate() garantiza `empty` cuando no hay filas.
        let empty = payload.empty.as_ref().expect("validate exigió empty");
        a4::draw_empty_state(&mut surface, cursor, &empty.title, &empty.subtitle, sans);
    } else {
        for (row_index, cells) in payload.rows.iter().enumerate() {
            let prepared: Vec<PreparedCell> = cells
                .iter()
                .zip(&columns)
                .map(|(cell, column)| PreparedCell {
                    lines: wrapped_cell(&cell.text, sans.pick(cell.bold), column.width),
                    bold: cell.bold,
                    color: cell.color,
                })
                .collect();
            let height = a4::row_height(&prepared);

            if cursor - height < a4::TABLE_BOTTOM_Y {
                surface.finish();
                page.finish();
                page = document.start_page_with(a4::page_settings());
                surface = page.surface();
                a4::draw_background(&mut surface, letterhead);
                a4::draw_title(&mut surface, &payload.title, 14.0, 78.0, sans);
                cursor = a4::draw_table_header(
                    &mut surface,
                    &columns,
                    a4::CONTINUATION_TABLE_TOP_Y,
                    sans,
                );
                pages += 1;
            }

            cursor = a4::draw_table_row(&mut surface, &columns, &prepared, cursor, row_index, sans);
        }
    }

    surface.finish();
    page.finish();
    let bytes = document
        .finish()
        .map_err(|e| (ErrorCode::RenderFailed, format!("krilla: {e:?}")))?;
    Ok((bytes, pages))
}

/// wrapPdfText del kernel: texto vacío produce la línea fallback.
fn wrapped_cell(text: &str, font: &crate::text::MeasuredFont, column_width: f64) -> Vec<String> {
    let lines = wrap(text, font, a4::TABLE_FONT_SIZE, column_width - CELL_PADDING);
    if lines.is_empty() {
        vec![FALLBACK.to_owned()]
    } else {
        lines
    }
}

fn validate(payload: &AttendanceDailyPayload) -> Result<(), String> {
    if payload.columns.is_empty() {
        return Err("columns no puede estar vacío".into());
    }
    if payload.summary.is_empty() {
        return Err("summary no puede estar vacío".into());
    }
    let total_width: f64 = payload.columns.iter().map(|c| c.width).sum();
    if total_width > a4::CONTENT_W + 0.01 {
        return Err(format!(
            "la suma de anchos de columns ({total_width:.2}) excede el ancho útil ({:.2})",
            a4::CONTENT_W
        ));
    }
    for (index, cells) in payload.rows.iter().enumerate() {
        if cells.len() != payload.columns.len() {
            return Err(format!(
                "rows[{index}] tiene {} celdas, columns declara {}",
                cells.len(),
                payload.columns.len()
            ));
        }
    }
    if payload.rows.is_empty() && payload.empty.is_none() {
        return Err("rows vacío requiere empty (estado vacío)".into());
    }
    let _ = PaletteColor::Text;
    Ok(())
}
