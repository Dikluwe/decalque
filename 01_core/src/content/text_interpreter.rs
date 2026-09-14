//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/content-stream-text-model.md
//! @layer L1
//! @updated 2026-09-14

use super::font_model::{FontModel, GlyphCodeDecoder};
use crate::entities::{GlyphInstance, PageGeometry, TextMappingStatus};
use crate::geometry::normalize_to_top_left;

#[derive(Debug, Clone, PartialEq)]
pub enum ContentOperation {
    BeginText,
    EndText,
    SetFont { name: String, size_pt: f64 },
    SetTextMatrix { m: [f64; 6] },
    MoveText { tx: f64, ty: f64 },
    MoveTextSetLeading { tx: f64, ty: f64 },
    NextLine,
    SetLeading { leading: f64 },
    SetCharSpacing { tc: f64 },
    SetWordSpacing { tw: f64 },
    SetHorizontalScaling { tz_percent: f64 },
    SetTextRise { ts: f64 },
    SetTextRenderMode { mode: i32 },
    ShowText { bytes: Vec<u8> },
    ShowTextAdjusted { items: Vec<TjItem> },
    SaveState,
    RestoreState,
    ConcatMatrix { m: [f64; 6] },
    InvokeXObject { name: String },
    UnsupportedTextOp { operator: String },
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TjItem {
    Text { bytes: Vec<u8> },
    Adjustment { thousandths: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct XObjectInfo {
    pub name: String,
    pub subtype: XObjectSubtype,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XObjectSubtype {
    Form,
    Image,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextInterpretationInput {
    pub page: PageGeometry,
    pub operations: Vec<ContentOperation>,
    pub fonts: Vec<FontModel>,
    pub xobjects: Vec<XObjectInfo>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextInterpretationOutput {
    pub glyphs: Vec<GlyphInstance>,
    pub diagnostics: Vec<TextInterpreterDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInterpreterDiagnostic {
    UnmappedGlyphs,
    TextStateNotFullyApplied,
    FormXObjectNotTraversed,
    MissingFont,
    UnsupportedTextOperator,
    TextOutsideTextObject,
    TrailingGlyphCodeBytes,
    InvisibleTextPresent,
}

type Matrix = [f64; 6];
const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

struct State {
    ctm: Matrix,
    text_matrix: Matrix,
    line_matrix: Matrix,
    in_text: bool,
    font_name: Option<String>,
    font_size: f64,
    char_spacing: f64,
    word_spacing: f64,
    horizontal_scaling: f64,
    leading: f64,
    rise: f64,
    render_mode: u8,
}

impl Default for State {
    fn default() -> Self {
        Self {
            ctm: IDENTITY,
            text_matrix: IDENTITY,
            line_matrix: IDENTITY,
            in_text: false,
            font_name: None,
            font_size: 0.0,
            char_spacing: 0.0,
            word_spacing: 0.0,
            horizontal_scaling: 100.0,
            leading: 0.0,
            rise: 0.0,
            render_mode: 0,
        }
    }
}

/// Interpreta semântica textual usando vetores coluna e concatenação à
/// esquerda (`nova = operando × corrente`). A posição é
/// `CTM × Tm × (0, Ts)`, normalizada somente depois para o canto superior
/// esquerdo. O avanço é
/// `(width/1000 × font_size + Tc + Tw_espaço) × Tz/100`; ajustes `TJ`
/// deslocam por `-(valor/1000) × font_size × Tz/100`.
pub fn interpret_text(input: &TextInterpretationInput) -> TextInterpretationOutput {
    let mut output = TextInterpretationOutput {
        glyphs: Vec::new(),
        diagnostics: Vec::new(),
    };
    let mut state = State::default();
    let mut ctm_stack = Vec::new();

    for operation in &input.operations {
        match operation {
            ContentOperation::SaveState => ctm_stack.push(state.ctm),
            ContentOperation::RestoreState => {
                if let Some(ctm) = ctm_stack.pop() {
                    state.ctm = ctm;
                }
            }
            ContentOperation::ConcatMatrix { m } => state.ctm = multiply(*m, state.ctm),
            ContentOperation::BeginText => {
                state.in_text = true;
                state.text_matrix = IDENTITY;
                state.line_matrix = IDENTITY;
            }
            ContentOperation::EndText => state.in_text = false,
            ContentOperation::InvokeXObject { name } => {
                if input
                    .xobjects
                    .iter()
                    .any(|x| x.name == *name && x.subtype == XObjectSubtype::Form)
                {
                    diagnostic_once(
                        &mut output.diagnostics,
                        TextInterpreterDiagnostic::FormXObjectNotTraversed,
                    );
                }
            }
            ContentOperation::Other => {}
            ContentOperation::UnsupportedTextOp { .. } => {
                diagnostic_once(
                    &mut output.diagnostics,
                    TextInterpreterDiagnostic::UnsupportedTextOperator,
                );
                if !state.in_text {
                    diagnostic_once(
                        &mut output.diagnostics,
                        TextInterpreterDiagnostic::TextOutsideTextObject,
                    );
                }
            }
            _ if !state.in_text => diagnostic_once(
                &mut output.diagnostics,
                TextInterpreterDiagnostic::TextOutsideTextObject,
            ),
            ContentOperation::SetFont { name, size_pt } => {
                state.font_name = Some(name.clone());
                state.font_size = *size_pt;
                if !input.fonts.iter().any(|font| font.resource_name == *name) {
                    diagnostic_once(
                        &mut output.diagnostics,
                        TextInterpreterDiagnostic::MissingFont,
                    );
                }
            }
            ContentOperation::SetTextMatrix { m } => {
                state.text_matrix = *m;
                state.line_matrix = *m;
            }
            ContentOperation::MoveText { tx, ty } => move_text(&mut state, *tx, *ty),
            ContentOperation::MoveTextSetLeading { tx, ty } => {
                state.leading = -*ty;
                move_text(&mut state, *tx, *ty);
            }
            ContentOperation::NextLine => {
                let leading = state.leading;
                move_text(&mut state, 0.0, -leading);
            }
            ContentOperation::SetLeading { leading } => state.leading = *leading,
            ContentOperation::SetCharSpacing { tc } => state.char_spacing = *tc,
            ContentOperation::SetWordSpacing { tw } => state.word_spacing = *tw,
            ContentOperation::SetHorizontalScaling { tz_percent } => {
                state.horizontal_scaling = *tz_percent
            }
            ContentOperation::SetTextRise { ts } => state.rise = *ts,
            ContentOperation::SetTextRenderMode { mode } => {
                if (0..=7).contains(mode) {
                    state.render_mode = *mode as u8;
                } else {
                    state.render_mode = 0;
                    diagnostic_once(
                        &mut output.diagnostics,
                        TextInterpreterDiagnostic::UnsupportedTextOperator,
                    );
                }
            }
            ContentOperation::ShowText { bytes } => {
                emit_bytes(input, &mut state, bytes, &mut output)
            }
            ContentOperation::ShowTextAdjusted { items } => {
                for item in items {
                    match item {
                        TjItem::Text { bytes } => emit_bytes(input, &mut state, bytes, &mut output),
                        TjItem::Adjustment { thousandths } => {
                            let displacement = -(*thousandths / 1000.0)
                                * state.font_size
                                * (state.horizontal_scaling / 100.0);
                            state.text_matrix =
                                multiply(translation(displacement, 0.0), state.text_matrix);
                        }
                    }
                }
            }
        }
    }
    output
}

fn emit_bytes(
    input: &TextInterpretationInput,
    state: &mut State,
    bytes: &[u8],
    output: &mut TextInterpretationOutput,
) {
    let Some(font_name) = state.font_name.as_deref() else {
        diagnostic_once(
            &mut output.diagnostics,
            TextInterpreterDiagnostic::MissingFont,
        );
        return;
    };
    let Some(font) = input
        .fonts
        .iter()
        .find(|font| font.resource_name == font_name)
    else {
        diagnostic_once(
            &mut output.diagnostics,
            TextInterpreterDiagnostic::MissingFont,
        );
        return;
    };

    let (codes, trailing) = font.decoder.decode(bytes);
    if trailing > 0 {
        diagnostic_once(
            &mut output.diagnostics,
            TextInterpreterDiagnostic::TrailingGlyphCodeBytes,
        );
    }
    for glyph_code in codes {
        let codepoints = font
            .unicode_map
            .as_ref()
            .and_then(|mapping| mapping.lookup(glyph_code));
        let mapping_status = if codepoints.is_some() {
            TextMappingStatus::Mapped
        } else {
            diagnostic_once(
                &mut output.diagnostics,
                TextInterpreterDiagnostic::UnmappedGlyphs,
            );
            TextMappingStatus::Unmapped
        };
        if matches!(state.render_mode, 3 | 7) {
            diagnostic_once(
                &mut output.diagnostics,
                TextInterpreterDiagnostic::InvisibleTextPresent,
            );
        }

        let text_point = apply(state.text_matrix, (0.0, state.rise));
        let user_point = apply(state.ctm, text_point);
        let word_spacing = if glyph_code == 0x20 && font.decoder == GlyphCodeDecoder::SingleByte {
            state.word_spacing
        } else {
            0.0
        };
        let advance = (font.widths.width_of(glyph_code) / 1000.0 * state.font_size
            + state.char_spacing
            + word_spacing)
            * (state.horizontal_scaling / 100.0);
        output.glyphs.push(GlyphInstance {
            glyph_code,
            codepoints,
            position: normalize_to_top_left(user_point, &input.page),
            advance,
            font_ref: font_name.to_string(),
            font_size_pt: state.font_size,
            mapping_status,
            render_mode: state.render_mode,
        });
        state.text_matrix = multiply(translation(advance, 0.0), state.text_matrix);
    }
}

fn move_text(state: &mut State, tx: f64, ty: f64) {
    state.line_matrix = multiply(translation(tx, ty), state.line_matrix);
    state.text_matrix = state.line_matrix;
}

fn translation(tx: f64, ty: f64) -> Matrix {
    [1.0, 0.0, 0.0, 1.0, tx, ty]
}

fn multiply(left: Matrix, right: Matrix) -> Matrix {
    [
        left[0] * right[0] + left[2] * right[1],
        left[1] * right[0] + left[3] * right[1],
        left[0] * right[2] + left[2] * right[3],
        left[1] * right[2] + left[3] * right[3],
        left[0] * right[4] + left[2] * right[5] + left[4],
        left[1] * right[4] + left[3] * right[5] + left[5],
    ]
}

fn apply(matrix: Matrix, point: (f64, f64)) -> (f64, f64) {
    (
        matrix[0] * point.0 + matrix[2] * point.1 + matrix[4],
        matrix[1] * point.0 + matrix[3] * point.1 + matrix[5],
    )
}

fn diagnostic_once(
    diagnostics: &mut Vec<TextInterpreterDiagnostic>,
    diagnostic: TextInterpreterDiagnostic,
) {
    if !diagnostics.contains(&diagnostic) {
        diagnostics.push(diagnostic);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{CmapEntry, CmapMapping, FontWidths};
    use crate::entities::PageRotation;

    fn page() -> PageGeometry {
        PageGeometry {
            width: 612.0,
            height: 792.0,
            origin: (0.0, 0.0),
            rotation: PageRotation::Deg0,
            user_unit: 1.0,
        }
    }

    fn font() -> FontModel {
        FontModel {
            resource_name: "f0".to_string(),
            base_font: None,
            decoder: GlyphCodeDecoder::SingleByte,
            widths: FontWidths {
                default_width: 500.0,
                widths: Vec::new(),
            },
            unicode_map: Some(CmapMapping {
                entries: vec![
                    CmapEntry::Char {
                        src: b'A' as u32,
                        dst: vec!['A'],
                    },
                    CmapEntry::Char {
                        src: b'B' as u32,
                        dst: vec!['B'],
                    },
                    CmapEntry::Char {
                        src: b' ' as u32,
                        dst: vec![' '],
                    },
                ],
            }),
        }
    }

    fn interpret(operations: Vec<ContentOperation>) -> TextInterpretationOutput {
        interpret_text(&TextInterpretationInput {
            page: page(),
            operations,
            fonts: vec![font()],
            xobjects: Vec::new(),
        })
    }

    fn text_prefix() -> Vec<ContentOperation> {
        vec![
            ContentOperation::BeginText,
            ContentOperation::SetFont {
                name: "f0".to_string(),
                size_pt: 12.0,
            },
            ContentOperation::SetTextMatrix {
                m: [1.0, 0.0, 0.0, 1.0, 100.0, 700.0],
            },
        ]
    }

    fn approx(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
    }

    #[test]
    fn emits_mapped_glyph_with_normalized_position_and_advance() {
        let mut operations = text_prefix();
        operations.push(ContentOperation::ShowText { bytes: vec![b'A'] });
        operations.push(ContentOperation::EndText);
        let output = interpret(operations);
        assert!(output.diagnostics.is_empty());
        assert_eq!(output.glyphs.len(), 1);
        let glyph = &output.glyphs[0];
        assert_eq!(glyph.position, (100.0, 92.0));
        assert_eq!(glyph.mapping_status, TextMappingStatus::Mapped);
        assert_eq!(glyph.codepoints, Some(vec!['A']));
        approx(glyph.advance, 6.0);
    }

    #[test]
    fn tj_adjustment_moves_the_following_glyph_with_pdf_sign() {
        let mut operations = text_prefix();
        operations.push(ContentOperation::ShowTextAdjusted {
            items: vec![
                TjItem::Text { bytes: vec![b'A'] },
                TjItem::Adjustment {
                    thousandths: -120.0,
                },
                TjItem::Text { bytes: vec![b'B'] },
            ],
        });
        let output = interpret(operations);
        approx(
            output.glyphs[1].position.0 - output.glyphs[0].position.0,
            7.44,
        );
    }

    #[test]
    fn word_spacing_only_changes_single_byte_space() {
        let mut operations = text_prefix();
        operations.push(ContentOperation::SetWordSpacing { tw: 2.0 });
        operations.push(ContentOperation::ShowText {
            bytes: vec![b'A', b' ', b'B'],
        });
        let output = interpret(operations);
        approx(output.glyphs[0].advance, 6.0);
        approx(output.glyphs[1].advance, 8.0);
        approx(output.glyphs[2].advance, 6.0);
    }

    #[test]
    fn char_spacing_and_horizontal_scaling_are_applied() {
        let mut operations = text_prefix();
        operations.push(ContentOperation::SetCharSpacing { tc: 2.0 });
        operations.push(ContentOperation::SetHorizontalScaling { tz_percent: 50.0 });
        operations.push(ContentOperation::ShowText {
            bytes: vec![b'A', b'B'],
        });
        let output = interpret(operations);
        approx(output.glyphs[0].advance, 4.0);
        approx(
            output.glyphs[1].position.0 - output.glyphs[0].position.0,
            4.0,
        );
        assert!(!output
            .diagnostics
            .contains(&TextInterpreterDiagnostic::TextStateNotFullyApplied));
    }

    #[test]
    fn rise_and_ctm_are_applied_before_page_normalization() {
        let operations = vec![
            ContentOperation::ConcatMatrix {
                m: [1.0, 0.0, 0.0, 1.0, 10.0, 20.0],
            },
            ContentOperation::BeginText,
            ContentOperation::SetFont {
                name: "f0".to_string(),
                size_pt: 12.0,
            },
            ContentOperation::SetTextMatrix {
                m: [1.0, 0.0, 0.0, 1.0, 100.0, 700.0],
            },
            ContentOperation::SetTextRise { ts: 3.0 },
            ContentOperation::ShowText { bytes: vec![b'A'] },
        ];
        assert_eq!(interpret(operations).glyphs[0].position, (110.0, 69.0));
    }

    #[test]
    fn missing_font_suppresses_glyphs_and_diagnoses_once() {
        let output = interpret(vec![
            ContentOperation::BeginText,
            ContentOperation::SetFont {
                name: "missing".to_string(),
                size_pt: 12.0,
            },
            ContentOperation::ShowText { bytes: vec![b'A'] },
        ]);
        assert!(output.glyphs.is_empty());
        assert_eq!(
            output.diagnostics,
            vec![TextInterpreterDiagnostic::MissingFont]
        );
    }

    #[test]
    fn unmapped_code_is_preserved_without_inventing_unicode() {
        let mut operations = text_prefix();
        operations.push(ContentOperation::ShowText { bytes: vec![0xFF] });
        let output = interpret(operations);
        assert_eq!(output.glyphs[0].glyph_code, 0xFF);
        assert_eq!(output.glyphs[0].mapping_status, TextMappingStatus::Unmapped);
        assert_eq!(output.glyphs[0].codepoints, None);
        assert_eq!(
            output.diagnostics,
            vec![TextInterpreterDiagnostic::UnmappedGlyphs]
        );
    }

    #[test]
    fn render_mode_is_recorded_and_invisible_text_is_not_discarded() {
        let mut operations = text_prefix();
        operations.extend([
            ContentOperation::SetTextRenderMode { mode: 3 },
            ContentOperation::ShowText { bytes: vec![b'A'] },
            ContentOperation::SetTextRenderMode { mode: 0 },
            ContentOperation::ShowText { bytes: vec![b'B'] },
        ]);
        let output = interpret(operations);
        assert_eq!(output.glyphs.len(), 2);
        assert_eq!(output.glyphs[0].render_mode, 3);
        assert_eq!(output.glyphs[1].render_mode, 0);
        assert_eq!(
            output.diagnostics,
            vec![TextInterpreterDiagnostic::InvisibleTextPresent]
        );
    }

    #[test]
    fn invalid_render_mode_falls_back_to_zero() {
        let mut operations = text_prefix();
        operations.push(ContentOperation::SetTextRenderMode { mode: 9 });
        operations.push(ContentOperation::ShowText { bytes: vec![b'A'] });
        let output = interpret(operations);
        assert_eq!(output.glyphs[0].render_mode, 0);
        assert_eq!(
            output.diagnostics,
            vec![TextInterpreterDiagnostic::UnsupportedTextOperator]
        );
    }

    #[test]
    fn form_xobject_is_diagnosed_but_image_is_not() {
        let output = interpret_text(&TextInterpretationInput {
            page: page(),
            operations: vec![
                ContentOperation::InvokeXObject {
                    name: "form".to_string(),
                },
                ContentOperation::InvokeXObject {
                    name: "image".to_string(),
                },
            ],
            fonts: vec![],
            xobjects: vec![
                XObjectInfo {
                    name: "form".to_string(),
                    subtype: XObjectSubtype::Form,
                },
                XObjectInfo {
                    name: "image".to_string(),
                    subtype: XObjectSubtype::Image,
                },
            ],
        });
        assert_eq!(
            output.diagnostics,
            vec![TextInterpreterDiagnostic::FormXObjectNotTraversed]
        );
    }

    #[test]
    fn text_outside_object_and_unsupported_operator_are_diagnosed() {
        let mut operations = vec![ContentOperation::ShowText { bytes: vec![b'A'] }];
        operations.extend(text_prefix());
        operations.extend([
            ContentOperation::UnsupportedTextOp {
                operator: "'".to_string(),
            },
            ContentOperation::Other,
            ContentOperation::ShowText { bytes: vec![b'A'] },
        ]);
        let output = interpret(operations);
        assert_eq!(output.glyphs.len(), 1);
        assert_eq!(
            output.diagnostics,
            vec![
                TextInterpreterDiagnostic::TextOutsideTextObject,
                TextInterpreterDiagnostic::UnsupportedTextOperator,
            ]
        );
    }

    #[test]
    fn identity_h_trailing_byte_is_diagnosed() {
        let mut identity_font = font();
        identity_font.decoder = GlyphCodeDecoder::IdentityH;
        identity_font.unicode_map = Some(CmapMapping {
            entries: vec![CmapEntry::Char {
                src: 1,
                dst: vec!['A'],
            }],
        });
        let output = interpret_text(&TextInterpretationInput {
            page: page(),
            operations: vec![
                ContentOperation::BeginText,
                ContentOperation::SetFont {
                    name: "f0".to_string(),
                    size_pt: 12.0,
                },
                ContentOperation::ShowText {
                    bytes: vec![0, 1, 0xFF],
                },
            ],
            fonts: vec![identity_font],
            xobjects: vec![],
        });
        assert_eq!(output.glyphs.len(), 1);
        assert_eq!(
            output.diagnostics,
            vec![TextInterpreterDiagnostic::TrailingGlyphCodeBytes]
        );
    }

    #[test]
    fn q_and_q_restore_ctm() {
        let operations = vec![
            ContentOperation::SaveState,
            ContentOperation::ConcatMatrix {
                m: [1.0, 0.0, 0.0, 1.0, 50.0, 0.0],
            },
            ContentOperation::BeginText,
            ContentOperation::SetFont {
                name: "f0".to_string(),
                size_pt: 12.0,
            },
            ContentOperation::ShowText { bytes: vec![b'A'] },
            ContentOperation::EndText,
            ContentOperation::RestoreState,
            ContentOperation::BeginText,
            ContentOperation::ShowText { bytes: vec![b'B'] },
        ];
        let output = interpret(operations);
        assert_eq!(output.glyphs[0].position.0, 50.0);
        assert_eq!(output.glyphs[1].position.0, 0.0);
    }

    #[test]
    fn concat_matrix_is_applied_on_the_left() {
        let operations = vec![
            ContentOperation::ConcatMatrix {
                m: [1.0, 0.0, 0.0, 1.0, 10.0, 0.0],
            },
            ContentOperation::ConcatMatrix {
                m: [2.0, 0.0, 0.0, 2.0, 0.0, 0.0],
            },
            ContentOperation::BeginText,
            ContentOperation::SetFont {
                name: "f0".to_string(),
                size_pt: 12.0,
            },
            ContentOperation::ShowText { bytes: vec![b'A'] },
        ];
        assert_eq!(interpret(operations).glyphs[0].position.0, 20.0);
    }

    #[test]
    fn td_sets_leading_and_next_line_reuses_it() {
        let mut operations = text_prefix();
        operations.extend([
            ContentOperation::MoveTextSetLeading {
                tx: 10.0,
                ty: -20.0,
            },
            ContentOperation::NextLine,
            ContentOperation::ShowText { bytes: vec![b'A'] },
        ]);
        assert_eq!(interpret(operations).glyphs[0].position, (110.0, 132.0));
    }
}
