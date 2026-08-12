# Prompt: modelo de texto do content stream — `ContentOperation`, `GlyphInstance`, intérprete

**Camada**: L1 — `01_core`
**Arquivo gerado**: `01_core/src/entities/glyph_instance.rs` (revisão do existente) + `01_core/src/content/text_interpreter.rs` (novo) + testes nos mesmos ficheiros
**Substitui**: `00_nucleo/prompts/_deprecated/glyph-instance.md` (absorvido)
**Depende de**: `00_nucleo/prompts/pdf-font-model.md` (`FontModel`), `00_nucleo/prompts/cmap-tounicode-parser.md` (`CmapMapping`), `00_nucleo/prompts/coordinate-normalization.md` (`normalize_to_top_left`), `00_nucleo/prompts/page-geometry-model.md` (`PageGeometry`)
**ADR**: `00_nucleo/adr/0002-lopdf-backend-parsing.md` (secções "Escopo de content stream", "Comportamento para ToUnicode ausente ou incompleto", "Comportamento para Form XObjects"), `00_nucleo/adr/0003-convencoes-transversais.md` (`f64`)

## Contexto

Decisões do dono (2026-08-12):

- **`GlyphInstance` é o átomo do núcleo.** Não existe `TextSpan` na fronteira
  `03_infra`→`01_core`. Agrupamento textual, se um dia necessário, é `TextRun` derivado —
  **não implementar agora**.
- **O intérprete recebe operações já decodificadas, não bytes.** `03_infra` usa
  `lopdf::content::Content::decode` (validado em `_lab/lopdf_probe/`) e converte para
  `ContentOperation` (tipo de `01_core` definido aqui). Parsing de sintaxe PDF (strings
  literais/hex, escapes, arrays) é responsabilidade do lopdf, em `03_infra` — `01_core`
  interpreta semântica, nunca sintaxe.
- `Tc`/`Tw`/`Tz`/`Ts` são **aplicados na v1** (geometria confiável é a razão do Decalque).

Tipo numérico: `f64`, conforme ADR 0003. Códigos de glifo são inteiros.

## Escopo de impacto

Este prompt revisa/cria **apenas**:

- `01_core/src/entities/glyph_instance.rs`
- `01_core/src/content/text_interpreter.rs`

Este prompt **não** actualiza: motor de comparação, `normalize_to_top_left`,
`DocumentGeometry`, testes existentes desses componentes — serão revistos por prompts
próprios (a revisão de `GlyphInstance` pode quebrá-los temporariamente).

## Restrições estruturais

- L1: zero I/O, **não importa lopdf** nem tipos de `03_infra`.
- `ShowText`/`ShowTextAdjusted` carregam os **bytes crus** da string mostrada — já sem
  parênteses, hex ou escapes (isso é da decodificação lopdf), mas **antes** da decodificação
  em códigos de glifo (isso é do `GlyphCodeDecoder`, ver `pdf-font-model.md`).

## Instrução

### Operações de content stream (contrato com `03_infra`)

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ContentOperation {
    BeginText,
    EndText,
    SetFont { name: String, size_pt: f64 },               // Tf
    SetTextMatrix { m: [f64; 6] },                        // Tm
    MoveText { tx: f64, ty: f64 },                        // Td
    MoveTextSetLeading { tx: f64, ty: f64 },              // TD
    NextLine,                                             // T*
    SetLeading { leading: f64 },                          // TL
    SetCharSpacing { tc: f64 },                           // Tc
    SetWordSpacing { tw: f64 },                           // Tw
    SetHorizontalScaling { tz_percent: f64 },             // Tz
    SetTextRise { ts: f64 },                              // Ts
    ShowText { bytes: Vec<u8> },                          // Tj
    ShowTextAdjusted { items: Vec<TjItem> },              // TJ
    SaveState,                                            // q
    RestoreState,                                         // Q
    ConcatMatrix { m: [f64; 6] },                         // cm
    InvokeXObject { name: String },                       // Do
    UnsupportedTextOp { operator: String },               // operador de texto fora da lista (inclui ' e ")
    Other,                                                // operadores não-texto (cor, traço, clip...) — ignorados
}

#[derive(Debug, Clone, PartialEq)]
pub enum TjItem {
    Text { bytes: Vec<u8> },
    Adjustment { thousandths: f64 },   // ajuste TJ em milésimos de unidade de espaço de texto
}
```

`03_infra` classifica cada operação do lopdf neste enum; operadores fora da lista que sejam
da categoria texto (começam por `T`, ou `'`/`"`) viram `UnsupportedTextOp`; os restantes viram
`Other` (ignorados silenciosamente — decisão do dono).

### Modelo de glifo (revisão de `GlyphInstance`)

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphInstance {
    pub glyph_code: u32,
    pub codepoints: Option<Vec<char>>,   // via ToUnicode; None se não mapeado (ADR 0001: ligaduras)
    pub position: (f64, f64),            // normalizado por normalize_to_top_left
    pub advance: f64,                    // avanço horizontal efectivo, em pontos (fórmula abaixo)
    pub font_ref: String,
    pub font_size_pt: f64,
    pub mapping_status: TextMappingStatus,
}

pub enum TextMappingStatus { Mapped, Unmapped }   // PartiallyMapped removido — sem caso real (decisão do dono)
```

Regras de mapeamento (ADR 0002): `ToUnicode` existente e cobrindo o código → `Mapped` com
`codepoints`; inexistente, sem cobertura, ou **produzindo sequência Unicode inválida** →
`Unmapped` e `codepoints: None`, `glyph_code` preservado — **nunca inventar texto**. Glifos
`Unmapped` não são descartados (emparelhamento posicional, ver `engine/compare.md`).

### Modelo de entrada/saída do intérprete

```rust
pub struct XObjectInfo {
    pub name: String,
    pub subtype: XObjectSubtype,
}

pub enum XObjectSubtype { Form, Image, Other }   // Other: subtipo ausente ou desconhecido — nunca tratado como Form

pub struct TextInterpretationInput {
    pub page: PageGeometry,                  // necessário a normalize_to_top_left
    pub operations: Vec<ContentOperation>,
    pub fonts: Vec<FontModel>,               // pdf-font-model.md
    pub xobjects: Vec<XObjectInfo>,
}

pub struct TextInterpretationOutput {
    pub glyphs: Vec<GlyphInstance>,
    pub diagnostics: Vec<TextInterpreterDiagnostic>,
}

pub enum TextInterpreterDiagnostic {
    UnmappedGlyphs,             // há glifos sem mapeamento ToUnicode
    TextStateNotFullyApplied,   // Tc/Tw/Tz/Ts presentes e NÃO aplicados (salvaguarda; v1 aplica)
    FormXObjectNotTraversed,    // Do com XObjectSubtype::Form — não percorrido na v1
    MissingFont,                // Tf referencia fonte ausente em fonts
    UnsupportedTextOperator,    // UnsupportedTextOp encontrado
    TextOutsideTextObject,      // operador de texto fora de BT/ET
    TrailingGlyphCodeBytes,     // decode de string descartou bytes (ver pdf-font-model.md)
}

pub fn interpret_text(input: &TextInterpretationInput) -> TextInterpretationOutput;
```

Diagnósticos de página (`ImageOnlyPage`, `NoTextOperators`) **não** pertencem a este enum —
ficam para `pdf-scan-like-diagnostic.md` (decisão do dono: separar diagnóstico de página de
diagnóstico de texto).

### Máquina de estado

Estado inicial: CTM identidade; matriz de texto e de linha identidade; sem fonte;
`font_size = 0`; `Tc = 0`; `Tw = 0`; `Tz = 100`; `TL = 0`; `Ts = 0`.

Semântica dos operadores:

- `q`/`Q`: pilha de estado gráfico (guarda/restaura CTM). `cm`: `CTM = m × CTM`.
- `BT`: reinicia matriz de texto e de linha para identidade. `ET`: encerra o objecto de texto.
- `Tm`: define matriz de texto **e** matriz de linha. `Td`: move a linha
  (`Tlm = T(tx,ty) × Tlm; Tm = Tlm`). `TD`: `Td` + `TL = -ty`. `T*`: `Td(0, -TL)`.
- `Tf`: selecciona fonte e tamanho. Fonte ausente em `fonts` → **não emitir glifos dessa
  fonte** + `MissingFont` (sem larguras, o avanço seria desconhecido e as posições seguintes
  silenciosamente erradas — decisão do dono).
- Operadores de texto fora de `BT`/`ET` → ignorados + `TextOutsideTextObject`.
- Convenção de matrizes (documentar no doc-comment): vectores **coluna**; concatenação à
  esquerda (`nova = operando × corrente`); a posição do glifo é `CTM × Tm` aplicada a `(0,0)`,
  e só depois `normalize_to_top_left(point, page)`. Validar a ordem de multiplicação
  contra PDF 32000-1 na implementação.

### Fórmula de avanço (escrita horizontal; PDF 32000-1, operadores de text state/showing)

```text
w_pt   = width_of(glyph_code) / 1000 × font_size_pt        // largura do glifo em pontos
tw_eff = Tw, se o código for espaço (0x20 em SingleByte); senão 0
advance = (w_pt + Tc + tw_eff) × (Tz / 100)
```

Ajuste de `TJ` (deslocamento antes do próximo glifo, sinal conforme PDF 32000-1 — ajuste
positivo aproxima os glifos):

```text
deslocamento = −(thousandths / 1000) × font_size_pt × (Tz / 100)
```

`Ts` desloca a coordenada vertical do glifo (`y += Ts`) sem afectar o avanço. A ordem exacta
das parcelas e os sinais devem ser validados contra a especificação na implementação e
registados em doc-comment.

`Tc`/`Tw`/`Tz`/`Ts` **são aplicados na v1**; `TextStateNotFullyApplied` só é emitido se algum
deles aparecer sem ser aplicado (salvaguarda contra regressão, não comportamento esperado).

### Limitação explícita da v1 (Form XObjects)

Somente content streams directos da página. `InvokeXObject` com `XObjectSubtype::Form` →
`FormXObjectNotTraversed`, sem percorrer. Com `XObjectSubtype::Image` → sem diagnóstico (é o
caso scan; diagnóstico de página é outro prompt). Premissa a validar com PDFs reais: Typst
não usa Form XObjects para texto de corpo (ADR 0002, "Validações pendentes").

## Resultado esperado

- `glyph_instance.rs`: `GlyphInstance` (campos acima), `TextMappingStatus { Mapped, Unmapped }`,
  testes inline.
- `text_interpreter.rs`: `ContentOperation`, `TjItem`, `XObjectInfo`, `XObjectSubtype`,
  `TextInterpretationInput`, `TextInterpretationOutput`, `TextInterpreterDiagnostic`,
  `interpret_text`, testes inline cobrindo todos os critérios.

## Critérios de verificação

Dado `CTM` identidade, `PageGeometry` 612×792 sem rotação, e operações
`BT /f0 12 Tf 1 0 0 1 100 700 Tm (A) Tj ET`
com `FontModel` mapeando o código de "A", `width_of = 500`
Quando `interpret_text` corre
Então emite um glifo com `position` normalizada `(100.0, 792.0 − 700.0) = (100.0, 92.0)`
(via `normalize_to_top_left`),
`advance == (500/1000 × 12 + 0 + 0) × 1 == 6.0`, `mapping_status == Mapped`, sem diagnósticos

Dado `TJ [(A) −120 (B)]` com `font_size 12`, `Tz = 100`, larguras de 500
Quando `interpret_text` corre
Então o segundo glifo tem posição x deslocada de `6.0 + (120/1000 × 12) == 7.44` relativamente
à origem do primeiro (avanço do primeiro + deslocamento do ajuste)

Dado `Tw = 2.0` e uma string contendo o código de espaço entre duas palavras
Quando `interpret_text` corre
Então o `advance` do glifo de espaço inclui `+ 2.0`; os glifos não-espaço não incluem `Tw`

Dado `Tz = 50` e os mesmos glifos
Quando `interpret_text` corre
Então todos os avanços ficam multiplicados por `0.5`

Dado `InvokeXObject` com `XObjectSubtype::Form`
Quando `interpret_text` corre
Então nenhum glifo do XObject é emitido e `FormXObjectNotTraversed` é registado

Dado `InvokeXObject` com `XObjectSubtype::Image`
Quando `interpret_text` corre
Então **não** há diagnóstico `FormXObjectNotTraversed`

Dado `Tf` referenciando fonte inexistente, seguido de `Tj`
Quando `interpret_text` corre
Então nenhum glifo é emitido e `MissingFont` é registado

Dado um código de glifo sem entrada no `ToUnicode`
Quando `interpret_text` corre
Então o glifo é emitido com `mapping_status == Unmapped`, `codepoints == None`, e o
diagnóstico `UnmappedGlyphs` é registado

Dado `UnsupportedTextOp { operator: "'" }`
Quando `interpret_text` corre
Então `UnsupportedTextOperator` é registado e a execução continua

Dado `Tj` fora de `BT`/`ET`
Quando `interpret_text` corre
Então o glifo não é emitido e `TextOutsideTextObject` é registado

Dado operadores não-texto (`re`, `S`, `rg`, `W`)
Quando `interpret_text` corre
Então são ignorados silenciosamente (classificados `Other` por `03_infra`)

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| 2026-08-12 | Criação — `GlyphInstance` átomo, `TextSpan` removido, `TextRun` adiado; absorve `entities/glyph-instance.md` | `glyph_instance.rs` (revisão), `text_interpreter.rs` (novo) |
| 2026-08-12 | Precisão de `Tc`/`Tw`/`Tz`/`Ts`; Unicode inválido → `Unmapped`; `f64` confirmado (ADR 0003) | — |
| 2026-08-12 | Revisão do dono (análise de prontidão): intérprete recebe **operações decodificadas** (`ContentOperation`), não bytes; modelo de entrada/saída explícito; modelo de fonte extraído para `pdf-font-model.md`; fórmula de avanço e ajuste TJ especificadas; `Tc`/`Tw`/`Tz`/`Ts` aplicados na v1; `Do` com `XObjectInfo` (Form→diagnóstico, imagem→não); diagnósticos divididos (`TextInterpreterDiagnostic` vs. diagnósticos de página adiados); `PartiallyMapped` removido; fonte ausente → sem glifos + `MissingFont`; estado inicial e semântica de operadores definidos; convenção de matrizes explícita; escopo de impacto declarado; critérios com números literais | `glyph_instance.rs` (revisão), `text_interpreter.rs` (novo) |
| 2026-08-12 | `CartesianOrigin` removido do modelo de entrada — a normalização é função pura `normalize_to_top_left` (`coordinate-normalization.md`); o espaço de usuário PDF é YUp por especificação, a inversão via `cm` é tratada pela CTM | — |
