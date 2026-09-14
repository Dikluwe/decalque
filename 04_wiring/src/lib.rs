//! Composição do pipeline do Decalque (L4).
//!
//! Este módulo liga a tradução sintática de `03_infra` ao domínio puro de
//! `01_core`. Como arquivo de composição, não declara uma linhagem de prompt
//! própria (ADR 0003).

use decalque_core::entities::{resolve_page_geometry, PageGeometryDiagnostic};
use decalque_core::{
    build_font_model, diagnose_page, interpret_text, DocumentGeometry, FontModelDiagnostic,
    TextInterpretationInput, TextInterpreterDiagnostic,
};
use decalque_infra::PageSource;
use std::path::Path;

/// Resultado completo da materialização de uma página.
///
/// `DocumentGeometry` recebe apenas diagnósticos de página, conforme o seu
/// contrato. Os diagnósticos das etapas de geometria, fonte e interpretação
/// permanecem separados para uma futura política de apresentação do shell.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterializedPage {
    pub geometry: DocumentGeometry,
    pub page_geometry_diagnostics: Vec<PageGeometryDiagnostic>,
    pub font_diagnostics: Vec<FontModelDiagnostic>,
    pub text_diagnostics: Vec<TextInterpreterDiagnostic>,
}

/// Materializa a fonte de uma página no contrato consumido pelo comparador.
pub fn materialize_page(source: PageSource) -> MaterializedPage {
    let (page, page_geometry_diagnostics) = resolve_page_geometry(&source.box_model);

    let mut fonts = Vec::with_capacity(source.fonts.len());
    let mut font_diagnostics = Vec::new();
    for raw_font in &source.fonts {
        let (font, diagnostics) = build_font_model(raw_font);
        fonts.push(font);
        font_diagnostics.extend(diagnostics);
    }

    let page_diagnostics = diagnose_page(
        source.hints.has_text_show_operators,
        source.hints.has_do_operator,
        &source.xobjects,
    );
    let interpretation = interpret_text(&TextInterpretationInput {
        page,
        operations: source.operations,
        fonts,
        xobjects: source.xobjects,
    });

    MaterializedPage {
        geometry: DocumentGeometry {
            page,
            glyphs: interpretation.glyphs,
            diagnostics: page_diagnostics,
        },
        page_geometry_diagnostics,
        font_diagnostics,
        text_diagnostics: interpretation.diagnostics,
    }
}

/// Carrega e materializa uma página de PDF pelo adaptador configurado.
pub fn load_materialized_page(
    path: &Path,
    page_index: usize,
) -> Result<MaterializedPage, decalque_core::PdfError> {
    decalque_infra::load_page_source(path, page_index).map(materialize_page)
}

#[cfg(test)]
mod tests {
    use super::*;
    use decalque_core::entities::{PageBoxModel, Rect};
    use decalque_core::{
        ContentOperation, PageDiagnostic, RawFontData, RawFontEncoding, RawFontSubtype,
        XObjectInfo, XObjectSubtype,
    };
    use decalque_infra::PageSourceHints;

    fn box_model() -> PageBoxModel {
        PageBoxModel {
            media_box: Rect {
                x0: 0.0,
                y0: 0.0,
                x1: 200.0,
                y1: 300.0,
            },
            crop_box: None,
            rotate: None,
            user_unit: None,
        }
    }

    #[test]
    fn materializa_pagina_de_imagem_sem_inventar_glifos() {
        let source = PageSource {
            box_model: box_model(),
            operations: vec![ContentOperation::InvokeXObject {
                name: "Im1".to_string(),
            }],
            fonts: Vec::new(),
            xobjects: vec![XObjectInfo {
                name: "Im1".to_string(),
                subtype: XObjectSubtype::Image,
            }],
            hints: PageSourceHints {
                has_text_show_operators: false,
                has_do_operator: true,
            },
        };

        let result = materialize_page(source);
        assert!(result.geometry.glyphs.is_empty());
        assert_eq!(
            result.geometry.diagnostics,
            vec![PageDiagnostic::ImageOnlyPage]
        );
        assert!(result.page_geometry_diagnostics.is_empty());
        assert!(result.font_diagnostics.is_empty());
    }

    #[test]
    fn materializa_texto_e_preserva_diagnosticos_por_etapa() {
        let source = PageSource {
            box_model: box_model(),
            operations: vec![
                ContentOperation::BeginText,
                ContentOperation::SetFont {
                    name: "F1".to_string(),
                    size_pt: 10.0,
                },
                ContentOperation::SetTextMatrix {
                    m: [1.0, 0.0, 0.0, 1.0, 20.0, 30.0],
                },
                ContentOperation::ShowText { bytes: vec![65] },
                ContentOperation::EndText,
            ],
            fonts: vec![RawFontData {
                resource_name: "F1".to_string(),
                subtype: RawFontSubtype::Type1,
                base_font: None,
                encoding: RawFontEncoding::Absent,
                default_width: Some(500.0),
                widths: vec![(65, 500.0)],
                tounicode: None,
            }],
            xobjects: Vec::new(),
            hints: PageSourceHints {
                has_text_show_operators: true,
                has_do_operator: false,
            },
        };

        let result = materialize_page(source);
        assert_eq!(result.geometry.glyphs.len(), 1);
        assert!(result.geometry.diagnostics.is_empty());
        assert!(result
            .text_diagnostics
            .contains(&TextInterpreterDiagnostic::UnmappedGlyphs));
    }
}
