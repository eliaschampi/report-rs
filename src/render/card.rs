//! `student_card`: sonda de factibilidad R3 — CR80 con template PNG, QR y
//! etiqueta rotada 90°. Las posiciones son de PRUEBA: el mapeo px→pt real del
//! template queda para R3 (leer student-card.service.ts). Lo que este módulo
//! demuestra: embebido de template, generación de QR nativa, texto rotado y
//! su costo en tiempo/bytes.

use krilla::Document;
use krilla::color::rgb;
use krilla::geom::{PathBuilder, Rect, Size, Transform};
use krilla::image::Image;
use krilla::metadata::Metadata;
use krilla::page::PageSettings;
use krilla::paint::Stroke;
use krilla::surface::Surface;

use crate::protocol::{ErrorCode, PaletteColor, StudentCardPayload};
use crate::render::a4::palette;
use crate::text::{Fonts, solid_fill};

// CR80 en puntos: 85.60mm × 53.98mm.
pub const CARD_W: f64 = 242.645;
pub const CARD_H: f64 = 153.080;

const QR_SIZE: f64 = 27.0;
const QR_X: f64 = 12.0;
const QR_Y: f64 = 114.0;
const NAME_X: f64 = 46.0;
const NAME_Y: f64 = 118.0;
const CODE_X: f64 = 46.0;
const CODE_Y: f64 = 130.0;
const DOC_LABEL_X: f64 = 46.0;
const DOC_LABEL_Y: f64 = 98.0;
const DOC_VALUE_X: f64 = 46.0;
const DOC_VALUE_Y: f64 = 109.0;

pub fn render(
    payload: &StudentCardPayload,
    fonts: &Fonts,
    template: Option<&Image>,
) -> Result<(Vec<u8>, u32), (ErrorCode, String)> {
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
    let sans = &fonts.sans;

    match template {
        Some(image) => {
            surface.push_transform(&Transform::from_translate(0.0, 0.0));
            surface.draw_image(
                image.clone(),
                Size::from_wh(CARD_W as f32, CARD_H as f32).unwrap(),
            );
            surface.pop();
        }
        None => {
            let rect = Rect::from_xywh(0.5, 0.5, CARD_W as f32 - 1.0, CARD_H as f32 - 1.0)
                .ok_or((ErrorCode::RenderFailed, "margen CR80 inválido".to_owned()))?;
            let mut builder = PathBuilder::new();
            builder.push_rect(rect);
            if let Some(path) = builder.finish() {
                surface.set_fill(Some(solid_fill((1.0, 1.0, 1.0))));
                surface.set_stroke(Some(Stroke {
                    paint: rgb::Color::new(200, 203, 209).into(),
                    width: 1.0,
                    ..Default::default()
                }));
                surface.draw_path(&path);
            }
        }
    }

    // Etiqueta vertical (90°) — prueba de texto rotado para R3.
    surface.push_transform(&Transform::from_rotate_at(
        90.0,
        (CARD_W - 9.0) as f32,
        (CARD_H - 12.0) as f32,
    ));
    sans.bold.draw_line(&mut surface, "ALUMNO", 6.5, 0.0, 0.0);
    surface.pop();

    // Datos del alumno.
    draw_field(
        &mut surface,
        &sans.bold,
        &payload.full_name,
        9.5,
        NAME_X,
        NAME_Y,
        PaletteColor::Text,
    );
    draw_field(
        &mut surface,
        &sans.regular,
        &payload.student_code,
        7.5,
        CODE_X,
        CODE_Y,
        PaletteColor::Muted,
    );
    draw_field(
        &mut surface,
        &sans.regular,
        &payload.document_label.to_uppercase(),
        6.5,
        DOC_LABEL_X,
        DOC_LABEL_Y,
        PaletteColor::Muted,
    );
    draw_field(
        &mut surface,
        &sans.bold,
        &payload.document_value,
        9.2,
        DOC_VALUE_X,
        DOC_VALUE_Y,
        PaletteColor::Text,
    );

    draw_qr(&mut surface, &payload.qr_text, QR_X, QR_Y, QR_SIZE).map_err(|message| {
        (
            ErrorCode::PayloadInvalid,
            format!("qr_text no se puede codificar: {message}"),
        )
    })?;

    surface.finish();
    page.finish();
    let bytes = document
        .finish()
        .map_err(|e| (ErrorCode::RenderFailed, format!("krilla: {e:?}")))?;
    Ok((bytes, 1))
}

fn draw_field(
    surface: &mut Surface,
    font: &crate::text::MeasuredFont,
    text: &str,
    size: f64,
    x: f64,
    y: f64,
    color: PaletteColor,
) {
    surface.set_fill(Some(solid_fill(palette(color))));
    surface.set_stroke(None);
    font.draw_line(surface, text, size, x, y);
}

/// QR como un solo path relleno (un rect por módulo oscuro) — sin rasterizar,
/// vectorial, nítido a cualquier escala.
fn draw_qr(surface: &mut Surface, text: &str, x: f64, y: f64, size: f64) -> Result<(), String> {
    use qrcodegen::{QrCode, QrCodeEcc};

    let qr = QrCode::encode_text(text, QrCodeEcc::Medium).map_err(|e| format!("{e:?}"))?;
    let modules = qr.size() as f64;
    let scale = size / modules;
    let mut builder = PathBuilder::new();
    for row in 0..qr.size() {
        for column in 0..qr.size() {
            if qr.get_module(column, row) {
                let rx = x + column as f64 * scale;
                let ry = y + row as f64 * scale;
                if let Some(rect) =
                    Rect::from_xywh(rx as f32, ry as f32, scale as f32, scale as f32)
                {
                    builder.push_rect(rect);
                }
            }
        }
    }
    if let Some(path) = builder.finish() {
        surface.set_fill(Some(solid_fill((0.08, 0.08, 0.08))));
        surface.set_stroke(None);
        surface.draw_path(&path);
    }
    Ok(())
}
