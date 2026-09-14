//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/entities/measurement-resolution.md
//! @layer L2
//! @updated 2026-09-14
//!
//! Políticas do shell já decididas pelas especificações do núcleo.

use decalque_core::{ComparisonReport, Coverage, MeasurementResolution};

/// Perfil padrão do Caso 1 (digital↔digital).
///
/// O valor de 0.5pt preserva o comportamento validado no protótipo P948. O
/// Caso 2 permanece fora deste módulo enquanto a sua especificação estiver em
/// estado de planeamento.
pub fn digital_to_digital_resolution() -> MeasurementResolution {
    MeasurementResolution::Absolute { tolerance_pt: 0.5 }
}

/// Métricas que o shell pode apresentar sem separar a mediana da cobertura.
///
/// Agrupar esses campos impede que uma mediana excelente sobre poucos glifos
/// seja tratada como paridade do documento. `None` continua significando que
/// nenhuma medição aconteceu; o shell não o converte em zero.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportMetrics {
    pub median_abs_dx: Option<f64>,
    pub median_abs_dy: Option<f64>,
    pub max_abs_dx: Option<f64>,
    pub max_abs_dy: Option<f64>,
    pub coverage: Coverage,
}

impl ReportMetrics {
    pub fn from_report(report: &ComparisonReport<'_>) -> Self {
        Self {
            median_abs_dx: report.median_abs_dx,
            median_abs_dy: report.median_abs_dy,
            max_abs_dx: report.max_abs_dx,
            max_abs_dy: report.max_abs_dy,
            coverage: report.coverage.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use decalque_core::entities::PageRotation;
    use decalque_core::{compare, DocumentGeometry, PageGeometry};

    fn empty_document() -> DocumentGeometry {
        DocumentGeometry {
            page: PageGeometry {
                width: 595.0,
                height: 842.0,
                origin: (0.0, 0.0),
                rotation: PageRotation::Deg0,
                user_unit: 1.0,
            },
            glyphs: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn caso_digital_usa_meio_ponto_absoluto() {
        let resolution = digital_to_digital_resolution();
        assert_eq!(resolution.tolerance_for(6.0), 0.5);
        assert_eq!(resolution.tolerance_for(40.0), 0.5);
    }

    #[test]
    fn metricas_sem_pares_preservam_none_e_cobertura() {
        let document = empty_document();
        let report = compare(&document, &document, &digital_to_digital_resolution());
        let metrics = ReportMetrics::from_report(&report);
        assert_eq!(metrics.median_abs_dx, None);
        assert_eq!(metrics.max_abs_dy, None);
        assert_eq!(metrics.coverage.total_a, 0);
        assert_eq!(metrics.coverage.total_b, 0);
    }
}
