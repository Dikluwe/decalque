# Prompt: diagnósticos de página — `PageDiagnostic`

**Camada**: L1 — `01_core`
**Arquivo gerado**: `01_core/src/entities/page_diagnostic.rs` (novo) + testes no mesmo ficheiro
**Depende de**: `00_nucleo/prompts/lopdf-backend-adapter.md` (`PageSourceHints`, `XObjectInfo`), `00_nucleo/prompts/content-stream-text-model.md` (`ContentOperation`)
**ADR**: `00_nucleo/adr/0002-lopdf-backend-parsing.md` (secção "Comportamento para páginas sem texto")

## Contexto

Página sem texto não é erro: no Caso 2, o lado do scan é *sempre* imagem (ADR 0001), e texto
ausente deve aparecer como **ausência de texto**, nunca como falha inesperada (ADR 0002,
consequências de produto). A divisão de responsabilidades já está fixada: `03_infra` produz
apenas *hints* brutos (`PageSourceHints`), `01_core` decide o diagnóstico. Este prompt define
a taxonomia de diagnósticos **de página** — separada de `TextInterpreterDiagnostic`
(diagnósticos de texto, `content-stream-text-model.md`), conforme decisão do dono.

OCR está fora de escopo (ADR 0002).

## Restrições estruturais

- L1: zero I/O, decisão pura sobre hints + operações já extraídas.
- Não importa lopdf nem tipos de `03_infra` (os hints chegam como valores).

## Instrução

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageDiagnostic {
    ImageOnlyPage,     // sem operadores de mostrar texto, com XObject de imagem invocado
    NoTextOperators,   // sem operadores de mostrar texto, sem XObject de imagem invocado
}

pub fn diagnose_page(
    has_text_show_operators: bool,
    has_do_operator: bool,
    xobjects: &[XObjectInfo],
) -> Vec<PageDiagnostic>;
```

Regras (decisão do dono):

1. `has_text_show_operators == true` → **nenhum** diagnóstico (página tem texto; mesmo que
   também tenha imagem, isso é normal).
2. `has_text_show_operators == false` e existe `InvokeXObject` cujo `XObjectInfo` tem
   `XObjectSubtype::Image` → `ImageOnlyPage`.
3. `has_text_show_operators == false` e **nenhum** XObject de imagem invocado →
   `NoTextOperators`.
4. Os hints vêm de `03_infra` (`PageSourceHints.has_text_show_operators`,
   `has_do_operator`); a verificação do subtipo usa `xobjects` + a presença de
   `InvokeXObject` nas operações — `has_do_operator` é atalho, mas a regra 2 exige cruzar o
   `Do` com o subtipo (um `Do` para Form não é `ImageOnlyPage`).
5. Cada diagnóstico é emitido **uma vez por página**, no máximo.

Nota: a emissão efectiva destes diagnósticos no resultado final faz parte de
`document-geometry.md` (que agrega `diagnostics: Vec<PageDiagnostic>` por página); este
prompt define apenas a taxonomia e a regra de decisão.

Nota sobre scans pesquisáveis (2026-08-12): um scan com camada de OCR **tem** operadores de
mostrar texto — invisíveis (`Tr 3`), mas presentes. Pela regra 1, essa página **não** produz
`ImageOnlyPage`, e isso está correcto: ela tem texto comparável, que é o que estes
diagnósticos descrevem. O sinal de que o texto é invisível vem do intérprete
(`TextInterpreterDiagnostic::InvisibleTextPresent`, `content-stream-text-model.md`), não
daqui — coerente com a divisão entre diagnóstico de página e diagnóstico de texto. Um scan
**sem** camada de OCR continua a produzir `ImageOnlyPage`, como o Caso 2 pressupõe.

## Resultado esperado

- `01_core/src/entities/page_diagnostic.rs`: `PageDiagnostic`, `diagnose_page`, testes
  inline cobrindo os critérios.

## Critérios de verificação

Dado `has_text_show_operators == true`
Quando `diagnose_page` é chamado
Então devolve lista vazia (texto presente nunca gera diagnóstico, mesmo com imagem na página)

Dado `has_text_show_operators == false` e um `Do` invocando XObject com `XObjectSubtype::Image`
Quando `diagnose_page` é chamado
Então devolve `[ImageOnlyPage]` (o caso do lado scan, ADR 0001)

Dado `has_text_show_operators == false` e nenhum XObject de imagem invocado
Quando `diagnose_page` é chamado
Então devolve `[NoTextOperators]` (página em branco ou só com desenho vectorial)

Dado `has_text_show_operators == false` e um `Do` invocando XObject com `XObjectSubtype::Form`
Quando `diagnose_page` é chamado
Então devolve `[NoTextOperators]` — Form não é imagem (texto dentro de Form é limitação da
v1, diagnosticada pelo intérprete, não aqui)

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| 2026-08-12 | Criação — último prompt da lista do ADR 0002; taxonomia de página separada de `TextInterpreterDiagnostic` (decisão do dono); `03_infra` dá hints, `01_core` decide | `page_diagnostic.rs` (novo) |
| 2026-08-12 | Nota sobre scans pesquisáveis: consequência da decisão de `Tr` (`content-stream-text-model.md`) — camada de OCR conta como texto presente, logo sem `ImageOnlyPage`; o sinal de invisibilidade é do intérprete, não deste prompt | — |
