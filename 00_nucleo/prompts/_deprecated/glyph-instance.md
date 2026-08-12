# Prompt: `GlyphInstance` e `DocumentGeometry`

**Camada**: L1 — `01_core`
**Arquivo gerado**: `01_core/src/entities/glyph_instance.rs` + testes no mesmo ficheiro

## Contexto

A unidade atómica de comparação: um traço/glifo desenhado numa posição. `DocumentGeometry` é a
colecção de todos os `GlyphInstance` de um PDF, já em coordenadas normalizadas (depois de
`CartesianOrigin::normalize`), mais o contexto (`PageGeometry`) que os produziu.

## Restrições estruturais

- L1: zero I/O. Construída por `03_infra` a partir da leitura real do content stream
  (`Tj`/`TJ`/`cm`/`Tf`, extracção de `ToUnicode`) — esta struct só representa o resultado.

## Instrução

```rust
struct GlyphInstance {
    position: (f64, f64),            // já normalizado (CartesianOrigin::normalize aplicado)
    codepoints: Option<Vec<char>>,   // via ToUnicode; None se não mapeado
    font_size_pt: f64,
    font_ref: String,                // identificador da fonte no documento (nome do recurso, ou hash — decidir na implementação)
}

struct DocumentGeometry {
    page: PageGeometry,
    glyphs: Vec<GlyphInstance>,
}
```

Nota de proveniência (lição do projecto irmão, P948): glifos sem codepoints mapeados (peças de
assembly sem entrada em `ToUnicode`, por exemplo) não devem ser descartados — ainda têm posição e
participam na comparação, só não têm âncora textual para o emparelhamento por conteúdo
(`engine/compare.md` decide como tratar este caso).

**Por que sequência e não `char` único** (ADR 0001): uma ligadura é um glifo que mapeia para
vários codepoints — `ToUnicode` normalmente mapeia o glifo "fi" para a sequência `['f','i']`.
Com `char` único essa informação é perdida e a comparação scan→digital reportaria falsa
divergência (original com ligadura vs. gerado com "f"+"i" separados). A sequência é a âncora
de emparelhamento que permite a normalização de ligaduras no motor (`engine/compare.md`).

**Ligação ao Caso 2 (scan→digital)**: esta struct é também o contrato de saída da extracção
externa (OCR/análise da imagem do scan) — o lado do scan entra no pipeline como um
`DocumentGeometry` produzido fora do núcleo, na mesma forma. Detalhes em
`00_nucleo/prompts/case2-scan-to-digital.md` (planeamento, não gera código agora).

## Critérios de verificação

Dado um `Vec<GlyphInstance>` vazio
Quando um `DocumentGeometry` é construído com ele
Então o campo `glyphs` fica vazio, sem erro (documento sem conteúdo matemático/texto é um caso
válido, não excepcional)

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| 2026-08-11 | Criação inicial + primeira geração de `01_core` | `glyph_instance.rs` |
| 2026-08-11 | ADR 0001: `codepoint: Option<char>` → `codepoints: Option<Vec<char>>` (ligaduras); nota de ligação ao Caso 2 | — |
