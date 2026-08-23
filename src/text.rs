//! Medición de texto y wrap/fit — el único motor de wrap del binario
//! (unifica los tres duplicados de TS: kernel A4, carnet y ticket).

use std::path::Path;
use std::sync::Arc;

use crate::protocol::ManifestError;

pub struct MeasuredFont {
    data: &'static [u8],
    face: ttf_parser::Face<'static>,
}

impl MeasuredFont {
    fn load(path: &Path) -> Result<Self, String> {
        let data: Arc<[u8]> = std::fs::read(path)
            .map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?
            .into();
        // El proceso vive lo justo para un lote: las fuentes se quedan para siempre.
        let data: &'static [u8] = Box::leak(data.into());
        let face = ttf_parser::Face::parse(data, 0)
            .map_err(|e| format!("fuente inválida {}: {e}", path.display()))?;
        Ok(Self { data, face })
    }

    /// Ancho del texto en puntos al tamaño dado (mismas métricas que usa krilla
    /// al renderizar; sin kerning, como pdf-lib con fuentes estándar).
    pub fn width(&self, text: &str, size: f64) -> f64 {
        let upem = self.face.units_per_em().max(1) as f64;
        let units: f64 = text
            .chars()
            .filter_map(|ch| self.face.glyph_index(ch))
            .map(|gid| self.face.glyph_hor_advance(gid).unwrap_or(0) as f64)
            .sum();
        units / upem * size
    }

    pub fn krilla_font(&self) -> krilla::text::Font {
        krilla::text::Font::new(self.data.into(), 0)
            .expect("ttf-parser ya validó esta fuente; krilla debe aceptarla")
    }
}

pub struct FontPair {
    pub regular: MeasuredFont,
    pub bold: MeasuredFont,
}

impl FontPair {
    pub fn load(assets_dir: &Path) -> Result<Self, ManifestError> {
        let join = |name: &str| assets_dir.join("fonts").join(name);
        let regular = MeasuredFont::load(&join("LiberationMono-Regular.ttf"))
            .map_err(|e| ManifestError(format!("assets: {e}")))?;
        let bold = MeasuredFont::load(&join("LiberationMono-Bold.ttf"))
            .map_err(|e| ManifestError(format!("assets: {e}")))?;
        Ok(Self { regular, bold })
    }

    pub fn pick(&self, bold: bool) -> &MeasuredFont {
        if bold { &self.bold } else { &self.regular }
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

fn without_last_char(s: &str) -> &str {
    match s.char_indices().next_back() {
        Some((index, _)) => &s[..index],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> MeasuredFont {
        MeasuredFont::load(Path::new("assets/fonts/LiberationMono-Regular.ttf")).unwrap()
    }

    #[test]
    fn wrap_keeps_lines_within_width() {
        let font = font();
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
        let font = font();
        let fitted = fit("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", &font, 8.0, 40.0);
        assert!(fitted.ends_with("..."));
        assert!(font.width(&fitted, 8.0) <= 40.0);
    }

    #[test]
    fn wrap_collapses_whitespace() {
        let font = font();
        assert_eq!(wrap("hola   mundo", &font, 8.0, 500.0), vec!["hola mundo"]);
        assert!(wrap("   ", &font, 8.0, 500.0).is_empty());
    }
}
