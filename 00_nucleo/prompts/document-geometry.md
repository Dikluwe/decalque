# Prompt: `DocumentGeometry` — contrato do motor de comparação por glifo

**Camada**: L1 — `01_core`
**Arquivo gerado**: `01_core/src/entities/document_geometry.rs` (novo) + testes no mesmo ficheiro
**Substitui**: a definição de `DocumentGeometry` que existia em `_deprecated/glyph-instance.md`
**Depende de**: `00_nucleo/prompts/page-geometry-model.md` (`PageGeometry`), `00_nucleo/prompts/content-stream-text-model.md` (`GlyphInstance`), `00_nucleo/prompts/pdf-scan-like-diagnostic.md` (`PageDiagnostic`)
**ADR**: `00_nucleo/adr/0001-escopo-dois-casos-de-uso.md` (a fronteira que isola o backend), `00_nucleo/adr/0003-convencoes-transversais.md` (`f64`), `00_nucleo/adr/0004-fronteira-observacao-scan.md` (OCR usa contrato próprio)

## Contexto

`DocumentGeometry` é o contrato de uma página com glifos estruturais e a entrada do motor de
comparação por glifo. `03_infra` o constrói para PDFs digitais e `engine/compare.md` o consome.
Ficou órfão quando `glyph-instance.md` foi absorvido por `content-stream-text-model.md` (que
define só o átomo) — este prompt fecha o buraco (decisão do dono: prompt próprio, não definido
dentro de `compare.md`, porque `02_shell` também o consome para apresentação).

O Caso 2 não materializa OCR neste tipo. Região, linha e palavra entram como
`ScanObservation` (`scan-observation-model.md`) e são comparadas ao `DocumentGeometry` do
candidato por `scan-observation-compare.md`. Somente uma fonte que já possua glifos estruturais
reais pode construir `DocumentGeometry`; granularidade OCR não é promovida implicitamente.

## Restrições estruturais

- L1: zero I/O. Construído pelo pipeline estrutural (via `interpret_text` +
  `resolve_page_geometry` + `diagnose_page`) — este tipo só representa o resultado.
- Não depende de `ScanObservation`, proveniência OCR, JSON ou tipos de `03_infra`.
- Um `DocumentGeometry` por **página** (o pipeline actual é de página única; multi-página é
  `Vec<DocumentGeometry>` no chamador, não dentro desta struct — decisão a registar no
  doc-comment, coerente com o caso motivador typst de página única).

## Instrução

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentGeometry {
    pub page: PageGeometry,
    pub glyphs: Vec<GlyphInstance>,
    pub diagnostics: Vec<PageDiagnostic>,
}
```

Regras:

1. `glyphs` em **ordem de emissão do content stream** — a ordenação por ordem de leitura é
   responsabilidade do motor (`compare.md`), não deste tipo. Documentar no doc-comment.
2. Documento sem texto (`glyphs` vazio) é caso válido, não excepcional — tipicamente
   acompanhado de `PageDiagnostic::ImageOnlyPage` ou `NoTextOperators` (herdado da spec
   original: `Vec` vazio não é erro).
3. `diagnostics` agrega os diagnósticos de página (`diagnose_page`); os diagnósticos de
   texto (`TextInterpreterDiagnostic`) **não** entram aqui na v1 — pertencem ao resultado do
   intérprete; se o relatório precisar deles, é decisão de `02_shell` combinar as duas saídas.
   Registar esta divisão no doc-comment.
4. Derives mínimos: `Debug`, `Clone`, `PartialEq`.

## Resultado esperado

- `01_core/src/entities/document_geometry.rs`: `DocumentGeometry`, testes inline cobrindo
  os critérios.

## Critérios de verificação

Dado um `Vec<GlyphInstance>` vazio
Quando um `DocumentGeometry` é construído com ele
Então `glyphs` fica vazio, sem erro (documento sem texto é caso válido)

Dado um `DocumentGeometry` com glifos fora de ordem de leitura (ordem de emissão)
Quando inspeccionado
Então `glyphs` preserva a ordem de emissão (não reordena — ordenação é do motor)

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| 2026-08-12 | Criação — `DocumentGeometry` estava órfão após absorção de `glyph-instance.md`; decisão do dono: prompt próprio (consumido pelo motor e por `02_shell`); um por página; ordem de emissão preservada; diagnósticos de página agregados, de texto fora na v1 | `document_geometry.rs` (novo) |
| 2026-09-18 | ADR 0004: removida a promessa de que OCR por região/linha/palavra produz `DocumentGeometry`; o Caso 2 passa a consumir `ScanObservation` em comparador próprio. | futura revisão de documentação do tipo |
