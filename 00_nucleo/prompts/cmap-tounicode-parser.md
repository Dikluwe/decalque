# Prompt: parser de `ToUnicode` / CMap

**Camada**: L1 — `01_core`
**Arquivo gerado**: `01_core/src/content/cmap.rs` (novo) + testes no mesmo ficheiro
**ADR**: `00_nucleo/adr/0002-lopdf-backend-parsing.md` (secção "Parser de ToUnicode / CMap")

## Contexto

O lopdf não exporta parser de CMap (verificado no experimento `_lab/lopdf_probe/`). Sem este
parser, `GlyphInstance.codepoints` fica vazio e o emparelhamento por conteúdo
(`engine/compare.md`) perde a âncora principal. Decisão do dono (2026-08-12): parser **puro**
em `01_core` — recebe bytes, devolve estrutura de mapeamento; `03_infra` apenas obtém o stream
`ToUnicode` e passa os dados.

## Restrições estruturais

- L1: zero I/O, **não importa lopdf**. Entrada: bytes do stream (já descomprimidos por
  `03_infra`).
- **Não falhar de forma fatal**: CMap malformado produz diagnósticos, não erro de documento.
  Mapeamentos válidos extraídos antes/depois da parte inválida são preservados.

## Instrução

```rust
pub struct CmapMapping {
    pub entries: Vec<CmapEntry>,
}

pub enum CmapEntry {
    Char { src: u32, dst: Vec<char> },                       // bfchar; dst pode ser sequência (ligadura, ADR 0001)
    Range { src_start: u32, src_end: u32, dst_start: Vec<char> }, // bfrange simples
}

pub struct CmapParseResult {
    pub mapping: CmapMapping,
    pub diagnostics: Vec<CmapDiagnostic>,
}

pub enum CmapDiagnostic {
    EmptyInput,
    UnknownOperator,      // operador/keyword fora do conjunto mínimo — ignorado, registado
    InvalidRange,         // src_end < src_start, ou contagem declarada ≠ contagem de entradas
    TruncatedMapping,     // secção termina sem o marcador end* correspondente
    InvalidUnicode,       // destino não é sequência Unicode válida — entrada descartada
}

pub fn parse_tounicode_cmap(bytes: &[u8]) -> CmapParseResult;
```

Formato mínimo a suportar:

```text
begincodespacerange / endcodespacerange
beginbfchar / endbfchar
beginbfrange / endbfrange
```

Regras:

1. `codespacerange` define a largura dos códigos-fonte; a v1 pode registar os intervalos sem
   os usar activamente (os códigos chegam já decodificados do content stream) — documentar.
2. `bfchar`: `<src> <dst>` por linha; `dst` é uma string hexadecimal UTF-16BE que pode mapear
   para **vários** codepoints (ligaduras — motivo de `dst: Vec<char>`, ver ADR 0001).
3. `bfrange`: duas formas — `<start> <end> <dst_start>` (destinos consecutivos) e
   `<start> <end> [<d0> <d1> ...]` (array explícito). A forma de array pode ser expandida em
   entradas `Char` individuais, ou guardada como `Range` — escolher e documentar.
4. Destino que não decodifica como UTF-16BE válido → entrada descartada +
   `CmapDiagnostic::InvalidUnicode` (nunca inventar texto, coerente com
   `content-stream-text-model.md`).
5. Operador desconhecido → ignorado + `UnknownOperator` (CMaps reais trazem mais keywords:
   `begincmap`, `usecmap`, WMode etc.).
6. Entrada vazia → mapeamento vazio + `EmptyInput`.

API de consulta (usada por `content/text_interpreter.rs`):

```rust
impl CmapMapping {
    pub fn lookup(&self, glyph_code: u32) -> Option<Vec<char>>;
}
```

## Critérios de verificação

Dado um CMap com `beginbfchar` contendo `<0001> <0041>`
Quando `parse_tounicode_cmap` corre e `lookup(0x0001)` é chamado
Então devolve `Some(['A'])` e não há diagnósticos

Dado um `bfchar` cujo destino é `<00660069>` ("fi" em UTF-16BE, dois codepoints)
Quando `lookup` é chamado
Então devolve `Some(['f', 'i'])` (ligadura preservada como sequência, ADR 0001)

Dado um `bfrange` `<0001> <0003> <0041>`
Quando `lookup(0x0002)` é chamado
Então devolve `Some(['B'])` (destinos consecutivos)

Dado um `bfrange` em forma de array `<0001> <0002> [<0041> <0042>]`
Quando `lookup(0x0002)` é chamado
Então devolve `Some(['B'])`

Dado uma secção `beginbfchar` sem `endbfchar`
Quando o parse corre
Então as entradas lidas até ao fim são preservadas e `TruncatedMapping` é registado

Dado uma entrada cujo destino não é UTF-16BE válido
Quando o parse corre
Então a entrada é descartada, `InvalidUnicode` é registado, e as restantes entradas sobrevivem

Dado bytes vazios
Quando o parse corre
Então mapeamento vazio + `EmptyInput`, sem pânico

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| 2026-08-12 | Criação — restrições do dono: parser puro sem lopdf; resultado com diagnósticos em vez de falha total; comportamento definido para entrada vazia, operador desconhecido, intervalo inválido, truncamento, Unicode inválido | `cmap.rs` (novo) |
