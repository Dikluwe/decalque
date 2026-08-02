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
    position: (f64, f64),      // já normalizado (CartesianOrigin::normalize aplicado)
    codepoint: Option<char>,   // via ToUnicode; None se não mapeado
    font_size_pt: f64,
    font_ref: String,          // identificador da fonte no documento (nome do recurso, ou hash — decidir na implementação)
}

struct DocumentGeometry {
    page: PageGeometry,
    glyphs: Vec<GlyphInstance>,
}
```

Nota de proveniência (lição do projecto irmão, P948): glifos sem `codepoint` mapeado (peças de
assembly sem entrada em `ToUnicode`, por exemplo) não devem ser descartados — ainda têm posição e
participam na comparação, só não têm âncora textual para o emparelhamento por conteúdo
(`engine/compare.md` decide como tratar este caso).

## Critérios de verificação

Dado um `Vec<GlyphInstance>` vazio
Quando um `DocumentGeometry` é construído com ele
Então o campo `glyphs` fica vazio, sem erro (documento sem conteúdo matemático/texto é um caso
válido, não excepcional)

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| (preencher na execução) | Criação inicial | `glyph_instance.rs` |
