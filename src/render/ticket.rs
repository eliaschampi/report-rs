//! `payment_ticket`: ticket térmico 80 mm, filas declarativas, altura variable.
//! Puerto de payment-ticket.service.ts — misma matemática de layout, mismas
//! constantes físicas del formato (el contenido vive 100 % en el payload).

use krilla::Document;
use krilla::color::rgb;
use krilla::geom::{PathBuilder, Point};
use krilla::metadata::Metadata;
use krilla::page::PageSettings;
use krilla::paint::{Fill, Stroke};
use krilla::text::TextDirection;

use crate::protocol::{Align, TicketPayload, TicketRow};
use crate::text::{FontPair, wrap};

// Constantes físicas del formato térmico 80 mm (idénticas al TS).
pub const WIDTH: f64 = 226.77;
pub const MARGIN: f64 = 14.0;
const BODY_SIZE: f64 = 8.0;
const LINE_HEIGHT: f64 = 12.0;
const MIN_HEIGHT: f64 = 240.0;
const TOP_PAD: f64 = 20.0;
const BASE_HEIGHT: f64 = 34.0;

const TEXT_COLOR: (u8, u8, u8) = (20, 20, 20);
const RULE_COLOR: (u8, u8, u8) = (89, 89, 89);

fn wrapped_rows(rows: &[TicketRow], fonts: &FontPair) -> Vec<TicketRow> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(text) = row
            .text
            .as_deref()
            .filter(|t| !row.separator && !t.is_empty())
        else {
            out.push(row.clone());
            continue;
        };
        let font = fonts.pick(row.bold);
        let size = row.size.unwrap_or(BODY_SIZE);
        let max_width = WIDTH - MARGIN * 2.0;
        let lines = wrap(text, font, size, max_width);
        if lines.is_empty() {
            out.push(row.clone());
            continue;
        }
        for (index, line) in lines.iter().enumerate() {
            let mut expanded = row.clone();
            expanded.text = Some(line.clone());
            expanded.gap_after = if index == lines.len() - 1 {
                row.gap_after
            } else {
                0.0
            };
            out.push(expanded);
        }
    }
    out
}

fn x_for(text_width: f64, align: Align) -> f64 {
    match align {
        Align::Left => MARGIN,
        Align::Center => (WIDTH - text_width) / 2.0,
        Align::Right => WIDTH - MARGIN - text_width,
    }
}

pub fn render(
    payload: &TicketPayload,
    fonts: &FontPair,
) -> Result<(Vec<u8>, u32), (crate::protocol::ErrorCode, String)> {
    use crate::protocol::ErrorCode;

    for (index, row) in payload.rows.iter().enumerate() {
        if let Some(size) = row.size
            && !(0.0..200.0).contains(&size)
        {
            return Err((
                ErrorCode::PayloadInvalid,
                format!("rows[{index}].size fuera de rango: {size}"),
            ));
        }
        if row.gap_after < 0.0 {
            return Err((
                ErrorCode::PayloadInvalid,
                format!("rows[{index}].gap_after negativo"),
            ));
        }
    }

    let rows = wrapped_rows(&payload.rows, fonts);
    // Altura final idéntica al TS: 34 + Σ(line_height + gap_after), mínimo 240.
    let height = (BASE_HEIGHT
        + rows
            .iter()
            .map(|row| LINE_HEIGHT + row.gap_after)
            .sum::<f64>())
    .max(MIN_HEIGHT);

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

    let settings = PageSettings::from_wh(WIDTH as f32, height as f32).ok_or_else(|| {
        (
            ErrorCode::RenderFailed,
            format!("geometría de página inválida: {WIDTH}x{height}"),
        )
    })?;
    let mut page = document.start_page_with(settings);
    let mut surface = page.surface();

    let text_fill = Fill {
        paint: rgb::Color::new(TEXT_COLOR.0, TEXT_COLOR.1, TEXT_COLOR.2).into(),
        ..Default::default()
    };
    let rule_stroke = Stroke {
        paint: rgb::Color::new(RULE_COLOR.0, RULE_COLOR.1, RULE_COLOR.2).into(),
        width: 0.5,
        ..Default::default()
    };

    // El layout se calcula en convención PDF (origen abajo-izquierda, y hacia
    // arriba, idéntico al TS de origen) y se convierte al dibujar: krilla usa
    // origen arriba-izquierda con y hacia abajo.
    let mut y = height - TOP_PAD;
    for row in &rows {
        if row.separator {
            let rule_y = height - (y + 4.0);
            let mut builder = PathBuilder::new();
            builder.move_to(MARGIN as f32, rule_y as f32);
            builder.line_to((WIDTH - MARGIN) as f32, rule_y as f32);
            if let Some(path) = builder.finish() {
                surface.set_fill(None);
                surface.set_stroke(Some(rule_stroke.clone()));
                surface.draw_path(&path);
            }
        } else if let Some(text) = row.text.as_deref() {
            let font = fonts.pick(row.bold);
            let size = row.size.unwrap_or(BODY_SIZE);
            let x = x_for(font.width(text, size), row.align);
            surface.set_fill(Some(text_fill.clone()));
            surface.set_stroke(None);
            surface.draw_text(
                Point::from_xy(x as f32, (height - y) as f32),
                font.krilla_font(),
                size as f32,
                text,
                false,
                TextDirection::Auto,
            );
        }
        y -= LINE_HEIGHT + row.gap_after;
    }

    surface.finish();
    page.finish();
    let bytes = document
        .finish()
        .map_err(|e| (ErrorCode::RenderFailed, format!("krilla: {e:?}")))?;
    Ok((bytes, 1))
}
