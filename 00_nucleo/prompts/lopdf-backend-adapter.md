# Prompt: adaptador lopdf — `03_infra`

**Camada**: L3 — `03_infra`
**Arquivo gerado**: `03_infra/src/lopdf_adapter.rs` (novo) + testes de integração em `03_infra/tests/` com fixtures próprios em `03_infra/tests/fixtures/` (copiados de `_lab/lopdf_probe/fixtures/`, com origem e hash registados)
**Depende de**: `00_nucleo/prompts/content-stream-text-model.md` (`ContentOperation`, `XObjectInfo`), `00_nucleo/prompts/pdf-font-model.md` (`RawFontData`), `00_nucleo/prompts/pdf-error-policy.md` (`PdfError`), `00_nucleo/prompts/page-geometry-model.md` (`PageBoxModel`)
**ADR**: `00_nucleo/adr/0002-lopdf-backend-parsing.md` (regra de camada, escopo de leitura, criptografia)

## Contexto

`03_infra` é a única camada que toca ficheiros e a única que importa lopdf. O seu papel:
ler o PDF, descomprimir, **traduzir a sintaxe** decodificada pelo lopdf para tipos de
`01_core` — nunca interpretar semântica (matrizes, posições, CMap, diagnósticos de página).

## Restrições estruturais

- Direcção permitida: `03_infra → lopdf`, `03_infra → 01_core`. Proibida: `01_core → lopdf`.
- **Não vazar tipos do lopdf** na interface pública.
- Converter todos os números para `f64` antes de entregar (ADR 0003).
- `PdfError` é definido em `01_core` (`pdf-error-policy.md`); o adaptador **importa** esse
  tipo e apenas converte erros do lopdf para ele — não define taxonomia própria.
- O adaptador **não emite diagnósticos** de página nem de texto — produz apenas *hints*
  brutos; a decisão (`ImageOnlyPage`, `NoTextOperators`, etc.) pertence a `01_core`
  (`pdf-scan-like-diagnostic.md`).

## Instrução

O adaptador **deve**:

- abrir o PDF (`Document::load`); **v1 não aceita senha** — gate `is_encrypted()` →
  `PdfError::Encrypted` (ADR 0002; `load_with_password` registado para versão futura);
- ler trailer, root e catálogo; localizar páginas na árvore;
- **resolver herança** da árvore de páginas para `MediaBox`, `CropBox`, `Rotate`,
  `UserUnit` e `Resources`; montar `PageBoxModel` com valores **brutos** (a resolução é de
  `01_core`); `MediaBox` ausente mesmo por herança → `PdfError::Parse`;
- obter `/Contents` (stream único ou array) descomprimido; **múltiplos streams são
  processados na ordem do array e as operações concatenadas nessa ordem**; stream que não
  descomprime/decodifica → `PdfError::Parse`;
- decodificar operações via `lopdf::content::Content::decode` e converter para
  `ContentOperation` conforme a **tabela de conversão** abaixo;
- extrair `/Resources`/`/Font`: para cada fonte, montar `RawFontData` — incluindo, para
  Type0, o **descendente** `/DescendantFonts[0]` (onde vivem `/DW` e `/W`); `/Widths` +
  `/FirstChar`/`/LastChar` e `/W` expandidos a pares `(código, largura)`; `/Encoding`
  representado como `RawFontEncoding`; `ToUnicode` descomprimido, ou `None` se ausente **ou
  ilegível** (ilegível = ausente, não fatal — `01_core` produzirá `Unmapped`);
- extrair metadados dos XObjects da página: `XObjectInfo { name, subtype }`, com subtipo
  `Form`, `Image` ou `Other` (ausente/desconhecido → `Other`, nunca tratado como Form);
- converter números para `f64`.

O adaptador **não deve**:

- calcular geometria, interpretar CMap, interpretar semântica de operadores (matrizes,
  posições, avanços);
- derivar orientação de coordenadas nem normalizar posições (o espaço de usuário PDF é YUp
  por especificação; a convenção de saída é `normalize_to_top_left`, em `01_core` — ver
  `coordinate-normalization.md`);
- emitir diagnósticos (só hints);
- decidir política de relatório; implementar OCR; reparar/escrever/mesclar PDF.

### Tabela de conversão de operadores

| Operador PDF | `ContentOperation` |
|---|---|
| `BT` | `BeginText` |
| `ET` | `EndText` |
| `Tf` | `SetFont { name, size_pt }` |
| `Tm` | `SetTextMatrix { m }` |
| `Td` | `MoveText { tx, ty }` |
| `TD` | `MoveTextSetLeading { tx, ty }` |
| `T*` | `NextLine` |
| `TL` | `SetLeading { leading }` |
| `Tc` | `SetCharSpacing { tc }` |
| `Tw` | `SetWordSpacing { tw }` |
| `Tz` | `SetHorizontalScaling { tz_percent }` |
| `Ts` | `SetTextRise { ts }` |
| `Tr` | `SetTextRenderMode { mode }` |
| `Tj` | `ShowText { bytes }` (bytes crus, sem parênteses/hex/escapes) |
| `TJ` | `ShowTextAdjusted { items }` (`TjItem::Text`/`Adjustment`) |
| `q` | `SaveState` |
| `Q` | `RestoreState` |
| `cm` | `ConcatMatrix { m }` |
| `Do` | `InvokeXObject { name }` |

Regras fora da tabela:

- Operadores da **categoria texto** fora da lista (inclui `'` e `"`) →
  `UnsupportedTextOp { operator }`. **Nunca** mapear `Tc`/`Tw`/`Tz`/`Ts`/`Tr` como
  `UnsupportedTextOp` — eles estão na tabela; aplicá-los ou não é decisão de `01_core`.
  `Tr` foi acrescentado à tabela em 2026-08-12: o fixture `typst.pdf` mostra que o Typst
  emite `0 Tr`, pelo que a regra anterior produzia `UnsupportedTextOp` em todas as páginas
  Typst, para um operador semanticamente neutro.
- Operadores não-texto (desenho, cor, clipping, imagem inline `BI`/`EI`) →
  `ContentOperation::Other` (decisão do dono, 2026-08-12: **não são descartados**). O vector
  `operations` fica com a sequência completa do content stream, permitindo reconstruir e
  auditar a ordem real dos operadores; `01_core` ignora `Other` explicitamente. Esta regra
  substitui a anterior ("descartados"), que contradizia o critério de verificação de
  `content-stream-text-model.md`.
- `Other` **não** carrega o nome do operador na v1: se o diagnóstico vier a precisar de o
  distinguir, é revisão de prompt, não decisão de implementação.

### Interface

```rust
pub struct PageSourceHints {
    pub has_text_show_operators: bool,   // true sse existe ShowText ou ShowTextAdjusted
                                         // no content stream directo (BT/ET/Tf NÃO contam)
    pub has_do_operator: bool,
}

pub struct PageSource {
    pub box_model: PageBoxModel,
    pub operations: Vec<ContentOperation>,
    pub fonts: Vec<RawFontData>,
    pub xobjects: Vec<XObjectInfo>,
    pub hints: PageSourceHints,
}

pub fn load_page_source(path: &Path, page_index: usize) -> Result<PageSource, PdfError>;
```

`PdfError` vem de `01_core` (`pdf-error-policy.md`). Mapeamento de erros do lopdf conforme a
tabela de contrato desse prompt; `page_index` fora do intervalo →
`PdfError::PageNotFound { page_index }`.

## Resultado esperado

- `03_infra/src/lopdf_adapter.rs`:
  - `load_page_source`
  - conversão de página → `PageBoxModel` (com herança resolvida)
  - conversão de content stream → `Vec<ContentOperation>` (tabela acima, descarte de não-texto)
  - conversão de fontes → `Vec<RawFontData>` (incl. descendentes CID)
  - conversão de XObjects → `Vec<XObjectInfo>`
  - mapeamento de erros lopdf → `PdfError`
- `03_infra/tests/`:
  - fixtures copiados de `_lab/lopdf_probe/fixtures/` para `03_infra/tests/fixtures/`,
    com origem e hash registados (não importar nem referenciar código de `_lab`)
  - teste PDF Typst; teste xref stream/object streams; teste PDF criptografado; teste
    scan-like; teste CropBox/Rotate; teste página inexistente

## Critérios de verificação

Dado o fixture `typst.pdf` (Typst 0.15.1)
Quando `load_page_source(path, 0)` é chamado
Então devolve `PageSource` com `box_model.media_box` preenchido, `operations` não vazio
(contendo pelo menos `BeginText`, `SetFont` e `ShowText*`/`UnsupportedTextOp`), e `fonts`
contendo pelo menos uma fonte com `tounicode: Some(_)`

Dado o fixture com xref stream + object streams (recompactado via qpdf)
Quando `load_page_source` é chamado
Então comporta-se como o anterior (transparência de formato de xref)

Dado o fixture criptografado (AES-256, qpdf)
Quando `load_page_source` é chamado
Então devolve `Err(PdfError::Encrypted)` (v1 sem senha)

Dado `page_index` fora do intervalo do documento
Quando `load_page_source` é chamado
Então devolve `Err(PdfError::PageNotFound { page_index })`

Dado o fixture scan-like (página só com XObject de imagem)
Quando `load_page_source` é chamado
Então `operations` decodifica sem erro, `hints.has_text_show_operators == false`,
`hints.has_do_operator == true` e `xobjects` contém a imagem com `XObjectSubtype::Image`

Dado um content stream com `BT /F1 12 Tf 100 700 Td (A) Tj ET`
Quando `load_page_source` é chamado
Então `operations` contém `BeginText`, `SetFont`, `MoveText`, `ShowText`, `EndText` nesta
ordem, e `hints.has_text_show_operators == true`

Dado um content stream com `Tc 0.5 Tw 1 Tz 80 Ts 2 Tr 0`
Quando `load_page_source` é chamado
Então produz `SetCharSpacing`, `SetWordSpacing`, `SetHorizontalScaling`, `SetTextRise` e
`SetTextRenderMode { mode: 0 }` — **nunca** `UnsupportedTextOp`

Dado o fixture `typst.pdf`, que contém `0 Tr`
Quando `load_page_source` é chamado
Então `operations` contém `SetTextRenderMode { mode: 0 }` e **nenhum** `UnsupportedTextOp`
para `Tr`

Dado um operador de texto fora da lista (`'`)
Quando `load_page_source` é chamado
Então a operação correspondente é `UnsupportedTextOp { operator: "'" }`

Dado operadores de desenho/cor (`re`, `S`, `rg`, `W`)
Quando `load_page_source` é chamado
Então aparecem em `operations` como `ContentOperation::Other`, na posição original da
sequência (não são descartados)

Dado um content stream `q 1 0 0 -1 0 792 cm BT /F1 12 Tf (A) Tj ET Q` misturado com
operadores de cor entre `q` e `BT`
Quando `load_page_source` é chamado
Então a ordem relativa de `SaveState`, `ConcatMatrix`, `Other`, `BeginText`… é a do content
stream original (a sequência é reconstruível)

Dado um PDF com fonte Type0 cujo `/W` e `/DW` estão no descendente `/DescendantFonts[0]`
Quando `load_page_source` é chamado
Então `RawFontData.widths` e `default_width` vêm do descendente (não ficam vazios)

Dado um PDF com fonte sem `ToUnicode` (ou com stream ilegível)
Quando `load_page_source` é chamado
Então a fonte aparece em `fonts` com `tounicode == None`, sem erro

Dado um PDF cuja página herda `MediaBox` e `Rotate` da árvore de páginas
Quando `load_page_source` é chamado
Então `box_model.media_box` e `box_model.rotate` reflectem os valores herdados

Dado um PDF com página sem `CropBox` e com `Rotate: 90`
Quando `load_page_source` é chamado
Então `box_model.crop_box == None` e `box_model.rotate == Some(90)` (bruto, sem normalizar)

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| 2026-08-12 | Criação — sem vazamento de tipos lopdf, conversão para `f64`, gate de criptografia, dados brutos | `lopdf_adapter.rs` (novo) |
| 2026-08-12 | Adaptador passa a decodificar operações (`ContentOperation`); `RawFontData`; metadados de XObjects; `PageSource` revista | — |
| 2026-08-12 | Decisão do dono, fecho de contradições de spec: (1) `Tr` acrescentado à tabela de conversão como `SetTextRenderMode { mode }` — sem isso caía na regra de "operador de texto fora da lista" e produzia `UnsupportedTextOp` em todas as páginas Typst, que emitem `0 Tr` (evidência: fixture `typst.pdf`); (2) operadores não-texto passam a `ContentOperation::Other` em vez de descartados, resolvendo a contradição com o critério de verificação de `content-stream-text-model.md` — `operations` fica com a sequência completa e auditável, e `01_core` ignora `Other`. Critérios correspondentes reescritos | `lopdf_adapter.rs` (novo) |
| 2026-08-12 | Revisão do dono (análise de prontidão): `PdfError` importado de `01_core` (`pdf-error-policy.md`); v1 sem senha; `origin` removido de `PageSource` (origem é derivada por `01_core`); tabela completa de conversão de operadores (incl. `Tc`/`Tw`/`Tz`/`Ts` mapeados, nunca `UnsupportedTextOp`); operadores não-texto descartados; herança de atributos de página; `DescendantFonts` para Type0; `RawFontEncoding`; `XObjectSubtype::Other`; hints em vez de diagnósticos (`PageSourceHints`); `has_text_operators_hint` definido só por `ShowText*`; `ToUnicode` ilegível = ausente; múltiplos `/Contents` concatenados em ordem; `PdfError::Parse` para stream/MediaBox inválidos; `PageNotFound`; fixtures copiados para `03_infra/tests/fixtures/`; secção "Resultado esperado" | `lopdf_adapter.rs` (novo) |
