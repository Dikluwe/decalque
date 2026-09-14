//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/document-geometry.md
//! @layer L1
//! @updated 2026-09-14

use super::{GlyphInstance, PageDiagnostic, PageGeometry};

/// Geometria de uma página, contrato único de entrada do motor.
///
/// Um documento com várias páginas é representado pelo chamador como
/// `Vec<DocumentGeometry>`. Os glifos permanecem na ordem de emissão do
/// content stream; a ordenação de leitura pertence ao motor. `diagnostics`
/// contém somente diagnósticos de página, não os do intérprete textual.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentGeometry {
    pub page: PageGeometry,
    pub glyphs: Vec<GlyphInstance>,
    pub diagnostics: Vec<PageDiagnostic>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{PageRotation, TextMappingStatus};

    fn page() -> PageGeometry {
        PageGeometry {
            width: 300.0,
            height: 400.0,
            origin: (0.0, 0.0),
            rotation: PageRotation::Deg0,
            user_unit: 1.0,
        }
    }

    fn glyph(code: u32, x: f64) -> GlyphInstance {
        GlyphInstance {
            glyph_code: code,
            codepoints: None,
            position: (x, 10.0),
            advance: 5.0,
            font_ref: "F0".to_string(),
            font_size_pt: 10.0,
            mapping_status: TextMappingStatus::Unmapped,
            render_mode: 0,
        }
    }

    #[test]
    fn empty_page_is_valid_and_can_carry_page_diagnostic() {
        let document = DocumentGeometry {
            page: page(),
            glyphs: Vec::new(),
            diagnostics: vec![PageDiagnostic::ImageOnlyPage],
        };
        assert!(document.glyphs.is_empty());
        assert_eq!(document.diagnostics, vec![PageDiagnostic::ImageOnlyPage]);
    }

    #[test]
    fn glyphs_preserve_content_stream_emission_order() {
        let document = DocumentGeometry {
            page: page(),
            glyphs: vec![glyph(2, 200.0), glyph(1, 100.0)],
            diagnostics: Vec::new(),
        };
        assert_eq!(
            document
                .glyphs
                .iter()
                .map(|glyph| glyph.glyph_code)
                .collect::<Vec<_>>(),
            vec![2, 1]
        );
    }
}
