//! Modelo de conteúdo de PDF (L1): parser de `ToUnicode`/CMap, fontes e
//! interpretação semântica de operações. Zero I/O.
//!
//! Sem cabeçalho de linhagem: nenhum prompt gera este ficheiro — é agregação
//! de módulos (ADR 0003, excepção para ficheiros sem prompt de origem).
//!
pub mod cmap;
pub mod font_model;
pub mod text_interpreter;

pub use cmap::{parse_tounicode_cmap, CmapDiagnostic, CmapEntry, CmapMapping, CmapParseResult};
pub use font_model::{
    build_font_model, FontModel, FontModelDiagnostic, FontWidths, GlyphCodeDecoder, RawFontData,
    RawFontEncoding, RawFontSubtype,
};
pub use text_interpreter::{
    interpret_text, ContentOperation, TextInterpretationInput, TextInterpretationOutput,
    TextInterpreterDiagnostic, TjItem, XObjectInfo, XObjectSubtype,
};
