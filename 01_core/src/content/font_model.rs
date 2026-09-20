//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/pdf-font-model.md
//! @layer L1
//! @updated 2026-08-13
//!
//! Modelo de fonte puro. O `ToUnicode` resolve apenas `código → Unicode`; para
//! posicionar glifos o intérprete precisa de mais duas coisas por fonte: a
//! **largura de cada glifo** e a **decodificação da string em códigos**.
//!
//! `03_infra` extrai os dados brutos do dicionário `/Font` (via lopdf) e
//! `01_core` constrói o `FontModel` a partir deles.

use super::cmap::{parse_tounicode_cmap, CmapMapping};

/// Como os bytes de uma string mostrada se dividem em códigos de glifo.
#[derive(Debug, Clone, PartialEq)]
pub enum GlyphCodeDecoder {
    /// Fontes simples: cada byte é um código.
    SingleByte,
    /// Type0/CID com `Identity-H`: códigos de 2 bytes big-endian.
    IdentityH,
}

impl GlyphCodeDecoder {
    /// Decodifica os bytes crus de uma string mostrada (`Tj`/`TJ`) em códigos
    /// de glifo.
    ///
    /// Devolve `(códigos, bytes descartados)`. Bytes que não formam código
    /// completo — o byte ímpar final em `IdentityH` — são descartados e
    /// contados, para que o intérprete possa emitir `TrailingGlyphCodeBytes`
    /// em vez de os silenciar.
    pub fn decode(&self, bytes: &[u8]) -> (Vec<u32>, usize) {
        match self {
            GlyphCodeDecoder::SingleByte => (bytes.iter().map(|&b| b as u32).collect(), 0),
            GlyphCodeDecoder::IdentityH => {
                let codigos = bytes
                    .chunks_exact(2)
                    .map(|par| u16::from_be_bytes([par[0], par[1]]) as u32)
                    .collect();
                (codigos, bytes.len() % 2)
            }
        }
    }
}

/// Larguras de glifo de uma fonte, em unidades de glyph space (1000 = 1 em).
#[derive(Debug, Clone, PartialEq)]
pub struct FontWidths {
    /// `/DW` (ou `/MissingWidth`), ou o fallback documentado de `1000.0`.
    pub default_width: f64,
    /// Pares `(código, largura)` já expandidos por `03_infra` a partir de
    /// `/Widths` + `/FirstChar`/`/LastChar` (fontes simples) ou `/W` (CID).
    pub widths: Vec<(u32, f64)>,
}

impl FontWidths {
    /// Largura do glifo em unidades de glyph space.
    ///
    /// Códigos sem entrada própria caem em `default_width` **silenciosamente**
    /// — é o que a especificação PDF manda, não uma omissão.
    ///
    /// Limitação da v1: busca linear. As tabelas observadas são pequenas;
    /// optimizar só perante dados reais de desempenho.
    pub fn width_of(&self, glyph_code: u32) -> f64 {
        self.widths
            .iter()
            .find(|(codigo, _)| *codigo == glyph_code)
            .map(|(_, largura)| *largura)
            .unwrap_or(self.default_width)
    }
}

/// Uma fonte, na forma que o intérprete de texto consome.
#[derive(Debug, Clone, PartialEq)]
pub struct FontModel {
    /// Nome no dicionário `/Font` da página (ex.: `"f0"`).
    pub resource_name: String,
    pub base_font: Option<String>,
    pub decoder: GlyphCodeDecoder,
    pub widths: FontWidths,
    /// `ToUnicode` parseado; `None` se ausente ou ilegível.
    pub unicode_map: Option<CmapMapping>,
}

/// Subtipo declarado da fonte no PDF.
#[derive(Debug, Clone, PartialEq)]
pub enum RawFontSubtype {
    Type0,
    Type1,
    TrueType,
    CIDFontType0,
    CIDFontType2,
    Other,
}

/// `/Encoding` tal como aparece no dicionário da fonte.
#[derive(Debug, Clone, PartialEq)]
pub enum RawFontEncoding {
    /// Ex.: `"Identity-H"`, `"WinAnsiEncoding"`.
    Name(String),
    /// `/Encoding` com `/Differences` — nomes de glifo brutos.
    Differences(Vec<String>),
    /// CMap de codificação como stream.
    Stream(Vec<u8>),
    Absent,
}

/// Dados brutos que `03_infra` entrega por fonte.
///
/// Divisão de responsabilidade (documentada aqui de propósito): a expansão de
/// `/Widths` + `/FirstChar`/`/LastChar` e dos arrays `/W` para pares
/// `(código, largura)` é de **`03_infra`**; `01_core` recebe os pares prontos.
/// Em fontes Type0, `/DW` e `/W` vivem no dicionário **descendente**
/// (`/DescendantFonts[0]`) — extraí-los de lá também é de `03_infra`.
#[derive(Debug, Clone, PartialEq)]
pub struct RawFontData {
    pub resource_name: String,
    pub base_font: Option<String>,
    pub subtype: RawFontSubtype,
    pub encoding: RawFontEncoding,
    /// `/DW` (do descendente CIDFont, em Type0) ou `/MissingWidth`.
    pub default_width: Option<f64>,
    pub widths: Vec<(u32, f64)>,
    /// Bytes do stream `ToUnicode`; `None` se ausente **ou ilegível** —
    /// `03_infra` trata stream ilegível como ausente (não fatal: `01_core`
    /// produzirá glifos `Unmapped`).
    pub tounicode: Option<Vec<u8>>,
}

/// O que ficou por resolver ao construir o modelo. Nenhum destes é fatal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontModelDiagnostic {
    /// Combinação de subtipo/codificação que não é `SingleByte` nem
    /// `IdentityH` — o decoder cai em `SingleByte` e regista.
    UnsupportedEncoding,
    /// Sem `/Widths` e sem `/DW`: `default_width` cai no fallback de `1000.0`.
    NoWidths,
    /// `ToUnicode` parseado com diagnósticos, propagados do parser CMap.
    PartialTounicode,
}

/// Fallback de largura quando a fonte não declara tabela nenhuma.
const LARGURA_PADRAO_FALLBACK: f64 = 1000.0;

/// Constrói o modelo de fonte a partir dos dados brutos de `03_infra`.
pub fn build_font_model(raw: &RawFontData) -> (FontModel, Vec<FontModelDiagnostic>) {
    let mut diagnostics = Vec::new();

    let decoder = match (&raw.subtype, &raw.encoding) {
        (RawFontSubtype::Type0, RawFontEncoding::Name(nome)) if nome == "Identity-H" => {
            GlyphCodeDecoder::IdentityH
        }
        (RawFontSubtype::Type1 | RawFontSubtype::TrueType, _) => GlyphCodeDecoder::SingleByte,
        _ => {
            // Fallback documentado: não falha, regista.
            diagnostics.push(FontModelDiagnostic::UnsupportedEncoding);
            GlyphCodeDecoder::SingleByte
        }
    };

    let default_width = match raw.default_width {
        Some(largura) => largura,
        None => {
            if raw.widths.is_empty() {
                diagnostics.push(FontModelDiagnostic::NoWidths);
            }
            LARGURA_PADRAO_FALLBACK
        }
    };

    let unicode_map = raw.tounicode.as_ref().map(|bytes| {
        let resultado = parse_tounicode_cmap(bytes);
        if !resultado.diagnostics.is_empty() {
            diagnostics.push(FontModelDiagnostic::PartialTounicode);
        }
        resultado.mapping
    });

    let modelo = FontModel {
        resource_name: raw.resource_name.clone(),
        base_font: raw.base_font.clone(),
        decoder,
        widths: FontWidths {
            default_width,
            widths: raw.widths.clone(),
        },
        unicode_map,
    };
    (modelo, diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(subtype: RawFontSubtype, encoding: RawFontEncoding) -> RawFontData {
        RawFontData {
            resource_name: "f0".to_string(),
            base_font: Some("ABCDEF+NimbusRoman".to_string()),
            subtype,
            encoding,
            default_width: None,
            widths: Vec::new(),
            tounicode: None,
        }
    }

    #[test]
    fn single_byte_decodifica_cada_byte_como_codigo() {
        let (codigos, descartados) = GlyphCodeDecoder::SingleByte.decode(&[0x41, 0x42]);
        assert_eq!(codigos, vec![0x41, 0x42]);
        assert_eq!(descartados, 0);
    }

    #[test]
    fn identity_h_decodifica_pares_big_endian() {
        let (codigos, descartados) = GlyphCodeDecoder::IdentityH.decode(&[0x00, 0x01, 0x00, 0x02]);
        assert_eq!(codigos, vec![1, 2]);
        assert_eq!(descartados, 0);
    }

    #[test]
    fn identity_h_com_comprimento_impar_descarta_e_conta_o_byte_orfao() {
        let (codigos, descartados) = GlyphCodeDecoder::IdentityH.decode(&[0x00, 0x01, 0xFF]);
        assert_eq!(codigos, vec![1]);
        assert_eq!(descartados, 1);
    }

    #[test]
    fn decode_de_bytes_vazios_nao_produz_codigos() {
        assert_eq!(GlyphCodeDecoder::SingleByte.decode(&[]), (vec![], 0));
        assert_eq!(GlyphCodeDecoder::IdentityH.decode(&[]), (vec![], 0));
    }

    #[test]
    fn width_of_usa_a_tabela_e_cai_no_default() {
        let w = FontWidths {
            default_width: 500.0,
            widths: vec![(1, 600.0)],
        };
        assert_eq!(w.width_of(1), 600.0);
        assert_eq!(w.width_of(99), 500.0);
    }

    #[test]
    fn fonte_simples_sem_widths_e_sem_dw_cai_no_fallback_com_diagnostico() {
        let (modelo, diags) = build_font_model(&raw(
            RawFontSubtype::Type1,
            RawFontEncoding::Name("WinAnsiEncoding".to_string()),
        ));
        assert_eq!(modelo.decoder, GlyphCodeDecoder::SingleByte);
        assert_eq!(modelo.widths.default_width, 1000.0);
        assert!(diags.contains(&FontModelDiagnostic::NoWidths));
    }

    #[test]
    fn type0_identity_h_usa_decoder_de_dois_bytes_sem_diagnostico_de_codificacao() {
        let (modelo, diags) = build_font_model(&raw(
            RawFontSubtype::Type0,
            RawFontEncoding::Name("Identity-H".to_string()),
        ));
        assert_eq!(modelo.decoder, GlyphCodeDecoder::IdentityH);
        assert!(!diags.contains(&FontModelDiagnostic::UnsupportedEncoding));
    }

    #[test]
    fn type0_com_codificacao_desconhecida_cai_em_single_byte_e_regista() {
        let (modelo, diags) = build_font_model(&raw(
            RawFontSubtype::Type0,
            RawFontEncoding::Name("Identity-V".to_string()),
        ));
        assert_eq!(modelo.decoder, GlyphCodeDecoder::SingleByte);
        assert!(diags.contains(&FontModelDiagnostic::UnsupportedEncoding));
    }

    #[test]
    fn subtipo_fora_do_suportado_cai_em_single_byte_e_regista() {
        for subtipo in [
            RawFontSubtype::CIDFontType0,
            RawFontSubtype::CIDFontType2,
            RawFontSubtype::Other,
        ] {
            let (modelo, diags) = build_font_model(&raw(subtipo, RawFontEncoding::Absent));
            assert_eq!(modelo.decoder, GlyphCodeDecoder::SingleByte);
            assert!(diags.contains(&FontModelDiagnostic::UnsupportedEncoding));
        }
    }

    #[test]
    fn dw_presente_e_usado_sem_diagnostico() {
        let mut r = raw(RawFontSubtype::Type1, RawFontEncoding::Absent);
        r.default_width = Some(250.0);
        let (modelo, diags) = build_font_model(&r);
        assert_eq!(modelo.widths.default_width, 250.0);
        assert!(!diags.contains(&FontModelDiagnostic::NoWidths));
    }

    #[test]
    fn sem_dw_mas_com_tabela_de_larguras_nao_emite_no_widths() {
        // `NoWidths` é para "não há tabela nenhuma", não para "falta o /DW".
        let mut r = raw(RawFontSubtype::Type1, RawFontEncoding::Absent);
        r.widths = vec![(1, 600.0)];
        let (modelo, diags) = build_font_model(&r);
        assert_eq!(modelo.widths.default_width, 1000.0);
        assert_eq!(modelo.widths.width_of(1), 600.0);
        assert!(!diags.contains(&FontModelDiagnostic::NoWidths));
    }

    #[test]
    fn tounicode_valido_e_parseado_sem_diagnostico() {
        let mut r = raw(
            RawFontSubtype::Type0,
            RawFontEncoding::Name("Identity-H".to_string()),
        );
        r.tounicode = Some(b"beginbfchar <0001> <0041> endbfchar".to_vec());
        let (modelo, diags) = build_font_model(&r);
        let mapa = modelo.unicode_map.expect("mapa presente");
        assert_eq!(mapa.lookup(1), Some(vec!['A']));
        assert!(!diags.contains(&FontModelDiagnostic::PartialTounicode));
    }

    #[test]
    fn tounicode_com_defeito_propaga_partial_tounicode_preservando_o_que_e_valido() {
        let mut r = raw(RawFontSubtype::Type1, RawFontEncoding::Absent);
        // Segunda entrada com destino inválido: a primeira sobrevive.
        r.tounicode = Some(b"beginbfchar <0001> <0041> <0002> <004G> endbfchar".to_vec());
        let (modelo, diags) = build_font_model(&r);
        let mapa = modelo.unicode_map.expect("mapa presente");
        assert_eq!(mapa.lookup(1), Some(vec!['A']));
        assert_eq!(mapa.lookup(2), None);
        assert!(diags.contains(&FontModelDiagnostic::PartialTounicode));
    }

    #[test]
    fn tounicode_ausente_deixa_o_mapa_a_none_sem_diagnostico() {
        let (modelo, diags) = build_font_model(&raw(
            RawFontSubtype::TrueType,
            RawFontEncoding::Name("WinAnsiEncoding".to_string()),
        ));
        assert!(modelo.unicode_map.is_none());
        assert!(!diags.contains(&FontModelDiagnostic::PartialTounicode));
    }

    #[test]
    fn identidade_da_fonte_e_preservada() {
        let (modelo, _) = build_font_model(&raw(
            RawFontSubtype::Type1,
            RawFontEncoding::Differences(vec!["/fi".to_string()]),
        ));
        assert_eq!(modelo.resource_name, "f0");
        assert_eq!(modelo.base_font.as_deref(), Some("ABCDEF+NimbusRoman"));
    }
}
