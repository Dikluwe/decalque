//! Leitura e parsing estrutural de PDF (L3).
//!
//! Sem cabeçalho de linhagem: este ficheiro apenas agrega módulos.

pub use decalque_core::PdfError;

pub mod lopdf_adapter;
pub mod raster_identity;
pub mod scan_observation_json;
mod typst_candidate;
mod typst_reconstruction;

pub use lopdf_adapter::{
    load_page_source, load_single_page_source_from_pdf_bytes, PageSource, PageSourceHints,
};
pub use raster_identity::{
    bind_scan_observation_raster, probe_raster_bytes, BoundRasterIdentity, RasterIdentityError,
    RasterMetadata,
};
pub use scan_observation_json::{
    load_scan_observation_json, parse_scan_observation_json, ScanObservationJsonError,
    ScanObservationLimits,
};
pub use typst_candidate::{
    compile_typst_candidate, identify_typst_compiler, publish_new_file, IdentifiedTypstCompiler,
    PublishNewFileError, TypstCompilation, TypstExecutionError, TypstExecutionLimits,
};
pub use typst_reconstruction::{render_strict_typst_source, render_typst_source};
