//! @prompt 00_nucleo/prompts/entities/cartesian-origin.md
//!
//! Representa a origem e a direcção do eixo y do sistema de coordenadas
//! nativo de um PDF (já detectadas por `03_infra`), e converte pontos desse
//! sistema para o sistema comum de comparação.

use super::page_geometry::PageGeometry;

/// Direcção em que o eixo y cresce no sistema nativo do PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisDirection {
    /// y cresce para cima (convenção clássica do content stream PDF).
    YUp,
    /// y cresce para baixo.
    YDown,
}

/// Origem do sistema de coordenadas nativo do PDF, mais a direcção do eixo y.
///
/// L1: zero I/O — detectar a orientação real (inspeccionar a `MediaBox` e a
/// matriz de transformação inicial do content stream) é trabalho de `03_infra`;
/// esta struct só representa o resultado já decidido.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartesianOrigin {
    /// Coordenada x da origem nativa, em pontos.
    pub x: f64,
    /// Coordenada y da origem nativa, em pontos.
    pub y: f64,
    /// Direcção do eixo y nativo.
    pub y_axis: AxisDirection,
}

impl CartesianOrigin {
    /// Converte um ponto do sistema nativo do PDF para o sistema comum de
    /// comparação: **y-para-baixo, origem no canto superior esquerdo**.
    ///
    /// Escolha explícita do sistema alvo: y-para-baixo é mais intuitivo para
    /// quem lê o relatório depois — "y maior = mais para baixo na página
    /// impressa". Por isso fica registado aqui, não implícito.
    ///
    /// Fórmula (com translação da origem):
    /// - `x' = point.0 - self.x`
    /// - `YDown`: `y' = point.1 - self.y` (já está no sistema alvo, só translada)
    /// - `YUp`:   `y' = page.height - (point.1 - self.y)` (espelha em torno da altura)
    pub fn normalize(&self, point: (f64, f64), page: &PageGeometry) -> (f64, f64) {
        let x = point.0 - self.x;
        let y_local = point.1 - self.y;
        let y = match self.y_axis {
            AxisDirection::YDown => y_local,
            AxisDirection::YUp => page.height - y_local,
        };
        (x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> PageGeometry {
        PageGeometry { width: 595.0, height: 842.0 }
    }

    #[test]
    fn y_down_sem_translacao_devolve_ponto_identico() {
        let origem = CartesianOrigin { x: 0.0, y: 0.0, y_axis: AxisDirection::YDown };
        assert_eq!(origem.normalize((123.0, 456.0), &page()), (123.0, 456.0));
    }

    #[test]
    fn y_up_sem_translacao_espelha_em_torno_da_altura() {
        let origem = CartesianOrigin { x: 0.0, y: 0.0, y_axis: AxisDirection::YUp };
        // y' = page.height - y
        assert_eq!(origem.normalize((100.0, 800.0), &page()), (100.0, 42.0));
    }

    #[test]
    fn y_up_com_translacao_aplica_formula_completa() {
        // y' = page.height - (y - self.y)
        let origem = CartesianOrigin { x: 10.0, y: 20.0, y_axis: AxisDirection::YUp };
        let (x, y) = origem.normalize((110.0, 820.0), &page());
        assert_eq!(x, 100.0); // 110 - 10
        assert_eq!(y, 42.0); // 842 - (820 - 20)
    }

    #[test]
    fn y_down_com_translacao_apenas_translada() {
        let origem = CartesianOrigin { x: 10.0, y: 20.0, y_axis: AxisDirection::YDown };
        assert_eq!(origem.normalize((110.0, 220.0), &page()), (100.0, 200.0));
    }
}
