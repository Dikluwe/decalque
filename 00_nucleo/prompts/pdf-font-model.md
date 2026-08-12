# Prompt: modelo de fonte PDF — larguras, decodificação de códigos, `FontModel`

**Camada**: L1 — `01_core`
**Arquivo gerado**: `01_core/src/content/font_model.rs` (novo) + testes no mesmo ficheiro
**ADR**: `00_nucleo/adr/0002-lopdf-backend-parsing.md` (secção "Extração de texto"), `00_nucleo/adr/0003-convencoes-transversais.md` (tipo numérico)

## Contexto

`ToUnicode` resolve apenas `código do glifo → Unicode`. Para posicionar glifos, o intérprete
(`content-stream-text-model.md`) precisa de mais duas coisas por fonte: **largura de cada
glifo** (unidades de glyph space, normalizadas a 1000) e **decodificação da string em códigos
de glifo** (fontes simples: 1 byte/código; fontes Type0/CID com `Identity-H`: 2 bytes BE —
o caso do Typst, a confirmar na implementação). Este prompt define o modelo de fonte puro de
`01_core`: `03_infra` extrai os dados brutos do dicionário `/Font` (via lopdf) e `01_core`
constrói o `FontModel`.

Tipo numérico: `f64` (ADR 0003). Códigos de glifo são inteiros (`u32`) — ADR 0003, "não se
aplica a".

## Restrições estruturais

- L1: zero I/O, **não importa lopdf** nem tipos de `03_infra`. Entrada: dados brutos já
  extraídos por `03_infra` em tipos definidos aqui.

## Instrução

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum GlyphCodeDecoder {
    SingleByte,                 // fontes simples: cada byte é um código
    IdentityH,                  // Type0/CID: códigos de 2 bytes big-endian (caso Typst)
}

#[derive(Debug, Clone, PartialEq)]
pub struct FontWidths {
    pub default_width: f64,             // /DW ou derivado; unidades de glyph space (1000 = 1 em)
    pub widths: Vec<(u32, f64)>,        // (código, largura) já expandido por 01_core
}

#[derive(Debug, Clone, PartialEq)]
pub struct FontModel {
    pub resource_name: String,          // nome no dicionário /Font da página (ex.: "f0")
    pub base_font: Option<String>,      // /BaseFont
    pub decoder: GlyphCodeDecoder,
    pub widths: FontWidths,
    pub unicode_map: Option<CmapMapping>, // ToUnicode parseado (cmap-tounicode-parser.md)
}

impl GlyphCodeDecoder {
    /// Decodifica os bytes crus de uma string mostrada (Tj/TJ) em códigos de glifo.
    /// Bytes que não formam código completo (ex.: byte ímpar final em IdentityH) são
    /// descartados e contados — ver diagnóstico.
    pub fn decode(&self, bytes: &[u8]) -> (Vec<u32>, usize /* bytes descartados */);
}

impl FontWidths {
    /// Largura do glifo em unidades de glyph space; default_width se o código não constar.
    pub fn width_of(&self, glyph_code: u32) -> f64;
}
```

### Dados brutos que `03_infra` entrega por fonte (entrada do construtor)

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum RawFontSubtype { Type0, Type1, TrueType, CIDFontType0, CIDFontType2, Other }

#[derive(Debug, Clone, PartialEq)]
pub enum RawFontEncoding {
    Name(String),              // ex.: "Identity-H", "WinAnsiEncoding"
    Differences(Vec<String>),  // dicionário /Encoding com /Differences — glyph names brutos
    Stream(Vec<u8>),           // CMap de codificação como stream (bytes)
    Absent,
}

pub struct RawFontData {
    pub resource_name: String,
    pub base_font: Option<String>,
    pub subtype: RawFontSubtype,
    pub encoding: RawFontEncoding,
    pub default_width: Option<f64>,     // /DW (do descendente CIDFont em Type0) ou /MissingWidth
    pub widths: Vec<(u32, f64)>,        // já expandido de /Widths+/FirstChar+/LastChar ou /W por 03_infra
    pub tounicode: Option<Vec<u8>>,     // bytes do stream ToUnicode; None se ausente OU ilegível
}

pub fn build_font_model(raw: &RawFontData) -> (FontModel, Vec<FontModelDiagnostic>);

pub enum FontModelDiagnostic {
    UnsupportedEncoding,   // nem SingleByte nem IdentityH — decoder cai em SingleByte e regista
    NoWidths,              // sem /Widths nem /DW — default_width = 1000.0 (fallback documentado)
    PartialTounicode,      // ToUnicode parseado com diagnósticos (propagados do parser CMap)
}
```

Regras:

1. `decoder`: `subtype == Type0 && encoding == Name("Identity-H")` → `IdentityH`; fonte
   simples (`Type1`/`TrueType`) → `SingleByte`; qualquer outra combinação → `SingleByte` +
   `UnsupportedEncoding` (fallback documentado, não falha).
2. `default_width`: `/DW` se presente; senão `1000.0` + `NoWidths` quando não houver tabela
   nenhuma. Larguras ausentes para códigos individuais caem em `default_width` silenciosamente
   (é o que o PDF especifica).
3. **Fontes Type0/CID**: `/DW` e `/W` vivem no dicionário **descendente**
   (`/DescendantFonts[0]`), não no dicionário Type0 — a extracção do descendente é de
   `03_infra` (depende da estrutura de objectos do lopdf); `01_core` recebe já os valores.
4. A expansão de `/Widths` + `/FirstChar`/`/LastChar` (fontes simples) e dos arrays `/W`
   (CIDFont) para pares `(código, largura)` é responsabilidade de **`03_infra`**; `01_core`
   recebe os pares prontos. Documentar esta divisão no doc-comment de `RawFontData`.
5. `tounicode`: `None` tanto para stream ausente quanto para stream **ilegível**
   (incomprimível/corrompido) — `03_infra` trata ToUnicode ilegível como ausente (não fatal;
   `01_core` produzirá glifos `Unmapped`). Documentado em `lopdf-backend-adapter.md`.

## Critérios de verificação

Dado `GlyphCodeDecoder::SingleByte` e bytes `[0x41, 0x42]`
Quando `decode` é chamado
Então devolve `([0x41, 0x42], 0)`

Dado `GlyphCodeDecoder::IdentityH` e bytes `[0x00, 0x01, 0x00, 0x02]`
Quando `decode` é chamado
Então devolve `([1, 2], 0)`

Dado `IdentityH` e bytes com comprimento ímpar `[0x00, 0x01, 0xFF]`
Quando `decode` é chamado
Então devolve `([1], 1)` (o byte órfão é descartado e contado)

Dado `FontWidths { default_width: 500.0, widths: [(1, 600.0)] }`
Quando `width_of(1)` e `width_of(99)` são chamados
Então devolvem `600.0` e `500.0` respectivamente

Dado `RawFontData` de fonte simples sem `/Widths` e sem `/DW`
Quando `build_font_model` é chamado
Então `decoder == SingleByte`, `default_width == 1000.0` e o diagnóstico `NoWidths` é emitido

Dado `RawFontData` Type0 com `Identity-H`
Quando `build_font_model` é chamado
Então `decoder == IdentityH`, sem diagnóstico de codificação

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| 2026-08-12 | Criação — análise do dono sobre `content-stream-text-model.md`: `ToUnicode` não fornece larguras nem decodificação de códigos; modelo de fonte separado antes de gerar o intérprete | `font_model.rs` (novo) |
| 2026-08-12 | Revisão do dono (análise do adapter): `RawFontData` estendida — `RawFontSubtype`, `RawFontEncoding` (Name/Differences/Stream/Absent), regra de descendentes CID (`/DW`/`/W` em `/DescendantFonts[0]`), `/MissingWidth`, ToUnicode ilegível tratado como ausente | `font_model.rs` (novo) |
