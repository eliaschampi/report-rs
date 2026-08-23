//! `student_card` / `employee_card`: carnet CR80 — puerto completo de
//! student-card.service.ts con su geometría px→pt real (template 2032×1276).
//! QR vectorial ECC-H con zona de silencio, código rotado 90°, panel de
//! identidad con perfiles de nombre 9→5.5 pt, detalles en slots y foto con
//! panel de respaldo.

use krilla::Document;
use krilla::color::rgb;
use krilla::geom::{PathBuilder, Rect, Size, Transform};
use krilla::image::Image;
use krilla::metadata::Metadata;
use krilla::page::PageSettings;
use krilla::paint::Stroke;
use krilla::surface::Surface;

use crate::protocol::{ErrorCode, IdentityCardPayload};
use crate::text::{FontPair, MeasuredFont, solid_fill};

const PT_PER_MM: f64 = 72.0 / 25.4;
pub const CARD_W: f64 = 85.6 * PT_PER_MM;
pub const CARD_H: f64 = 53.98 * PT_PER_MM;

const TEMPLATE_W_PX: f64 = 2032.0;
const TEMPLATE_H_PX: f64 = 1276.0;

// Geometría del template, en píxeles del diseño (student-card.service.ts).
const QR_X_PX: f64 = 40.0;
const QR_Y_PX: f64 = 250.0;
const QR_SIZE_PX: f64 = 800.0;
const QR_MARGIN_MODULES: i32 = 1;
const CODE_CENTER_X_PX: f64 = 1040.0;
const PHOTO_X_PX: f64 = 1300.0;
const PHOTO_Y_PX: f64 = 110.0;
const PHOTO_SIZE_PX: f64 = 585.0;
const PANEL_W_PX: f64 = 660.0;
const PANEL_H_PX: f64 = 340.0;
const PANEL_TOP_PX: f64 = 720.0;
const PANEL_PAD_X_PX: f64 = 20.0;
const DETAIL_FONT_SIZE: f64 = 5.5;

const TEXT_COLOR: (f32, f32, f32) = (20.0 / 255.0, 29.0 / 255.0, 52.0 / 255.0);
const NAVY: (f32, f32, f32) = (49.0 / 255.0, 78.0 / 255.0, 234.0 / 255.0);
const PANEL: (f32, f32, f32) = (241.0 / 255.0, 246.0 / 255.0, 252.0 / 255.0);
const BORDER: (f32, f32, f32) = (220.0 / 255.0, 229.0 / 255.0, 239.0 / 255.0);
const QR_DARK: (f32, f32, f32) = (17.0 / 255.0, 24.0 / 255.0, 39.0 / 255.0);
const WHITE: (f32, f32, f32) = (1.0, 1.0, 1.0);

fn px_x(value: f64) -> f64 {
    value / TEMPLATE_W_PX * CARD_W
}

fn px_y(value: f64) -> f64 {
    value / TEMPLATE_H_PX * CARD_H
}

/// Conviención pdf-lib: Y del borde inferior de un rect cuyo top está a `top_px`.
fn rect_bottom_y(top_px: f64, height_px: f64) -> f64 {
    CARD_H - px_y(top_px + height_px)
}

pub fn render(
    payload: &IdentityCardPayload,
    fonts: &FontPair,
    template: Option<&Image>,
    photo: Option<&Image>,
) -> Result<(Vec<u8>, u32), (ErrorCode, String)> {
    if !(0.0..100.0).contains(&payload.code_font_size) {
        return Err((
            ErrorCode::PayloadInvalid,
            format!("code_font_size fuera de rango: {}", payload.code_font_size),
        ));
    }

    let mut document = Document::new();
    let mut metadata = Metadata::new();
    if let Some(title) = &payload.meta_title {
        metadata = metadata.title(title.clone());
    }
    if let Some(author) = &payload.meta_author {
        metadata = metadata.creator(author.clone());
    }
    if let Some(subject) = &payload.meta_subject {
        metadata = metadata.description(subject.clone());
    }
    document.set_metadata(metadata);

    let settings = PageSettings::from_wh(CARD_W as f32, CARD_H as f32).ok_or((
        ErrorCode::RenderFailed,
        "geometría CR80 inválida".to_owned(),
    ))?;
    let mut page = document.start_page_with(settings);
    let mut surface = page.surface();
    let bold = &fonts.bold;

    draw_background(&mut surface, template);
    draw_qr_and_code(&mut surface, bold, payload)?;
    draw_portrait_block(&mut surface, photo);
    draw_identity_info(&mut surface, bold, payload);

    surface.finish();
    page.finish();
    let bytes = document
        .finish()
        .map_err(|e| (ErrorCode::RenderFailed, format!("krilla: {e:?}")))?;
    Ok((bytes, 1))
}

fn draw_image_at(surface: &mut Surface, image: &Image, x: f64, y_top: f64, w: f64, h: f64) {
    surface.push_transform(&Transform::from_translate(x as f32, y_top as f32));
    if let Some(size) = Size::from_wh(w as f32, h as f32) {
        surface.draw_image(image.clone(), size);
    }
    surface.pop();
}

/// Rect en convención pdf-lib: (x, y_bottom) esquina inferior-izquierda.
struct CardRect {
    x: f64,
    y_bottom: f64,
    w: f64,
    h: f64,
}

fn stroke_rect(
    surface: &mut Surface,
    frame: CardRect,
    fill: (f32, f32, f32),
    border: (f32, f32, f32),
    border_width: f64,
) {
    let CardRect { x, y_bottom, w, h } = frame;
    let Some(rect) = Rect::from_xywh(x as f32, (CARD_H - y_bottom - h) as f32, w as f32, h as f32)
    else {
        return;
    };
    let mut builder = PathBuilder::new();
    builder.push_rect(rect);
    let Some(path) = builder.finish() else { return };
    surface.set_fill(Some(solid_fill(fill)));
    surface.set_stroke(Some(stroke(border, border_width)));
    surface.draw_path(&path);
}

fn stroke(color: (f32, f32, f32), width: f64) -> Stroke {
    Stroke {
        paint: rgb::Color::new(
            (color.0 * 255.0).round() as u8,
            (color.1 * 255.0).round() as u8,
            (color.2 * 255.0).round() as u8,
        )
        .into(),
        width: width as f32,
        ..Default::default()
    }
}

fn fill_text(surface: &mut Surface, color: (f32, f32, f32)) {
    surface.set_fill(Some(solid_fill(color)));
    surface.set_stroke(None);
}

fn draw_background(surface: &mut Surface, template: Option<&Image>) {
    match template {
        Some(image) => draw_image_at(surface, image, 0.0, 0.0, CARD_W, CARD_H),
        None => {
            let mut builder = PathBuilder::new();
            if let Some(rect) = Rect::from_xywh(0.0, 0.0, CARD_W as f32, CARD_H as f32) {
                builder.push_rect(rect);
            }
            if let Some(path) = builder.finish() {
                surface.set_fill(Some(solid_fill(WHITE)));
                surface.set_stroke(None);
                surface.draw_path(&path);
            }
        }
    }
}

/// QR vectorial ECC-H con zona de silencio (1 módulo, como el PNG del TS) +
/// código legible rotado 90° al costado del QR.
fn draw_qr_and_code(
    surface: &mut Surface,
    bold: &MeasuredFont,
    payload: &IdentityCardPayload,
) -> Result<(), (ErrorCode, String)> {
    use qrcodegen::{QrCode, QrCodeEcc};

    let qr = QrCode::encode_text(&payload.qr_text, QrCodeEcc::High).map_err(|e| {
        (
            ErrorCode::PayloadInvalid,
            format!("qr_text inválido: {e:?}"),
        )
    })?;

    let total_modules = qr.size() + QR_MARGIN_MODULES * 2;
    let scale = px_x(QR_SIZE_PX) / total_modules as f64;
    let mut builder = PathBuilder::new();
    for row in -QR_MARGIN_MODULES..qr.size() + QR_MARGIN_MODULES {
        for column in -QR_MARGIN_MODULES..qr.size() + QR_MARGIN_MODULES {
            let dark = (0..qr.size()).contains(&row)
                && (0..qr.size()).contains(&column)
                && qr.get_module(column, row);
            if dark {
                let x = px_x(QR_X_PX) + (column + QR_MARGIN_MODULES) as f64 * scale;
                let y = px_y(QR_Y_PX) + (row + QR_MARGIN_MODULES) as f64 * scale;
                if let Some(rect) = Rect::from_xywh(x as f32, y as f32, scale as f32, scale as f32)
                {
                    builder.push_rect(rect);
                }
            }
        }
    }
    if let Some(path) = builder.finish() {
        surface.set_fill(Some(solid_fill(QR_DARK)));
        surface.set_stroke(None);
        surface.draw_path(&path);
    }

    // Código rotado 90° (el TS usa degrees(90) en pdf-lib): se lee de abajo
    // hacia arriba junto al QR. Trasladar al pivote y luego rotar — rotar
    // alrededor del pivote mandaría el origen local fuera de la tarjeta.
    let code_text = if payload.space_code_characters {
        payload
            .code_text
            .chars()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        payload.code_text.clone()
    };
    let size = payload.code_font_size;
    let x = px_x(CODE_CENTER_X_PX) + size * 0.35;
    let y = CARD_H / 2.0 + 10.0;
    fill_text(surface, TEXT_COLOR);
    // Traslada al pivote y rota −90°: from_row(0,−1,1,0,x,y) ≡ T(x,y)·R(−90).
    surface.push_transform(&Transform::from_row(
        0.0, -1.0, 1.0, 0.0, x as f32, y as f32,
    ));
    bold.draw_line(surface, &code_text, size, 0.0, 0.0);
    surface.pop();
    Ok(())
}

fn draw_portrait_block(surface: &mut Surface, photo: Option<&Image>) {
    let x = px_x(PHOTO_X_PX);
    let w = px_x(PHOTO_SIZE_PX);
    let h = px_y(PHOTO_SIZE_PX);
    match photo {
        Some(image) => draw_image_at(surface, image, x, px_y(PHOTO_Y_PX), w, h),
        None => stroke_rect(
            surface,
            CardRect {
                x,
                y_bottom: rect_bottom_y(PHOTO_Y_PX, PHOTO_SIZE_PX),
                w,
                h,
            },
            PANEL,
            BORDER,
            1.0,
        ),
    }
}

/// Panel de identidad: nombre con perfiles tipográficos degradados y detalles
/// en slots verticales, todo centrado (puerto de drawIdentityInfo).
fn draw_identity_info(surface: &mut Surface, bold: &MeasuredFont, payload: &IdentityCardPayload) {
    let panel_x = PHOTO_X_PX + PHOTO_SIZE_PX / 2.0 - PANEL_W_PX / 2.0;
    stroke_rect(
        surface,
        CardRect {
            x: px_x(panel_x),
            y_bottom: rect_bottom_y(PANEL_TOP_PX, PANEL_H_PX),
            w: px_x(PANEL_W_PX),
            h: px_y(PANEL_H_PX),
        },
        WHITE,
        BORDER,
        0.8,
    );

    let name_block_x = px_x(panel_x + PANEL_PAD_X_PX);
    let name_block_w = px_x(PANEL_W_PX - PANEL_PAD_X_PX * 2.0);
    let layout = resolve_name_layout(&payload.full_name, bold, name_block_w);

    let name_center_y = CARD_H - px_y(PANEL_TOP_PX + PANEL_H_PX * 0.22);
    let mut line_y = name_center_y
        + (layout.font_size * 0.7 + (layout.lines.len() - 1) as f64 * layout.line_height) / 2.0
        - layout.font_size * 0.7;

    fill_text(surface, TEXT_COLOR);
    for line in &layout.lines {
        let width = bold.width(line, layout.font_size);
        bold.draw_line(
            surface,
            line,
            layout.font_size,
            name_block_x + (name_block_w - width) / 2.0,
            CARD_H - line_y,
        );
        line_y -= layout.line_height;
    }

    let detail_line_height = DETAIL_FONT_SIZE * 1.2;
    let detail_count = payload.details.len().max(1);
    fill_text(surface, NAVY);
    for (index, text) in payload.details.iter().enumerate() {
        let lines = attempt_wrap(text, bold, DETAIL_FONT_SIZE, name_block_w, 2).lines;
        let slot = if detail_count == 1 {
            0.7
        } else {
            0.5 + (0.9 - 0.5) * index as f64 / (detail_count - 1) as f64
        };
        let center_y = CARD_H - px_y(PANEL_TOP_PX + PANEL_H_PX * slot);
        let block_h =
            DETAIL_FONT_SIZE * 0.7 + (lines.len().saturating_sub(1)) as f64 * detail_line_height;
        let mut y = center_y + block_h / 2.0 - DETAIL_FONT_SIZE * 0.7;
        for line in &lines {
            let width = bold.width(line, DETAIL_FONT_SIZE);
            bold.draw_line(
                surface,
                line,
                DETAIL_FONT_SIZE,
                px_x(panel_x + PANEL_W_PX / 2.0) - width / 2.0,
                CARD_H - y,
            );
            y -= detail_line_height;
        }
    }
}

struct NameLayout {
    lines: Vec<String>,
    font_size: f64,
    line_height: f64,
}

/// Puerto de resolveNameLayout: prueba perfiles 9/1 → 8/1 → 7/2 → 6/2 → 5.5/2
/// y usa el primero que no trunca (o el último como límite).
fn resolve_name_layout(full_name: &str, bold: &MeasuredFont, max_width: f64) -> NameLayout {
    const PROFILES: [(f64, usize); 5] = [(9.0, 1), (8.0, 1), (7.0, 2), (6.0, 2), (5.5, 2)];
    for (size, max_lines) in PROFILES {
        let attempt = attempt_wrap(full_name, bold, size, max_width, max_lines);
        if !attempt.truncated || (size, max_lines) == PROFILES[PROFILES.len() - 1] {
            return NameLayout {
                lines: attempt.lines,
                font_size: size,
                line_height: size * 1.25,
            };
        }
    }
    NameLayout {
        lines: vec![full_name.to_owned()],
        font_size: 5.5,
        line_height: 7.0,
    }
}

struct WrapAttempt {
    lines: Vec<String>,
    truncated: bool,
}

/// Puerto de attemptWrap: greedy por palabras con límite de líneas (sin
/// ellipsis — el truncado se reporta para que el caller baje de perfil).
fn attempt_wrap(
    text: &str,
    font: &MeasuredFont,
    size: f64,
    max_width: f64,
    max_lines: usize,
) -> WrapAttempt {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return WrapAttempt {
            lines: Vec::new(),
            truncated: false,
        };
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in words {
        let candidate = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };
        if font.width(&candidate, size) <= max_width {
            current = candidate;
            continue;
        }
        if !current.is_empty() {
            lines.push(std::mem::take(&mut current));
        }
        if font.width(word, size) > max_width {
            lines.push(crate::text::fit(word, font, size, max_width));
        } else {
            current = word.to_owned();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }

    let limited: Vec<String> = lines.iter().take(max_lines).cloned().collect();
    let truncated = lines.len() > max_lines
        || limited
            .iter()
            .any(|line| font.width(line, size) > max_width);
    WrapAttempt {
        lines: limited,
        truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn bold() -> MeasuredFont {
        MeasuredFont::load(Path::new("assets/fonts/LiberationSans-Bold.ttf")).unwrap()
    }

    #[test]
    fn name_layout_prefers_larger_profile_that_fits() {
        let bold = bold();
        let short = resolve_name_layout("Valeria Ramírez", &bold, 100.0);
        assert_eq!(short.font_size, 9.0);
        assert_eq!(short.lines.len(), 1);

        let long = resolve_name_layout("María Fernanda Del Águila Villanueva Osorio", &bold, 40.0);
        assert!(long.font_size < 9.0);
        assert!(long.lines.len() <= 2);
    }

    #[test]
    fn attempt_wrap_reports_truncation() {
        let bold = bold();
        let one_line = attempt_wrap("Hugo Alejandro Ramírez Sotomayor", &bold, 8.0, 110.0, 1);
        assert!(one_line.truncated);
        let two_lines = attempt_wrap("Hugo Alejandro Ramírez Sotomayor", &bold, 8.0, 110.0, 2);
        assert!(!two_lines.truncated);
        assert_eq!(two_lines.lines.len(), 2);
    }
}
