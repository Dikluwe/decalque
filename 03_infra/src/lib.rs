//! Leitura e parsing estrutural de PDF (L3).
//!
//! Sem cabeçalho de linhagem: este ficheiro apenas agrega módulos.

pub mod lopdf_adapter;

pub use lopdf_adapter::{load_page_source, PageSource, PageSourceHints};
