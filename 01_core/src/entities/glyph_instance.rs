//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/content-stream-text-model.md
//! @layer L1
//! @updated 2026-09-14
//!
//! Unidade atómica de comparação: um glifo desenhado numa posição.

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
    /// Código bruto preservado mesmo quando não existe mapeamento Unicode.
    pub glyph_code: u32,
    /// Sequência de codepoints via `ToUnicode`; `None` se não mapeado.
    ///
    /// É sequência e não `char` único (ADR 0001) porque uma ligadura é um
    /// glifo que mapeia para vários codepoints — o glifo "fi" mapeia
    /// tipicamente para `['f','i']`. A sequência é a âncora que permite a
    /// normalização de ligaduras no motor de comparação.
    pub codepoints: Option<Vec<char>>,
    /// Posição já normalizada (`normalize_to_top_left` aplicado):
    /// y-para-baixo, origem no canto superior esquerdo.
    pub position: (f64, f64),
    /// Avanço horizontal efetivo em pontos, após espaçamentos e escala.
    pub advance: f64,
    /// Tamanho de fonte do glifo, em pontos.
    pub font_size_pt: f64,
    /// Identificador da fonte no documento (nome do recurso no PDF, ex. "F1").
    pub font_ref: String,
    pub mapping_status: TextMappingStatus,
    /// Modo de renderização textual em vigor (`Tr`), entre 0 e 7.
    pub render_mode: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextMappingStatus {
    Mapped,
    Unmapped,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glifo_suporta_ligadura_como_sequencia_de_codepoints() {
        let g = GlyphInstance {
            glyph_code: 1,
            position: (100.0, 200.0),
            codepoints: Some(vec!['f', 'i']),
            advance: 6.0,
            font_size_pt: 12.0,
            font_ref: "F1".to_string(),
            mapping_status: TextMappingStatus::Mapped,
            render_mode: 0,
        };
        assert_eq!(g.codepoints.as_deref(), Some(&['f', 'i'][..]));
    }

    #[test]
    fn glifo_sem_codepoints_mapeados_e_valido() {
        let g = GlyphInstance {
            glyph_code: 42,
            position: (50.0, 60.0),
            codepoints: None,
            advance: 5.0,
            font_size_pt: 10.0,
            font_ref: "F2".to_string(),
            mapping_status: TextMappingStatus::Unmapped,
            render_mode: 3,
        };
        assert!(g.codepoints.is_none());
        assert_eq!(g.glyph_code, 42);
        assert_eq!(g.render_mode, 3);
    }
}
