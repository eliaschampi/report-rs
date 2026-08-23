//! Kernel A4: membrete, títulos, tarjeta resumen, tablas con zebra,
//! separadores y paginación con header repetido. Puerto de
//! pdf-a4-report.service.ts — misma geometría, misma paleta.
//!
//! Convención: TODO el layout se calcula como en el TS (origen abajo-izquierda,
//! y hacia arriba) y se convierte a krilla (origen arriba-izquierda) al dibujar.

use krilla::color::rgb;
use krilla::geom::{PathBuilder, Rect, Size, Transform};
use krilla::image::Image;
use krilla::page::PageSettings;
use krilla::paint::Stroke;
use krilla::surface::Surface;

use crate::protocol::{PaletteColor, SummaryItem};
use crate::text::{FontPair, MeasuredFont, fit, limited_wrap, solid_fill, wrap};

pub const PAGE_W: f64 = 595.28;
pub const PAGE_H: f64 = 841.89;
pub const CONTENT_X: f64 = 48.0;
pub const CONTENT_W: f64 = PAGE_W - CONTENT_X * 2.0;
pub const TITLE_Y: f64 = 734.0;
pub const CONTINUATION_TABLE_TOP_Y: f64 = 712.0;
pub const TABLE_BOTTOM_Y: f64 = 104.0;
pub const TABLE_FONT_SIZE: f64 = 9.0;
pub const TABLE_LINE_HEIGHT: f64 = 11.0;
pub const TABLE_HEADER_HEIGHT: f64 = 26.0;
pub const MIN_ROW_HEIGHT: f64 = 32.0;

pub struct Column {
    pub label: String,
    pub x: f64,
    pub width: f64,
}

/// Resuelve las X acumuladas de las columnas (buildA4ReportTableColumns).
pub fn build_columns(specs: &[(String, f64)]) -> Vec<Column> {
    let mut x = CONTENT_X;
    specs
        .iter()
        .map(|(label, width)| {
            let column = Column {
                label: label.clone(),
                x,
                width: *width,
            };
            x += width;
            column
        })
        .collect()
}

pub fn palette(color: PaletteColor) -> (f32, f32, f32) {
    match color {
        PaletteColor::Text => (0.12, 0.13, 0.16),
        PaletteColor::Muted => (0.43, 0.46, 0.52),
        PaletteColor::Success => (0.12, 0.62, 0.31),
        PaletteColor::Warning => (0.85, 0.55, 0.10),
        PaletteColor::Danger => (0.84, 0.18, 0.18),
        PaletteColor::Info => (0.34, 0.41, 0.52),
        PaletteColor::Accent => (0.84, 0.16, 0.12),
    }
}

const BORDER: (f32, f32, f32) = (0.85, 0.87, 0.90);
const PANEL_BORDER: (f32, f32, f32) = (0.88, 0.89, 0.92);
const SURFACE: (f32, f32, f32) = (0.984, 0.986, 0.991);
const HEADER_SURFACE: (f32, f32, f32) = (0.965, 0.967, 0.972);
const WHITE: (f32, f32, f32) = (1.0, 1.0, 1.0);

// ---------- primitivas de dibujo (convención pdf-lib → krilla) ----------

fn y_k(page_h: f64, y: f64) -> f64 {
    page_h - y
}

pub fn draw_text(
    surface: &mut Surface,
    font: &MeasuredFont,
    text: &str,
    size: f64,
    x: f64,
    y: f64,
    color: (f32, f32, f32),
) {
    surface.set_fill(Some(solid_fill(color)));
    surface.set_stroke(None);
    font.draw_line(surface, text, size, x, y_k(PAGE_H, y));
}

fn stroke_line(
    surface: &mut Surface,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    width: f64,
    color: (f32, f32, f32),
) {
    let mut builder = PathBuilder::new();
    builder.move_to(x1 as f32, y_k(PAGE_H, y1) as f32);
    builder.line_to(x2 as f32, y_k(PAGE_H, y2) as f32);
    if let Some(path) = builder.finish() {
        surface.set_fill(None);
        surface.set_stroke(Some(Stroke {
            paint: rgb::Color::new(
                (color.0 * 255.0) as u8,
                (color.1 * 255.0) as u8,
                (color.2 * 255.0) as u8,
            )
            .into(),
            width: width as f32,
            ..Default::default()
        }));
        surface.draw_path(&path);
    }
}

/// Rectángulo en convención pdf-lib: (x, y) es la esquina inferior-izquierda
/// y el rectángulo sube `h`.
struct Frame {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

fn fill_rect(surface: &mut Surface, frame: Frame, color: (f32, f32, f32)) {
    stroke_rect(surface, frame, color, None, 0.0);
}

fn stroke_rect(
    surface: &mut Surface,
    frame: Frame,
    fill: (f32, f32, f32),
    border: Option<(f32, f32, f32)>,
    border_width: f64,
) {
    let Frame { x, y, w, h } = frame;
    let rect = Rect::from_xywh(x as f32, y_k(PAGE_H, y + h) as f32, w as f32, h as f32);
    let Some(rect) = rect else { return };
    let mut builder = PathBuilder::new();
    builder.push_rect(rect);
    let Some(path) = builder.finish() else { return };
    surface.set_fill(Some(solid_fill(fill)));
    match border {
        Some(border_color) => {
            surface.set_stroke(Some(Stroke {
                paint: rgb::Color::new(
                    (border_color.0 * 255.0) as u8,
                    (border_color.1 * 255.0) as u8,
                    (border_color.2 * 255.0) as u8,
                )
                .into(),
                width: border_width as f32,
                ..Default::default()
            }));
        }
        None => surface.set_stroke(None),
    }
    surface.draw_path(&path);
}

// ---------- página base ----------

/// Fondo de página: membrete a página completa o blanco (drawBackground).
pub fn draw_background(surface: &mut Surface, letterhead: Option<&Image>) {
    match letterhead {
        Some(image) => {
            surface.push_transform(&Transform::from_translate(0.0, 0.0));
            surface.draw_image(
                image.clone(),
                Size::from_wh(PAGE_W as f32, PAGE_H as f32).unwrap(),
            );
            surface.pop();
        }
        None => fill_rect(
            surface,
            Frame {
                x: 0.0,
                y: 0.0,
                w: PAGE_W,
                h: PAGE_H,
            },
            WHITE,
        ),
    }
}

/// Título centrado con subrayado de acento (createA4ReportPage). Las páginas
/// de continuación lo dibujan en tamaño 14; la primera página delega en
/// `draw_summary`, que maqueta su propio título.
pub fn draw_title(
    surface: &mut Surface,
    title: &str,
    title_size: f64,
    underline_half_width: f64,
    fonts: &FontPair,
) {
    let bold = &fonts.bold;
    let title_width = bold.width(title, title_size);
    draw_text(
        surface,
        bold,
        title,
        title_size,
        (PAGE_W - title_width) / 2.0,
        TITLE_Y,
        palette(PaletteColor::Text),
    );
    stroke_line(
        surface,
        PAGE_W / 2.0 - underline_half_width,
        TITLE_Y - 9.0,
        PAGE_W / 2.0 + underline_half_width,
        TITLE_Y - 9.0,
        1.0,
        palette(PaletteColor::Accent),
    );
}

// ---------- tarjeta resumen (createA4ReportSummaryPage) ----------

/// Dibuja título + subtítulo + tarjeta resumen de 2 columnas. Devuelve la Y
/// inferior de la tarjeta (summaryBottomY en TS).
pub fn draw_summary(
    surface: &mut Surface,
    title: &str,
    subtitle: &str,
    items: &[SummaryItem],
    fonts: &FontPair,
) -> f64 {
    let title_size = 15.0;
    let title_line_height = title_size * 1.28;
    let subtitle_size = 8.5;
    let subtitle_line_height = subtitle_size * 1.38;

    let bold = &fonts.bold;
    let regular = &fonts.regular;

    let title_lines = wrap(title, bold, title_size, CONTENT_W);
    let mut cursor_y = TITLE_Y + 2.0;
    for line in &title_lines {
        let line_width = bold.width(line, title_size);
        draw_text(
            surface,
            bold,
            line,
            title_size,
            CONTENT_X + (CONTENT_W - line_width) / 2.0,
            cursor_y,
            palette(PaletteColor::Text),
        );
        cursor_y -= title_line_height;
    }

    let mut block_bottom_y = cursor_y;
    let mut sub_cursor = cursor_y - 14.0;
    for line in &wrap(subtitle, regular, subtitle_size, CONTENT_W) {
        let line_width = regular.width(line, subtitle_size);
        draw_text(
            surface,
            regular,
            line,
            subtitle_size,
            CONTENT_X + (CONTENT_W - line_width) / 2.0,
            sub_cursor,
            palette(PaletteColor::Muted),
        );
        sub_cursor -= subtitle_line_height;
    }
    if !subtitle.trim().is_empty() {
        block_bottom_y = sub_cursor;
    }

    let summary_top_y = block_bottom_y - 22.0;
    stroke_line(
        surface,
        PAGE_W / 2.0 - 39.0,
        summary_top_y + 8.0,
        PAGE_W / 2.0 + 39.0,
        summary_top_y + 8.0,
        1.2,
        palette(PaletteColor::Accent),
    );

    let column_count = 2usize;
    let row_count = items.len().div_ceil(column_count).max(1);
    let row_height = 38.0;
    let card_height = 18.0 + row_count as f64 * row_height;
    let card_y = summary_top_y - card_height;
    let card_padding_x = 16.0;
    let column_gap = 18.0;
    let column_width = (CONTENT_W - card_padding_x * 2.0 - column_gap * (column_count - 1) as f64)
        / column_count as f64;

    stroke_rect(
        surface,
        Frame {
            x: CONTENT_X,
            y: card_y,
            w: CONTENT_W,
            h: card_height,
        },
        WHITE,
        Some(PANEL_BORDER),
        1.0,
    );
    fill_rect(
        surface,
        Frame {
            x: CONTENT_X,
            y: summary_top_y - 3.0,
            w: CONTENT_W,
            h: 3.0,
        },
        palette(PaletteColor::Accent),
    );

    for row_index in 1..row_count {
        let y = summary_top_y - 10.0 - row_index as f64 * row_height;
        stroke_line(
            surface,
            CONTENT_X + card_padding_x,
            y,
            CONTENT_X + CONTENT_W - card_padding_x,
            y,
            0.5,
            BORDER,
        );
    }

    stroke_line(
        surface,
        CONTENT_X + card_padding_x + column_width + column_gap / 2.0,
        card_y + 10.0,
        CONTENT_X + card_padding_x + column_width + column_gap / 2.0,
        summary_top_y - 12.0,
        0.5,
        BORDER,
    );

    for (index, item) in items.iter().enumerate() {
        let column_index = index % column_count;
        let row_index = index / column_count;
        let x = CONTENT_X + card_padding_x + column_index as f64 * (column_width + column_gap);
        let top_y = summary_top_y - 18.0 - row_index as f64 * row_height;

        let label = fit(&item.label.to_uppercase(), regular, 6.8, column_width);
        draw_text(
            surface,
            regular,
            &label,
            6.8,
            x,
            top_y,
            palette(PaletteColor::Muted),
        );

        for (line_index, line) in limited_wrap(&item.value, bold, 9.2, column_width, 2)
            .iter()
            .enumerate()
        {
            draw_text(
                surface,
                bold,
                line,
                9.2,
                x,
                top_y - 12.0 - line_index as f64 * 10.5,
                palette(item.color),
            );
        }
    }

    card_y
}

// ---------- tabla (header, filas, paginación, estado vacío) ----------

/// Header de tabla con fondo, línea de acento, separadores y labels centrados.
/// Devuelve la Y inferior del header (donde empieza la primera fila).
pub fn draw_table_header(
    surface: &mut Surface,
    columns: &[Column],
    top_y: f64,
    fonts: &FontPair,
) -> f64 {
    let header_y = top_y - TABLE_HEADER_HEIGHT;

    stroke_rect(
        surface,
        Frame {
            x: CONTENT_X,
            y: header_y,
            w: CONTENT_W,
            h: TABLE_HEADER_HEIGHT,
        },
        HEADER_SURFACE,
        Some(BORDER),
        1.0,
    );
    stroke_line(
        surface,
        CONTENT_X,
        top_y,
        CONTENT_X + CONTENT_W,
        top_y,
        1.0,
        palette(PaletteColor::Accent),
    );
    draw_column_separators(surface, columns, header_y, TABLE_HEADER_HEIGHT);

    let bold = &fonts.bold;
    for column in columns {
        let text_width = bold.width(&column.label, TABLE_FONT_SIZE);
        draw_text(
            surface,
            bold,
            &column.label,
            TABLE_FONT_SIZE,
            column.x + column.width / 2.0 - text_width / 2.0,
            header_y + 8.0,
            palette(PaletteColor::Text),
        );
    }

    header_y
}

fn draw_column_separators(surface: &mut Surface, columns: &[Column], bottom_y: f64, height: f64) {
    for column in columns.iter().skip(1) {
        stroke_line(
            surface,
            column.x,
            bottom_y,
            column.x,
            bottom_y + height,
            0.6,
            BORDER,
        );
    }
}

/// Celda pre-wrappeada lista para dibujar.
pub struct PreparedCell {
    pub lines: Vec<String>,
    pub bold: bool,
    pub color: PaletteColor,
}

/// Altura de fila (measureA4ReportTableRowHeight): max(32, max_lines*11 + 12).
pub fn row_height(cells: &[PreparedCell]) -> f64 {
    let max_lines = cells
        .iter()
        .map(|c| c.lines.len().max(1))
        .max()
        .unwrap_or(1);
    (MIN_ROW_HEIGHT).max(max_lines as f64 * TABLE_LINE_HEIGHT + 12.0)
}

/// Dibuja una fila zebra con borde y separadores; devuelve la Y inferior.
pub fn draw_table_row(
    surface: &mut Surface,
    columns: &[Column],
    cells: &[PreparedCell],
    top_y: f64,
    row_index: usize,
    fonts: &FontPair,
) -> f64 {
    let height = row_height(cells);
    let row_y = top_y - height;
    let background = if row_index.is_multiple_of(2) {
        WHITE
    } else {
        SURFACE
    };

    stroke_rect(
        surface,
        Frame {
            x: CONTENT_X,
            y: row_y,
            w: CONTENT_W,
            h: height,
        },
        background,
        Some(BORDER),
        1.0,
    );
    draw_column_separators(surface, columns, row_y, height);

    for (cell, column) in cells.iter().zip(columns) {
        let font = fonts.pick(cell.bold);
        for (line_index, line) in cell.lines.iter().enumerate() {
            draw_text(
                surface,
                font,
                line,
                TABLE_FONT_SIZE,
                column.x + 8.0,
                top_y - 16.0 - line_index as f64 * TABLE_LINE_HEIGHT,
                palette(cell.color),
            );
        }
    }

    row_y
}

/// Estado vacío centrado (drawA4ReportEmptyState).
pub fn draw_empty_state(
    surface: &mut Surface,
    top_y: f64,
    title: &str,
    subtitle: &str,
    fonts: &FontPair,
) {
    let height = 62.0;
    let empty_y = top_y - height;

    stroke_rect(
        surface,
        Frame {
            x: CONTENT_X,
            y: empty_y,
            w: CONTENT_W,
            h: height,
        },
        WHITE,
        Some(BORDER),
        1.0,
    );

    let title_width = fonts.bold.width(title, 10.5);
    draw_text(
        surface,
        &fonts.bold,
        title,
        10.5,
        CONTENT_X + CONTENT_W / 2.0 - title_width / 2.0,
        empty_y + 35.0,
        palette(PaletteColor::Text),
    );
    let subtitle_width = fonts.regular.width(subtitle, 9.0);
    draw_text(
        surface,
        &fonts.regular,
        subtitle,
        9.0,
        CONTENT_X + CONTENT_W / 2.0 - subtitle_width / 2.0,
        empty_y + 20.0,
        palette(PaletteColor::Muted),
    );
}

pub fn page_settings() -> PageSettings {
    PageSettings::from_wh(PAGE_W as f32, PAGE_H as f32).expect("A4 es geometría válida")
}
