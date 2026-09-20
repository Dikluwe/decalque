//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/cmap-tounicode-parser.md
//! @layer L1
//! @updated 2026-08-13
//!
//! Parser de `ToUnicode`/CMap. O lopdf não exporta parser de CMap (verificado
//! em `_lab/lopdf_probe/`), e sem este mapeamento `GlyphInstance.codepoints`
//! fica vazio e o emparelhamento por conteúdo perde a âncora principal.
//!
//! Recebe os bytes do stream já descomprimidos por `03_infra` e devolve
//! mapeamento + diagnósticos: **um CMap malformado nunca é erro fatal** — os
//! mapeamentos válidos ao redor do trecho inválido são preservados.

/// Uma entrada de mapeamento `código de glifo → Unicode`.
#[derive(Debug, Clone, PartialEq)]
pub enum CmapEntry {
    /// `bfchar`: um código para uma sequência de codepoints.
    ///
    /// `dst` é sequência e não `char` porque uma ligadura mapeia para vários
    /// codepoints (o glifo "fi" mapeia para `['f','i']` — ADR 0001).
    Char { src: u32, dst: Vec<char> },
    /// `bfrange` consecutivo: `src_start..=src_end` mapeia para
    /// `dst_start`, `dst_start + 1`, …
    ///
    /// `dst_start` é **um** scalar: não existe regra de incremento para
    /// sequências multi-caractere (ver `UnsupportedRangeDestination`).
    Range {
        src_start: u32,
        src_end: u32,
        dst_start: char,
    },
}

/// Mapeamento completo extraído de um stream `ToUnicode`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CmapMapping {
    /// Entradas na ordem em que foram lidas — a ordem importa para `lookup`.
    pub entries: Vec<CmapEntry>,
}

/// Resultado do parse: mapeamento mais o que correu mal pelo caminho.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CmapParseResult {
    pub mapping: CmapMapping,
    pub diagnostics: Vec<CmapDiagnostic>,
}

/// O que correu mal durante o parse. Nenhum destes é fatal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmapDiagnostic {
    /// Entrada sem bytes nenhuns.
    EmptyInput,
    /// Token inesperado dentro de uma secção de mapeamento, ou `end*` sem o
    /// `begin*` correspondente.
    UnknownOperator,
    /// Dígito não-hexadecimal, número ímpar de dígitos, ou string hex vazia.
    InvalidHex,
    /// `src_end < src_start`; contagem declarada ≠ lida; tamanho de array ≠
    /// tamanho do intervalo; destino do intervalo sai do Unicode válido.
    InvalidRange,
    /// Secção sem o marcador `end*` correspondente até ao fim da entrada.
    TruncatedMapping,
    /// Destino decodifica como hex mas não é UTF-16BE válido — entrada
    /// descartada, nunca substituída por texto inventado.
    InvalidUnicode,
    /// `bfrange` consecutivo cujo destino inicial decodifica para zero ou
    /// mais de um `char`.
    UnsupportedRangeDestination,
}

impl CmapMapping {
    /// Codepoints de um código de glifo, se mapeado.
    ///
    /// **Precedência**: as entradas são consultadas na ordem de parsing e a
    /// **primeira correspondência vence** — inclui duplicatas e sobreposições
    /// entre `Char` e `Range`. Determinístico e simples.
    ///
    /// **Limitação registada (v1)**: consulta **linear**, `O(n)`, e devolve
    /// `Vec<char>` por valor (uma alocação por consulta). Aceitável para os
    /// `ToUnicode` pequenos que se observam na prática; optimizar (índice,
    /// `&[char]`) só perante dados reais de desempenho.
    pub fn lookup(&self, glyph_code: u32) -> Option<Vec<char>> {
        for entry in &self.entries {
            match entry {
                CmapEntry::Char { src, dst } if *src == glyph_code => {
                    return Some(dst.clone());
                }
                CmapEntry::Range {
                    src_start,
                    src_end,
                    dst_start,
                } if glyph_code >= *src_start && glyph_code <= *src_end => {
                    let offset = glyph_code - src_start;
                    // O parse validou que todo o intervalo de destino é
                    // scalar válido; o `None` aqui é defesa, não caminho real.
                    return char::from_u32(*dst_start as u32 + offset).map(|c| vec![c]);
                }
                _ => {}
            }
        }
        None
    }
}

/// Parseia um stream `ToUnicode` (bytes já descomprimidos por `03_infra`).
///
/// `codespacerange` é reconhecido mas **não armazenado** na v1: os códigos
/// chegam ao intérprete já decodificados pelo `GlyphCodeDecoder`
/// (`pdf-font-model.md`), pelo que a informação de espaço de códigos não é
/// necessária para o mapeamento.
pub fn parse_tounicode_cmap(bytes: &[u8]) -> CmapParseResult {
    let mut resultado = CmapParseResult::default();
    if bytes.is_empty() {
        resultado.diagnostics.push(CmapDiagnostic::EmptyInput);
        return resultado;
    }

    let tokens = tokenizar(bytes);
    let mut estado = Seccao::Nenhuma;
    let mut contagem_declarada: Option<usize> = None;
    let mut contagem_da_seccao: Option<usize> = None;
    let mut linhas_lidas = 0usize;
    let mut pendentes: Vec<Vec<u8>> = Vec::new();
    let mut array: Option<Vec<Vec<u8>>> = None;

    for token in tokens {
        match token {
            Token::Palavra(palavra) => match classificar(&palavra) {
                Palavra::Begin(nova) => {
                    if estado != Seccao::Nenhuma {
                        // `begin*` dentro de secção: malformado, mas não fatal.
                        resultado.diagnostics.push(CmapDiagnostic::UnknownOperator);
                    }
                    estado = nova;
                    contagem_da_seccao = contagem_declarada.take();
                    linhas_lidas = 0;
                    pendentes.clear();
                    array = None;
                }
                Palavra::End(fecha) => {
                    if estado == Seccao::Nenhuma || estado != fecha {
                        // `end*` sem `begin*`, ou a fechar outra secção.
                        resultado.diagnostics.push(CmapDiagnostic::UnknownOperator);
                    }
                    if estado != Seccao::Nenhuma {
                        if let Some(esperadas) = contagem_da_seccao {
                            if esperadas != linhas_lidas {
                                resultado.diagnostics.push(CmapDiagnostic::InvalidRange);
                            }
                        }
                    }
                    estado = Seccao::Nenhuma;
                    contagem_da_seccao = None;
                    pendentes.clear();
                    array = None;
                }
                Palavra::Inteiro(n) => {
                    if estado == Seccao::Nenhuma {
                        // Contagem declarada antes do `begin*`.
                        contagem_declarada = Some(n);
                    } else {
                        resultado.diagnostics.push(CmapDiagnostic::UnknownOperator);
                    }
                }
                Palavra::Outra => {
                    if estado != Seccao::Nenhuma {
                        // Só dentro de secção de mapeamento é ruído.
                        resultado.diagnostics.push(CmapDiagnostic::UnknownOperator);
                    }
                    // Fora de secção: `begincmap`, `def`, `/Nomes`, `usecmap`,
                    // `findresource`… ignorados sem diagnóstico.
                }
            },
            Token::Hex(dados) => {
                if let Some(itens) = array.as_mut() {
                    itens.push(dados);
                    continue;
                }
                match estado {
                    Seccao::Nenhuma => {}
                    Seccao::Codespace => {
                        pendentes.push(dados);
                        if pendentes.len() == 2 {
                            // Reconhecido, não armazenado.
                            linhas_lidas += 1;
                            pendentes.clear();
                        }
                    }
                    Seccao::BfChar => {
                        pendentes.push(dados);
                        if pendentes.len() == 2 {
                            linhas_lidas += 1;
                            adicionar_bfchar(&pendentes[0], &pendentes[1], &mut resultado);
                            pendentes.clear();
                        }
                    }
                    Seccao::BfRange => {
                        pendentes.push(dados);
                        if pendentes.len() == 3 {
                            linhas_lidas += 1;
                            adicionar_bfrange_consecutivo(
                                &pendentes[0],
                                &pendentes[1],
                                &pendentes[2],
                                &mut resultado,
                            );
                            pendentes.clear();
                        }
                    }
                }
            }
            Token::HexInvalido => {
                if estado != Seccao::Nenhuma {
                    resultado.diagnostics.push(CmapDiagnostic::InvalidHex);
                    if array.is_none() {
                        // Entrada corrente descartada; conta como linha lida
                        // para a contagem declarada bater com o ficheiro.
                        linhas_lidas += 1;
                        pendentes.clear();
                    }
                }
            }
            Token::AbreArray => {
                if estado == Seccao::BfRange && pendentes.len() == 2 && array.is_none() {
                    array = Some(Vec::new());
                } else {
                    resultado.diagnostics.push(CmapDiagnostic::UnknownOperator);
                }
            }
            Token::FechaArray => match array.take() {
                Some(itens) => {
                    linhas_lidas += 1;
                    adicionar_bfrange_array(&pendentes[0], &pendentes[1], &itens, &mut resultado);
                    pendentes.clear();
                }
                None => resultado.diagnostics.push(CmapDiagnostic::UnknownOperator),
            },
        }
    }

    if estado != Seccao::Nenhuma {
        // Entradas já lidas ficam preservadas.
        resultado.diagnostics.push(CmapDiagnostic::TruncatedMapping);
    }

    resultado
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seccao {
    Nenhuma,
    Codespace,
    BfChar,
    BfRange,
}

enum Token {
    Hex(Vec<u8>),
    HexInvalido,
    AbreArray,
    FechaArray,
    Palavra(String),
}

enum Palavra {
    Begin(Seccao),
    End(Seccao),
    Inteiro(usize),
    Outra,
}

fn classificar(palavra: &str) -> Palavra {
    match palavra {
        "begincodespacerange" => Palavra::Begin(Seccao::Codespace),
        "beginbfchar" => Palavra::Begin(Seccao::BfChar),
        "beginbfrange" => Palavra::Begin(Seccao::BfRange),
        "endcodespacerange" => Palavra::End(Seccao::Codespace),
        "endbfchar" => Palavra::End(Seccao::BfChar),
        "endbfrange" => Palavra::End(Seccao::BfRange),
        outra => match outra.parse::<usize>() {
            Ok(n) => Palavra::Inteiro(n),
            Err(_) => Palavra::Outra,
        },
    }
}

/// Divide os bytes em tokens. Hex é case-insensitive; dentro de `<...>` só
/// dígitos hex e whitespace são aceites.
fn tokenizar(bytes: &[u8]) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if e_espaco(b) {
            i += 1;
        } else if b == b'<' && bytes.get(i + 1) == Some(&b'<') {
            // Delimitador de dicionário PDF: prosa estrutural do CMap, não hex.
            i += 2;
        } else if b == b'<' {
            let inicio = i + 1;
            match bytes[inicio..].iter().position(|&c| c == b'>') {
                Some(offset) => {
                    let fim = inicio + offset;
                    tokens.push(ler_hex(&bytes[inicio..fim]));
                    i = fim + 1;
                }
                None => {
                    // `<` sem `>` até ao fim: string hex truncada.
                    tokens.push(Token::HexInvalido);
                    i = bytes.len();
                }
            }
        } else if b == b'[' {
            tokens.push(Token::AbreArray);
            i += 1;
        } else if b == b']' {
            tokens.push(Token::FechaArray);
            i += 1;
        } else if b"{}>".contains(&b) {
            // Delimitadores estruturais não têm semântica nas secções
            // suportadas. O avanço explícito evita laço em entrada real.
            i += 1;
        } else {
            let inicio = i;
            while i < bytes.len() && !e_espaco(bytes[i]) && !b"<>[]{}".contains(&bytes[i]) {
                i += 1;
            }
            tokens.push(Token::Palavra(
                String::from_utf8_lossy(&bytes[inicio..i]).into_owned(),
            ));
        }
    }
    tokens
}

fn e_espaco(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'\x0c' | b'\0')
}

/// Converte o interior de `<...>` em bytes. Whitespace é ignorado; qualquer
/// outro caractere não-hex, comprimento ímpar ou conteúdo vazio invalidam.
fn ler_hex(conteudo: &[u8]) -> Token {
    let mut digitos = Vec::new();
    for &c in conteudo {
        if e_espaco(c) {
            continue;
        }
        match (c as char).to_digit(16) {
            Some(d) => digitos.push(d as u8),
            None => return Token::HexInvalido,
        }
    }
    if digitos.is_empty() || digitos.len() % 2 != 0 {
        return Token::HexInvalido;
    }
    Token::Hex(
        digitos
            .chunks(2)
            .map(|par| (par[0] << 4) | par[1])
            .collect(),
    )
}

/// Inteiro big-endian a partir dos bytes do código.
///
/// Códigos com mais de 4 bytes não existem na prática; o `wrapping_shl` evita
/// pânico e mantém os 4 bytes menos significativos.
fn inteiro_be(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .fold(0u32, |acc, &b| acc.wrapping_shl(8) | b as u32)
}

/// Descodifica UTF-16BE. `None` se o comprimento for ímpar em bytes ou se as
/// unidades não formarem Unicode válido (surrogate solitário, por exemplo).
fn utf16be(bytes: &[u8]) -> Option<Vec<char>> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let unidades: Vec<u16> = bytes
        .chunks(2)
        .map(|par| u16::from_be_bytes([par[0], par[1]]))
        .collect();
    char::decode_utf16(unidades)
        .collect::<Result<Vec<char>, _>>()
        .ok()
        .filter(|chars| !chars.is_empty())
}

fn adicionar_bfchar(src: &[u8], dst: &[u8], resultado: &mut CmapParseResult) {
    match utf16be(dst) {
        Some(chars) => resultado.mapping.entries.push(CmapEntry::Char {
            src: inteiro_be(src),
            dst: chars,
        }),
        None => resultado.diagnostics.push(CmapDiagnostic::InvalidUnicode),
    }
}

fn adicionar_bfrange_consecutivo(
    inicio: &[u8],
    fim: &[u8],
    dst: &[u8],
    resultado: &mut CmapParseResult,
) {
    let src_start = inteiro_be(inicio);
    let src_end = inteiro_be(fim);
    if src_end < src_start {
        resultado.diagnostics.push(CmapDiagnostic::InvalidRange);
        return;
    }
    let chars = match utf16be(dst) {
        Some(chars) => chars,
        None => {
            resultado.diagnostics.push(CmapDiagnostic::InvalidUnicode);
            return;
        }
    };
    if chars.len() != 1 {
        // Sem regra de incremento para sequências multi-caractere.
        resultado
            .diagnostics
            .push(CmapDiagnostic::UnsupportedRangeDestination);
        return;
    }
    let dst_start = chars[0];
    if !intervalo_de_destino_valido(dst_start, src_end - src_start) {
        resultado.diagnostics.push(CmapDiagnostic::InvalidRange);
        return;
    }
    resultado.mapping.entries.push(CmapEntry::Range {
        src_start,
        src_end,
        dst_start,
    });
}

/// Verifica que **todo** o intervalo de destino é scalar Unicode válido.
///
/// A spec exige validar o valor final; esta implementação valida o intervalo
/// inteiro, que é estritamente mais forte: um intervalo pode ter extremos
/// válidos e atravessar o bloco de surrogates (`U+D800..=U+DFFF`), e nesse
/// caso `lookup` de um código intermédio não teria destino. Ver histórico do
/// prompt, 2026-08-13.
fn intervalo_de_destino_valido(dst_start: char, extensao: u32) -> bool {
    let inicio = dst_start as u32;
    match inicio.checked_add(extensao) {
        None => false,
        Some(fim) => (inicio..=fim).all(|v| char::from_u32(v).is_some()),
    }
}

fn adicionar_bfrange_array(
    inicio: &[u8],
    fim: &[u8],
    itens: &[Vec<u8>],
    resultado: &mut CmapParseResult,
) {
    let src_start = inteiro_be(inicio);
    let src_end = inteiro_be(fim);
    if src_end < src_start {
        resultado.diagnostics.push(CmapDiagnostic::InvalidRange);
        return;
    }
    let esperados = (src_end - src_start + 1) as usize;
    for (i, item) in itens.iter().take(esperados).enumerate() {
        match utf16be(item) {
            // Cada destino pode ser sequência: cobre ligaduras dentro do array.
            Some(chars) => resultado.mapping.entries.push(CmapEntry::Char {
                src: src_start + i as u32,
                dst: chars,
            }),
            None => resultado.diagnostics.push(CmapDiagnostic::InvalidUnicode),
        }
    }
    if itens.len() != esperados {
        // Pares válidos preservados; excedentes ignorados.
        resultado.diagnostics.push(CmapDiagnostic::InvalidRange);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(texto: &str) -> CmapParseResult {
        parse_tounicode_cmap(texto.as_bytes())
    }

    #[test]
    fn bfchar_simples_mapeia_e_nao_diagnostica() {
        let r = parse("beginbfchar <0001> <0041> endbfchar");
        assert_eq!(r.mapping.lookup(0x0001), Some(vec!['A']));
        assert!(
            r.diagnostics.is_empty(),
            "diagnósticos: {:?}",
            r.diagnostics
        );
    }

    #[test]
    fn bfchar_com_destino_de_dois_codepoints_preserva_a_ligadura() {
        // <00660069> = "fi" em UTF-16BE.
        let r = parse("beginbfchar <0001> <00660069> endbfchar");
        assert_eq!(r.mapping.lookup(0x0001), Some(vec!['f', 'i']));
    }

    #[test]
    fn bfrange_consecutivo_incrementa_o_destino() {
        let r = parse("beginbfrange <0001> <0003> <0041> endbfrange");
        assert_eq!(r.mapping.lookup(0x0001), Some(vec!['A']));
        assert_eq!(r.mapping.lookup(0x0002), Some(vec!['B']));
        assert_eq!(r.mapping.lookup(0x0003), Some(vec!['C']));
        assert_eq!(r.mapping.lookup(0x0004), None);
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn bfrange_em_array_expande_em_entradas_individuais() {
        let r = parse("beginbfrange <0001> <0002> [<0041> <0042>] endbfrange");
        assert_eq!(r.mapping.lookup(0x0001), Some(vec!['A']));
        assert_eq!(r.mapping.lookup(0x0002), Some(vec!['B']));
        assert!(r.diagnostics.is_empty());
        // Expandido como `Char`, não como `Range`.
        assert!(r
            .mapping
            .entries
            .iter()
            .all(|e| matches!(e, CmapEntry::Char { .. })));
    }

    #[test]
    fn bfrange_consecutivo_com_destino_multi_caractere_e_descartado() {
        let r = parse("beginbfrange <0001> <0003> <00660069> endbfrange");
        assert!(r.mapping.entries.is_empty());
        assert!(r
            .diagnostics
            .contains(&CmapDiagnostic::UnsupportedRangeDestination));
    }

    #[test]
    fn bfrange_em_array_mais_curto_preserva_os_pares_validos() {
        let r = parse("beginbfrange <0001> <0003> [<0041>] endbfrange");
        assert_eq!(r.mapping.lookup(0x0001), Some(vec!['A']));
        assert_eq!(r.mapping.lookup(0x0002), None);
        assert!(r.diagnostics.contains(&CmapDiagnostic::InvalidRange));
    }

    #[test]
    fn destino_com_digito_nao_hex_e_invalid_hex_e_nao_invalid_unicode() {
        let r = parse("beginbfchar <0001> <004G> endbfchar");
        assert!(r.mapping.entries.is_empty());
        assert!(r.diagnostics.contains(&CmapDiagnostic::InvalidHex));
        assert!(!r.diagnostics.contains(&CmapDiagnostic::InvalidUnicode));
    }

    #[test]
    fn destino_vazio_e_invalid_hex() {
        let r = parse("beginbfchar <0001> <> endbfchar");
        assert!(r.mapping.entries.is_empty());
        assert!(r.diagnostics.contains(&CmapDiagnostic::InvalidHex));
    }

    #[test]
    fn comprimento_impar_de_digitos_e_invalid_hex() {
        let r = parse("beginbfchar <001> <0041> endbfchar");
        assert!(r.mapping.entries.is_empty());
        assert!(r.diagnostics.contains(&CmapDiagnostic::InvalidHex));
    }

    #[test]
    fn contagem_declarada_diferente_da_lida_emite_invalid_range() {
        let r = parse("2 beginbfchar <0001> <0041> endbfchar");
        // A entrada lida é preservada.
        assert_eq!(r.mapping.lookup(0x0001), Some(vec!['A']));
        assert!(r.diagnostics.contains(&CmapDiagnostic::InvalidRange));
    }

    #[test]
    fn contagem_declarada_correcta_nao_diagnostica() {
        let r = parse("1 beginbfchar <0001> <0041> endbfchar");
        assert!(
            r.diagnostics.is_empty(),
            "diagnósticos: {:?}",
            r.diagnostics
        );
    }

    #[test]
    fn duplicata_de_codigo_fonte_primeira_correspondencia_vence() {
        let r = parse("beginbfchar <0001> <0041> <0001> <0042> endbfchar");
        assert_eq!(r.mapping.lookup(0x0001), Some(vec!['A']));
    }

    #[test]
    fn sobreposicao_range_antes_de_char_vence_o_range() {
        let r = parse(
            "beginbfrange <0001> <0003> <0041> endbfrange \
             beginbfchar <0002> <0058> endbfchar",
        );
        assert_eq!(r.mapping.lookup(0x0002), Some(vec!['B']));
    }

    #[test]
    fn seccao_sem_end_preserva_o_lido_e_diagnostica_truncatura() {
        let r = parse("beginbfchar <0001> <0041>");
        assert_eq!(r.mapping.lookup(0x0001), Some(vec!['A']));
        assert!(r.diagnostics.contains(&CmapDiagnostic::TruncatedMapping));
    }

    #[test]
    fn end_sem_begin_e_ignorado_com_unknown_operator() {
        let r = parse("endbfchar");
        assert!(r.mapping.entries.is_empty());
        assert!(r.diagnostics.contains(&CmapDiagnostic::UnknownOperator));
    }

    #[test]
    fn duas_seccoes_consecutivas_acumulam_entradas() {
        let r = parse(
            "beginbfchar <0001> <0041> endbfchar \
             beginbfchar <0002> <0042> endbfchar",
        );
        assert_eq!(r.mapping.lookup(0x0001), Some(vec!['A']));
        assert_eq!(r.mapping.lookup(0x0002), Some(vec!['B']));
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn entrada_vazia_devolve_mapeamento_vazio_sem_panico() {
        let r = parse_tounicode_cmap(&[]);
        assert!(r.mapping.entries.is_empty());
        assert_eq!(r.diagnostics, vec![CmapDiagnostic::EmptyInput]);
    }

    #[test]
    fn prosa_de_cmap_real_nao_gera_unknown_operator() {
        let r = parse(
            "/CIDInit /ProcSet findresource begin\n\
             12 dict begin\n\
             begincmap\n\
             /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
             /CMapName /Adobe-Identity-UCS def\n\
             /CMapType 2 def\n\
             1 begincodespacerange\n\
             <0000> <FFFF>\n\
             endcodespacerange\n\
             1 beginbfchar\n\
             <0001> <004F>\n\
             endbfchar\n\
             endcmap\n\
             CMapName currentdict /CMap defineresource pop\n\
             end\n\
             end",
        );
        assert_eq!(r.mapping.lookup(0x0001), Some(vec!['O']));
        assert!(
            !r.diagnostics.contains(&CmapDiagnostic::UnknownOperator),
            "diagnósticos: {:?}",
            r.diagnostics
        );
    }

    #[test]
    fn delimitadores_estruturais_nao_sao_hex_nem_travam_o_tokenizador() {
        let r = parse("<< /Metadata { ignored } >> 1 beginbfchar <0001> <0041> endbfchar");
        assert_eq!(r.mapping.lookup(0x0001), Some(vec!['A']));
        assert!(
            r.diagnostics.is_empty(),
            "diagnósticos: {:?}",
            r.diagnostics
        );
    }

    #[test]
    fn codespacerange_e_reconhecido_mas_nao_armazenado() {
        let r = parse("1 begincodespacerange <0000> <FFFF> endcodespacerange");
        assert!(r.mapping.entries.is_empty());
        assert!(
            r.diagnostics.is_empty(),
            "diagnósticos: {:?}",
            r.diagnostics
        );
    }

    #[test]
    fn src_end_menor_que_src_start_e_invalid_range() {
        let r = parse("beginbfrange <0003> <0001> <0041> endbfrange");
        assert!(r.mapping.entries.is_empty());
        assert!(r.diagnostics.contains(&CmapDiagnostic::InvalidRange));
    }

    #[test]
    fn hex_e_case_insensitive() {
        let minusculas = parse("beginbfchar <000a> <00e1> endbfchar");
        let maiusculas = parse("beginbfchar <000A> <00E1> endbfchar");
        assert_eq!(minusculas.mapping, maiusculas.mapping);
        assert_eq!(minusculas.mapping.lookup(0x0A), Some(vec!['á']));
    }

    #[test]
    fn destino_surrogate_solitario_e_invalid_unicode() {
        // <D800> isolado não é UTF-16BE válido.
        let r = parse("beginbfchar <0001> <D800> endbfchar");
        assert!(r.mapping.entries.is_empty());
        assert!(r.diagnostics.contains(&CmapDiagnostic::InvalidUnicode));
        assert!(!r.diagnostics.contains(&CmapDiagnostic::InvalidHex));
    }

    #[test]
    fn par_de_surrogates_valido_e_aceite() {
        // <D83DDE00> = U+1F600.
        let r = parse("beginbfchar <0001> <D83DDE00> endbfchar");
        assert_eq!(r.mapping.lookup(0x0001), Some(vec!['\u{1F600}']));
        assert!(r.diagnostics.is_empty());
    }

    #[test]
    fn bfrange_que_atravessa_o_bloco_de_surrogates_e_invalid_range() {
        // U+D7FF + 4 atravessa U+D800..U+DFFF: nenhum código intermédio teria
        // destino válido, apesar de o extremo final ser scalar.
        let r = parse("beginbfrange <0001> <0800> <D7FF> endbfrange");
        assert!(r.mapping.entries.is_empty());
        assert!(r.diagnostics.contains(&CmapDiagnostic::InvalidRange));
    }

    #[test]
    fn lookup_de_codigo_ausente_devolve_none() {
        let r = parse("beginbfchar <0001> <0041> endbfchar");
        assert_eq!(r.mapping.lookup(0x0099), None);
    }

    #[test]
    fn token_inesperado_dentro_de_seccao_gera_unknown_operator() {
        let r = parse("beginbfchar <0001> <0041> lixo endbfchar");
        assert_eq!(r.mapping.lookup(0x0001), Some(vec!['A']));
        assert!(r.diagnostics.contains(&CmapDiagnostic::UnknownOperator));
    }

    #[test]
    fn hex_truncado_sem_fecho_nao_entra_em_panico() {
        let r = parse("beginbfchar <0001> <0041");
        assert!(r.diagnostics.contains(&CmapDiagnostic::InvalidHex));
        assert!(r.diagnostics.contains(&CmapDiagnostic::TruncatedMapping));
    }
}
