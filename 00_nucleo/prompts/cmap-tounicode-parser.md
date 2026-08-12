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

Tipo numérico: códigos de glifo e destinos são inteiros/caracteres (ADR 0003, "não se aplica
a" — `f64` não entra aqui).

## Restrições estruturais

- L1: zero I/O, **não importa lopdf**. Entrada: bytes do stream (já descomprimidos por
  `03_infra`).
- **Não falhar de forma fatal**: CMap malformado produz diagnósticos, não erro de documento.
  Mapeamentos válidos extraídos ao redor do trecho inválido são preservados.

## Instrução

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum CmapEntry {
    Char { src: u32, dst: Vec<char> },              // bfchar; dst pode ser sequência (ligadura, ADR 0001)
    Range { src_start: u32, src_end: u32, dst_start: char },  // bfrange consecutivo; dst_start é UM scalar
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CmapMapping {
    pub entries: Vec<CmapEntry>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CmapParseResult {
    pub mapping: CmapMapping,
    pub diagnostics: Vec<CmapDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmapDiagnostic {
    EmptyInput,
    UnknownOperator,               // token inesperado dentro de secção de mapeamento; end sem begin
    InvalidHex,                    // dígito não-hex, comprimento ímpar, string hex vazia
    InvalidRange,                  // src_end < src_start; contagem declarada ≠ lida; array ≠ intervalo; destino final fora do Unicode
    TruncatedMapping,              // secção sem o marcador end* correspondente
    InvalidUnicode,                // destino bfchar não é UTF-16BE válido — entrada descartada
    UnsupportedRangeDestination,   // bfrange consecutivo cujo destino inicial decodifica para 0 ou >1 char
}

pub fn parse_tounicode_cmap(bytes: &[u8]) -> CmapParseResult;
```

### Regras de parsing hexadecimal

- Hex é **case-insensitive**.
- Dentro de `<...>`, apenas dígitos hex e whitespace são aceitos.
- Comprimento ímpar de dígitos → `InvalidHex`, entrada descartada.
- Dígito não-hexadecimal → `InvalidHex`, entrada descartada.
- String hex **vazia** (`<>`) → `InvalidHex`, entrada descartada (também como destino —
  não criar mapeamento para sequência vazia).
- Erros de sintaxe hex emitem `InvalidHex`, nunca `InvalidUnicode` (este é só para destino
  que decodifica hex validamente mas não é UTF-16BE válido).

### Regras de secções e tokens

- Secções mínimas: `begincodespacerange`/`endcodespacerange`, `beginbfchar`/`endbfchar`,
  `beginbfrange`/`endbfrange`.
- **Múltiplas secções** do mesmo tipo são permitidas; entradas acumulam na ordem.
- Um inteiro pode preceder `beginbfchar`/`beginbfrange`/`begincodespacerange` (contagem
  declarada). Se a contagem ≠ entradas lidas até o `end` correspondente → `InvalidRange`;
  entradas válidas já lidas são preservadas. Ausência do contador é aceita.
- `end` sem `begin` correspondente → ignorado + `UnknownOperator`.
- Palavras-chave comuns de CMap **fora** das secções de mapeamento (`begincmap`, `endcmap`,
  `def`, `dict`, `begin`, `findresource`, `usecmap`, `WMode`, `beginnotdef*`, `/Nomes` etc.)
  são ignoradas **sem** diagnóstico — `UnknownOperator` só para tokens inesperados **dentro**
  das secções de mapeamento (evita ruído em CMaps reais).
- `begin` sem `end` correspondente até ao fim → entradas lidas preservadas + `TruncatedMapping`.
- `codespacerange`: reconhecido, **não armazenado** na v1 — os códigos chegam decodificados
  pelo `GlyphCodeDecoder` (`pdf-font-model.md`). Documentar no doc-comment.
- Entrada vazia → mapeamento vazio + `EmptyInput`, sem pânico.

### Regras de mapeamento

1. `bfchar`: `<src> <dst>`; `dst` é UTF-16BE e pode mapear para **vários** codepoints
   (ligaduras — `dst: Vec<char>`, ADR 0001). Destino que não decodifica como UTF-16BE válido
   → entrada descartada + `InvalidUnicode` (nunca inventar texto).
2. `bfrange` consecutivo (`<start> <end> <dst_start>`): suportado **apenas** quando
   `dst_start` decodifica para **exactamente um** `char`. Se decodificar para zero ou vários
   → entrada descartada + `UnsupportedRangeDestination` (não existe regra de incremento para
   sequência multi-caractere). Validar também que `dst_start + (src_end − src_start)` continua
   scalar Unicode válido; se não → `InvalidRange`.
3. `bfrange` com array (`<start> <end> [<d0> <d1> ...]`): **expandido em entradas `Char`
   individuais** (cada destino pode ser sequência, cobrindo ligaduras). Se o tamanho do array
   ≠ tamanho do intervalo → `InvalidRange`, preservando os pares válidos disponíveis
   (excedentes ignorados).
4. `src_end < src_start` → `InvalidRange`.

### Consulta

```rust
impl CmapMapping {
    pub fn lookup(&self, glyph_code: u32) -> Option<Vec<char>>;
}
```

- **Precedência**: entradas consultadas na ordem de parsing; a **primeira correspondência
  vence** (duplicatas e sobreposições Char/Range incluídas) — determinístico e simples.
- **Limitação registada**: v1 usa `lookup` **linear** (O(n)) e devolve `Vec<char>` por valor
  (alocação por consulta). Aceitável para ToUnicode pequenos; optimizar (índice, `&[char]`)
  só com dados reais de desempenho. Documentar no doc-comment.

## Resultado esperado

- `01_core/src/content/cmap.rs`:
  - `CmapMapping`
  - `CmapEntry`
  - `CmapParseResult`
  - `CmapDiagnostic`
  - `parse_tounicode_cmap`
  - `CmapMapping::lookup`
  - testes inline (`#[cfg(test)]`) cobrindo todos os critérios de verificação

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
Então devolve `Some(['B'])` (array expandido em `Char`)

Dado um `bfrange` consecutivo `<0001> <0003> <00660069>` (destino multi-caractere)
Quando o parse corre
Então a entrada é descartada e `UnsupportedRangeDestination` é emitido

Dado um `bfrange` em array com menos destinos que o intervalo `<0001> <0003> [<0041>]`
Quando o parse corre
Então o par `(0x0001, 'A')` é preservado e `InvalidRange` é emitido

Dado `bfchar` com destino inválido `<0001> <004G>`
Quando o parse corre
Então a entrada é descartada e `InvalidHex` é emitido (não `InvalidUnicode`)

Dado `bfchar` com destino vazio `<0001> <>`
Quando o parse corre
Então a entrada é descartada e `InvalidHex` é emitido

Dado `bfchar` com comprimento ímpar `<001> <0041>`
Quando o parse corre
Então a entrada é descartada e `InvalidHex` é emitido

Dado `2 beginbfchar` seguido de apenas uma entrada e `endbfchar`
Quando o parse corre
Então a entrada lida é preservada e `InvalidRange` é emitido (contagem declarada ≠ lida)

Dado duplicata de código fonte: `<0001> <0041>` depois `<0001> <0042>`
Quando `lookup(0x0001)` é chamado
Então devolve `Some(['A'])` (primeira correspondência vence)

Dado sobreposição: `bfrange <0001> <0003> <0041>` seguido de `bfchar <0002> <0058>`
Quando `lookup(0x0002)` é chamado
Então devolve `Some(['B'])` (o `Range` apareceu primeiro)

Dado uma secção `beginbfchar` sem `endbfchar`
Quando o parse corre
Então as entradas lidas até ao fim são preservadas e `TruncatedMapping` é registado

Dado `endbfchar` sem `beginbfchar` anterior
Quando o parse corre
Então é ignorado e `UnknownOperator` é registado

Dado duas secções `beginbfchar`/`endbfchar` consecutivas com uma entrada cada
Quando o parse corre
Então ambas as entradas existem no mapeamento final

Dado bytes vazios
Quando o parse corre
Então mapeamento vazio + `EmptyInput`, sem pânico

Dado prosa típica de CMap real (`/CIDInit /ProcSet findresource begin`, `begincmap`, `def`)
Quando o parse corre
Então nenhum `UnknownOperator` é emitido por esses tokens

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| 2026-08-12 | Criação — parser puro sem lopdf; resultado com diagnósticos em vez de falha total | `cmap.rs` (novo) |
| 2026-08-12 | Revisão do dono: `Range.dst_start` de `Vec<char>` para `char` (incremento de sequência multi-caractere é indefinido) + `UnsupportedRangeDestination`; bfrange-array expandido em `Char` com regra de tamanho; regras hex explícitas (`InvalidHex`, inclui destino vazio e comprimento ímpar); contadores declarados; precedência primeira-correspondência; múltiplas secções; `end` sem `begin`; palavras-chave comuns sem diagnóstico; `codespacerange` reconhecido mas não armazenado; lookup linear registado como limitação; secção "Resultado esperado"; critérios de erro e fronteira | `cmap.rs` (novo) |
