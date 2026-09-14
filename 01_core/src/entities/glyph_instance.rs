//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/_deprecated/glyph-instance.md
//! @layer L1
//! @updated 2026-08-12
//!
//! Unidade atómica de comparação: um glifo desenhado numa posição, e a
//! colecção de todos os glifos de um documento já em coordenadas normalizadas.
//!
//! ESTADO: implementa a spec arquivada em `_deprecated/`. Sucessoras activas,
//! ainda não implementadas:
//! - `GlyphInstance` → `00_nucleo/prompts/content-stream-text-model.md`
//!   (acrescenta `glyph_code`, `advance`, `mapping_status`);
//! - `DocumentGeometry` → `00_nucleo/prompts/document-geometry.md` (ficheiro
//!   próprio `entities/document_geometry.rs`, acrescenta `diagnostics`).
//!
//! A linhagem aponta para a spec arquivada porque é a que este ficheiro
//! cumpre — ADR 0003, regra de código gerado de spec arquivada.

use super::page_geometry::PageGeometry;

/// Um glifo desenhado numa posição da página.
///
/// L1: zero I/O — construído por `03_infra` a partir da leitura real do
/// content stream (`Tj`/`TJ`/`cm`/`Tf`, extracção de `ToUnicode`); esta struct
/// só representa o resultado. É também o contrato de saída da extracção
/// externa do Caso 2 (scan→digital): o lado do scan entra no pipeline como um
/// `DocumentGeometry` produzido fora do núcleo, na mesma forma (ADR 0001).
///
/// Nota de proveniência (lição do P948): glifos sem codepoints mapeados
/// (`codepoints: None`) **não são descartados** — ainda têm posição e
/// participam na comparação, só não têm âncora textual para o emparelhamento
/// por conteúdo (ver `engine/compare`).
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphInstance {
    /// Posição já normalizada (`normalize_to_top_left` aplicado):
    /// y-para-baixo, origem no canto superior esquerdo.
    pub position: (f64, f64),
    /// Sequência de codepoints via `ToUnicode`; `None` se não mapeado.
    ///
    /// É sequência e não `char` único (ADR 0001) porque uma ligadura é um
    /// glifo que mapeia para vários codepoints — o glifo "fi" mapeia
    /// tipicamente para `['f','i']`. A sequência é a âncora que permite a
    /// normalização de ligaduras no motor de comparação.
    pub codepoints: Option<Vec<char>>,
    /// Tamanho de fonte do glifo, em pontos.
    pub font_size_pt: f64,
    /// Identificador da fonte no documento (nome do recurso no PDF, ex. "F1").
    pub font_ref: String,
}

/// Geometria completa de um documento: a página e todos os seus glifos, já
/// em coordenadas normalizadas.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentGeometry {
    /// Dimensões da página que produziu estes glifos.
    pub page: PageGeometry,
    /// Glifos do documento. Vazio é um caso válido (documento sem conteúdo),
    /// não excepcional.
    pub glyphs: Vec<GlyphInstance>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::PageRotation;

    #[test]
    fn documento_sem_glifos_e_valido() {
        let doc = DocumentGeometry {
            page: PageGeometry {
                width: 595.0,
                height: 842.0,
                origin: (0.0, 0.0),
                rotation: PageRotation::Deg0,
                user_unit: 1.0,
            },
            glyphs: Vec::new(),
        };
        assert!(doc.glyphs.is_empty());
    }

    #[test]
    fn glifo_suporta_ligadura_como_sequencia_de_codepoints() {
        let g = GlyphInstance {
            position: (100.0, 200.0),
            codepoints: Some(vec!['f', 'i']),
            font_size_pt: 12.0,
            font_ref: "F1".to_string(),
        };
        assert_eq!(g.codepoints.as_deref(), Some(&['f', 'i'][..]));
    }

    #[test]
    fn glifo_sem_codepoints_mapeados_e_valido() {
        let g = GlyphInstance {
            position: (50.0, 60.0),
            codepoints: None,
            font_size_pt: 10.0,
            font_ref: "F2".to_string(),
        };
        assert!(g.codepoints.is_none());
    }
}
