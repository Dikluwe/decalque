//! Núcleo de domínio do Decalque (L1): geometria de páginas e glifos,
//! normalização de coordenadas, resolução de medição e motor de comparação.
//! Zero I/O, zero dependências externas.

pub mod engine;
pub mod entities;

pub use engine::{compare, ComparisonReport, GlyphPair};
pub use entities::{
    AxisDirection, CartesianOrigin, DocumentGeometry, GlyphInstance, MeasurementResolution,
    PageGeometry,
};
