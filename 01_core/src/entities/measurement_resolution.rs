//! @prompt 00_nucleo/prompts/entities/measurement-resolution.md
//!
//! Define quando uma diferença de posição conta como divergência a reportar,
//! versus ruído de precisão numérica aceitável.

/// Resolução de medição: o limiar a partir do qual um delta de posição é
/// considerado divergência.
///
/// Decisão de desenho (ADR 0001, 2026-08-11): **ambas as formas são suportadas
/// desde a versão inicial**, e a tolerância **não é uma constante do domínio —
/// é um parâmetro do caso de uso**, escolhido pelo chamador (`02_shell`/CLI):
///
/// - Caso 1 (digital↔digital, paridade de compiladores): tolerância apertada —
///   divergência é regressão. Default: `Absolute` com 0.5pt (paridade com o
///   comportamento validado no protótipo P948).
/// - Caso 2 (scan→digital): tolerância mais larga — fontes substituídas e
///   reflow são diferenças legitimamente esperadas.
///
/// `Absolute` é simples mas cegamente insensível à escala (0.5pt é muito para
/// um índice de 6pt, pouco para um título de 40pt — achado real do P948);
/// `RelativeToEm` escala automaticamente com o tamanho de fonte do glifo medido.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeasurementResolution {
    /// Tolerância fixa em pontos, igual para o documento inteiro.
    Absolute { tolerance_pt: f64 },
    /// Tolerância expressa como fracção do em (tamanho de fonte) do glifo medido.
    RelativeToEm { fraction: f64 },
}

impl MeasurementResolution {
    /// Tolerância efectiva, em pontos, para um glifo com o tamanho de fonte dado.
    ///
    /// `Absolute` ignora o tamanho de fonte; `RelativeToEm` devolve
    /// `fraction * font_size_pt`.
    pub fn tolerance_for(&self, font_size_pt: f64) -> f64 {
        match self {
            MeasurementResolution::Absolute { tolerance_pt } => *tolerance_pt,
            MeasurementResolution::RelativeToEm { fraction } => fraction * font_size_pt,
        }
    }

    /// `true` se `|delta|` está dentro da tolerância para o tamanho de fonte dado
    /// (fronteira inclusive: `|delta| <= tolerância`).
    pub fn is_within(&self, delta: f64, font_size_pt: f64) -> bool {
        delta.abs() <= self.tolerance_for(font_size_pt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absoluta_e_independente_do_tamanho_de_fonte() {
        let r = MeasurementResolution::Absolute { tolerance_pt: 0.5 };
        assert!(r.is_within(0.3, 6.0));
        assert!(r.is_within(0.3, 40.0));
        assert!(!r.is_within(0.6, 6.0));
        assert!(!r.is_within(0.6, 40.0));
        assert_eq!(r.tolerance_for(10.0), 0.5);
    }

    #[test]
    fn relativa_ao_em_escala_com_o_tamanho_de_fonte() {
        let r = MeasurementResolution::RelativeToEm { fraction: 0.02 };
        assert_eq!(r.tolerance_for(10.0), 0.2); // 2% de 10pt
        assert_eq!(r.tolerance_for(40.0), 0.8);
        assert!(r.is_within(0.15, 10.0));
        assert!(!r.is_within(0.25, 10.0));
    }

    #[test]
    fn is_within_usa_valor_absoluto_do_delta() {
        let r = MeasurementResolution::Absolute { tolerance_pt: 0.5 };
        assert!(r.is_within(-0.3, 10.0));
        assert!(!r.is_within(-0.6, 10.0));
    }
}
