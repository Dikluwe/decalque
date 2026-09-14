//! Políticas de apresentação e configuração do Decalque (L2).
//!
//! O parsing da CLI ainda aguarda especificação própria. Este crate materializa
//! somente decisões já fechadas pelos prompts do núcleo.

mod policy;

pub use policy::{digital_to_digital_resolution, ReportMetrics};
