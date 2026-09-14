//! Entidades do domínio (L1): geometria de página, resolução de medição e
//! instâncias de glifo.

pub mod glyph_instance;
pub mod measurement_resolution;
pub mod page_geometry;
pub mod pdf_error;

pub use glyph_instance::{DocumentGeometry, GlyphInstance};
pub use measurement_resolution::MeasurementResolution;
pub use page_geometry::{
    display_size, resolve_page_geometry, PageBoxModel, PageGeometry, PageGeometryDiagnostic,
    PageRotation, Rect,
};
pub use pdf_error::PdfError;
