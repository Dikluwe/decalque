//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/_deprecated/page-geometry.md
//! @layer L1
//! @updated 2026-08-12
//!
//! Geometria da página: largura e altura em pontos PDF (1/72 polegada),
//! a unidade nativa do formato — sem conversão para outra unidade.
//!
//! ESTADO: implementa a spec arquivada em `_deprecated/`. A sucessora activa é
//! `00_nucleo/prompts/page-geometry-model.md` (`Rect`, `PageBoxModel`,
//! `PageRotation`, `resolve_page_geometry`, `display_size`, `PageGeometry` com
//! `rotation`/`user_unit`), ainda não implementada. A linhagem aponta para a
//! spec que este ficheiro de facto cumpre — ADR 0003, regra de código gerado
//! de spec arquivada.

/// Dimensões de uma página de PDF, em pontos (1/72 polegada).
///
/// L1: zero I/O — os valores chegam já extraídos (é `03_infra` quem lê a
/// `MediaBox`/equivalente do PDF real e constrói esta struct).
///
/// Desenhada para não impedir extensão a múltiplas páginas: um documento
/// multi-página será simplesmente uma colecção destas structs (a versão
/// inicial assume 1 página, caso de uso typst de página única).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageGeometry {
    /// Largura da página em pontos PDF.
    pub width: f64,
    /// Altura da página em pontos PDF.
    pub height: f64,
}

impl PageGeometry {
    /// Diferença de tamanho entre duas páginas.
    ///
    /// Convenção de sinal: devolve `(other.width - self.width, other.height - self.height)`,
    /// ou seja, **Δ = other − self**. Um Δ positivo significa que `other` é maior.
    ///
    /// Não julga se a diferença é "grande" ou "pequena" — essa decisão é de
    /// quem consome o valor, não desta struct.
    pub fn size_delta(&self, other: &PageGeometry) -> (f64, f64) {
        (other.width - self.width, other.height - self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_delta_de_paginas_iguais_e_zero() {
        let a = PageGeometry { width: 595.0, height: 842.0 };
        let b = PageGeometry { width: 595.0, height: 842.0 };
        assert_eq!(a.size_delta(&b), (0.0, 0.0));
    }

    #[test]
    fn size_delta_com_larguras_diferentes_tem_sinal_other_menos_self() {
        let a = PageGeometry { width: 595.0, height: 842.0 };
        let b = PageGeometry { width: 612.0, height: 842.0 };
        let (dw, dh) = a.size_delta(&b);
        assert_eq!(dw, 17.0); // other - self, positivo
        assert_eq!(dh, 0.0);

        // Sentido inverso: sinal inverte.
        let (dw_inv, _) = b.size_delta(&a);
        assert_eq!(dw_inv, -17.0);
    }
}
