//! Núcleo de domínio do Decalque (L1): geometria de páginas e glifos,
//! normalização de coordenadas, resolução de medição e motor de comparação.
//! Zero I/O, zero dependências externas.

pub mod content;
pub mod engine;
pub mod entities;
pub mod geometry;

pub use content::{
    build_font_model, interpret_text, parse_tounicode_cmap, CmapDiagnostic, CmapEntry, CmapMapping,
    CmapParseResult, ContentOperation, FontModel, FontModelDiagnostic, FontWidths,
    GlyphCodeDecoder, RawFontData, RawFontEncoding, RawFontSubtype, TextInterpretationInput,
    TextInterpretationOutput, TextInterpreterDiagnostic, TjItem, XObjectInfo, XObjectSubtype,
};
pub use engine::{compare, ComparisonReport, GlyphPair};
pub use entities::{
    DocumentGeometry, GlyphInstance, MeasurementResolution, PageGeometry, PdfError,
    TextMappingStatus,
};
pub use geometry::normalize_to_top_left;
