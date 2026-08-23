//! Medición, wrap/fit y dibujado de texto — el ÚNICO motor de texto del
//! binario (unifica los tres duplicados de TS: kernel A4, carnet y ticket).
//!
//! Los runs de glifos se posicionan con ttf-parser: las mismas métricas con
//! las que se mide el wrap son las que se dibujan (modelo pdf-lib), sin
//! traer el shapeador completo (rustybuzz).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use krilla::geom::Point;
use krilla::paint::Fill;
use krilla::surface::Surface;
use krilla::text::KrillaGlyph;

use crate::protocol::ManifestError;

pub struct MeasuredFont {
    data: &'static [u8],
    face: ttf_parser::Face<'static>,
}

impl MeasuredFont {
    fn load(path: &Path) -> Result<Self, String> {
        let data: std::sync::Arc<[u8]> = std::fs::read(path)
            .map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?
            .into();
        // El proceso vive lo justo para un lote: las fuentes se quedan para siempre.
        let data: &'static [u8] = Box::leak(data.into());
        let face = ttf_parser::Face::parse(data, 0)
            .map_err(|e| format!("fuente inválida {}: {e}", path.display()))?;
        Ok(Self { data, face })
    }

    /// Ancho del texto en puntos al tamaño dado, sin kerning (igual que pdf-lib
    /// con fuentes estándar; coherente con `glyph_run` que dibuja sin kerning).
    pub fn width(&self, text: &str, size: f64) -> f64 {
        let upem = self.face.units_per_em().max(1) as f64;
        let units: f64 = text
            .chars()
            .map(|ch| {
                self.face
                    .glyph_index(ch)
                    .and_then(|gid| self.face.glyph_hor_advance(gid))
                    .unwrap_or(0) as f64
            })
            .sum();
        units / upem * size
    }

    /// Run de glifos posicionados 1:1 con los chars del texto. El glifo que
    /// falte en la fuente cae a .notdef (visible) en vez de romper el lote.
    fn glyph_run(&self, text: &str) -> Vec<KrillaGlyph> {
        let upem = self.face.units_per_em().max(1) as f32;
        text.char_indices()
            .map(|(byte_index, ch)| {
                let gid = self.face.glyph_index(ch).unwrap_or(ttf_parser::GlyphId(0));
                let advance = gid
                    .0
                    .checked_add(0)
                    .and_then(|_| self.face.glyph_hor_advance(gid))
                    .unwrap_or(0) as f32
                    / upem;
                KrillaGlyph {
                    glyph_id: krilla_gid(gid),
                    text_range: byte_index..byte_index + ch.len_utf8(),
                    x_advance: advance,
                    x_offset: 0.0,
                    y_offset: 0.0,
                    y_advance: 0.0,
                    location: None,
                }
            })
            .collect()
    }

    fn krilla_font(&self) -> krilla::text::Font {
        krilla::text::Font::new(self.data.into(), 0)
            .expect("ttf-parser ya validó esta fuente; krilla debe aceptarla")
    }

    /// Dibuja una línea con baseline en (x, y_krilla) y el fill activo.
    pub fn draw_line(&self, surface: &mut Surface, text: &str, size: f64, x: f64, y_krilla: f64) {
        let glyphs = self.glyph_run(text);
        surface.draw_glyphs(
            Point::from_xy(x as f32, y_krilla as f32),
            &glyphs,
            self.krilla_font(),
            text,
            size as f32,
            false,
        );
    }
}

fn krilla_gid(gid: ttf_parser::GlyphId) -> krilla::text::GlyphId {
    krilla::text::GlyphId::new(u32::from(gid.0))
}

pub struct FontPair {
    pub regular: MeasuredFont,
    pub bold: MeasuredFont,
}

impl FontPair {
    fn load(dir: &Path, family: &str) -> Result<Self, String> {
        let regular = MeasuredFont::load(&dir.join(format!("{family}-Regular.ttf")))?;
        let bold = MeasuredFont::load(&dir.join(format!("{family}-Bold.ttf")))?;
        Ok(Self { regular, bold })
    }

    pub fn pick(&self, bold: bool) -> &MeasuredFont {
        if bold { &self.bold } else { &self.regular }
    }
}

/// Las dos familias del contrato: Mono (ticket térmico) y Sans (A4/carnet,
/// métricamente compatible con Helvetica/Arial).
pub struct Fonts {
    pub mono: FontPair,
    pub sans: FontPair,
}

impl Fonts {
    pub fn load(assets_dir: &Path) -> Result<Self, ManifestError> {
        let dir = assets_dir.join("fonts");
        let mono = FontPair::load(&dir, "LiberationMono")
            .map_err(|e| ManifestError(format!("assets: {e}")))?;
        let sans = FontPair::load(&dir, "LiberationSans")
            .map_err(|e| ManifestError(format!("assets: {e}")))?;
        Ok(Self { mono, sans })
    }
}

/// Imágenes del directorio de assets, decodificadas una vez por lote.
pub struct ImageCache {
    dir: PathBuf,
    cache: HashMap<String, Option<krilla::image::Image>>,
}

impl ImageCache {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            cache: HashMap::new(),
        }
    }

    /// None si el archivo no existe o no es un PNG válido (el caller decide si
    /// es fatal: el membrete es opcional, un template de carnet quizá no).
    pub fn get(&mut self, file: &str) -> Option<krilla::image::Image> {
        self.cache
            .entry(file.to_owned())
            .or_insert_with(|| {
                let data = std::fs::read(self.dir.join(file)).ok()?;
                krilla::image::Image::from_png(data.into(), true).ok()
            })
            .clone()
    }
}

/// Puerto directo de `fitText`/`fitPdfText` de TS: trunca con ellipsis al ancho máximo.
pub fn fit(text: &str, font: &MeasuredFont, size: f64, max_width: f64) -> String {
    if font.width(text, size) <= max_width {
        return text.to_owned();
    }
    const SUFFIX: &str = "...";
    let suffix_width = font.width(SUFFIX, size);
    if suffix_width >= max_width {
        return String::new();
    }
    let mut fitted = text;
    while !fitted.is_empty() && font.width(&format!("{fitted}{SUFFIX}"), size) > max_width {
        fitted = without_last_char(fitted);
    }
    if fitted.is_empty() {
        SUFFIX.to_owned()
    } else {
        format!("{fitted}{SUFFIX}")
    }
}

/// Puerto directo de `wrapText`/`wrapPdfText` de TS: greedy por palabras.
pub fn wrap(text: &str, font: &MeasuredFont, size: f64, max_width: f64) -> Vec<String> {
    let normalized: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut line = String::new();
    for word in normalized.split(' ') {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if font.width(&candidate, size) <= max_width {
            line = candidate;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        if font.width(word, size) <= max_width {
            line = word.to_owned();
        } else {
            lines.push(fit(word, font, size, max_width));
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Puerto de `getLimitedWrappedLines`: wrap acotado a `max_lines`, la última
/// línea conservada se trunca con "..." (sufijo vacío, igual que el TS).
pub fn limited_wrap(
    text: &str,
    font: &MeasuredFont,
    size: f64,
    max_width: f64,
    max_lines: usize,
) -> Vec<String> {
    let lines = wrap(text, font, size, max_width);
    if lines.len() <= max_lines {
        return lines;
    }
    let mut kept: Vec<String> = lines.iter().take(max_lines - 1).cloned().collect();
    let remainder = format!("{}...", lines[max_lines - 1]);
    kept.push(fit(&remainder, font, size, max_width));
    kept
}

/// Fill sólido RGB — el único estilo de texto que usa el contrato v1.
pub fn solid_fill(rgb: (f32, f32, f32)) -> Fill {
    let [r, g, b] = [rgb.0, rgb.1, rgb.2].map(|c| (c * 255.0).round().clamp(0.0, 255.0) as u8);
    Fill {
        paint: krilla::color::rgb::Color::new(r, g, b).into(),
        ..Default::default()
    }
}

fn without_last_char(s: &str) -> &str {
    match s.char_indices().next_back() {
        Some((index, _)) => &s[..index],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mono() -> MeasuredFont {
        MeasuredFont::load(Path::new("assets/fonts/LiberationMono-Regular.ttf")).unwrap()
    }

    #[test]
    fn wrap_keeps_lines_within_width() {
        let font = mono();
        let text = "I.E. Coedula Nacional Independencia de Próceres del Perú";
        let lines = wrap(text, &font, 9.5, 198.77);
        assert!(lines.len() > 1);
        for line in &lines {
            assert!(
                font.width(line, 9.5) <= 198.77,
                "línea demasiado ancha: {line}"
            );
        }
        assert_eq!(lines.join(" "), text);
    }

    #[test]
    fn fit_truncates_long_word() {
        let font = mono();
        let fitted = fit("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", &font, 8.0, 40.0);
        assert!(fitted.ends_with("..."));
        assert!(font.width(&fitted, 8.0) <= 40.0);
    }

    #[test]
    fn wrap_collapses_whitespace() {
        let font = mono();
        assert_eq!(wrap("hola   mundo", &font, 8.0, 500.0), vec!["hola mundo"]);
        assert!(wrap("   ", &font, 8.0, 500.0).is_empty());
    }

    #[test]
    fn limited_wrap_caps_line_count() {
        let font = mono();
        let text = "palabra larga que seguramente ocupa varias líneas al envolver";
        let lines = limited_wrap(text, &font, 8.0, 60.0, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with("..."));
    }
}
