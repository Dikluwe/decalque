//! Modelo de conteúdo de PDF (L1): parser de `ToUnicode`/CMap e modelo de
//! fonte. Zero I/O — recebe bytes e dados brutos já extraídos por `03_infra`.
//!
//! Sem cabeçalho de linhagem: nenhum prompt gera este ficheiro — é agregação
//! de módulos (ADR 0003, excepção para ficheiros sem prompt de origem).
//!
//! Falta ainda `text_interpreter.rs`
//! (`00_nucleo/prompts/content-stream-text-model.md`), que depende destes dois.

pub mod cmap;
pub mod font_model;

pub use cmap::{parse_tounicode_cmap, CmapDiagnostic, CmapEntry, CmapMapping, CmapParseResult};
pub use font_model::{
    build_font_model, FontModel, FontModelDiagnostic, FontWidths, GlyphCodeDecoder, RawFontData,
    RawFontEncoding, RawFontSubtype,
};
