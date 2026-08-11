# Prompt: modelo de texto do content stream — `GlyphInstance`, operadores, diagnósticos

**Camada**: L1 — `01_core`
**Arquivo gerado**: `01_core/src/entities/glyph_instance.rs` (revisão do existente) + `01_core/src/content/text_interpreter.rs` (novo) + testes nos mesmos ficheiros
**Substitui**: `00_nucleo/prompts/entities/glyph-instance.md` (absorvido; o motor de comparação continua a consumir `GlyphInstance`/`DocumentGeometry`)
**ADR**: `00_nucleo/adr/0002-lopdf-backend-parsing.md` (secções "Escopo de content stream", "Comportamento para ToUnicode ausente ou incompleto", "Comportamento para Form XObjects")

## Contexto

Decisão do dono (2026-08-12): **`GlyphInstance` é o átomo do núcleo**. Não existe `TextSpan`
na fronteira entre `03_infra` e `01_core` — `03_infra` entrega dados brutos (streams
descomprimidos, dicionários de fonte, streams `ToUnicode`), e `01_core` interpreta os
operadores do content stream e produz glifos posicionados. Agrupamento textual, se um dia for
necessário para relatório, é tipo derivado (`TextRun`), nunca entrada de `03_infra` — **não
implementar `TextRun` agora** (o relatório actual não precisa de texto agrupado).

## Restrições estruturais

- L1: zero I/O. O intérprete recebe bytes do content stream (já descomprimidos por `03_infra`)
  e as tabelas de mapeamento de fonte (CMap parseado — ver `cmap-tounicode-parser.md`), e
  devolve glifos + diagnósticos. Não depende de tipos do lopdf.
- Unidades: pontos PDF. Precisão: seguir o tipo já usado no núcleo — ver nota no Histórico.

## Instrução

### Modelo de glifo (revisão de `GlyphInstance`)

```rust
pub struct GlyphInstance {
    pub glyph_code: u32,                    // código do glifo no content stream (antes do CMap)
    pub codepoints: Option<Vec<char>>,      // via ToUnicode; None se não mapeado (ver ADR 0001: ligaduras)
    pub position: (f64, f64),               // já normalizado (CartesianOrigin::normalize aplicado)
    pub advance: f64,                       // avanço horizontal efectivo aplicado após o glifo
    pub font_ref: String,                   // identificador da fonte no documento
    pub font_size_pt: f64,
    pub mapping_status: TextMappingStatus,
}

pub enum TextMappingStatus { Mapped, Unmapped, PartiallyMapped }
```

Regras de mapeamento (ADR 0002): `ToUnicode` existente e cobrindo o código → `Mapped` com
`codepoints`; inexistente ou sem cobertura → `Unmapped` e `codepoints: None` — **nunca inventar
texto**; a ausência de mapeamento não falha a leitura, é diagnóstico. Se o mapeamento produzir
uma **sequência Unicode inválida**, o comportamento é o mesmo de ausência: `codepoints: None`,
`glyph_code` preservado, `mapping_status == Unmapped`. Glifos `Unmapped` não são
descartados: têm posição e participam na comparação (emparelhamento posicional, ver
`engine/compare.md`).

### Intérprete de content stream (novo, `content/text_interpreter.rs`)

Máquina de estado que percorre as operações decodificadas e mantém: CTM (com `q`/`Q`/`cm`),
matriz de texto e de linha (`BT`/`ET`/`Tm`/`Td`/`TD`/`T*`/`TL`), fonte corrente (`Tf`), e
emite um `GlyphInstance` por código de glifo em `Tj`/`TJ` (nos `TJ`, os ajustes numéricos
afectam o avanço — aplicar). A posição final de cada glifo é o produto CTM × matriz de texto,
depois normalizado por `CartesianOrigin::normalize`.

Operadores processados na v1: `BT`, `ET`, `Tf`, `Tm`, `Td`, `TD`, `T*`, `TL`, `Tj`, `TJ`,
`q`, `Q`, `cm`.

**Estado de texto incompleto** (correcção do dono): `Tc`, `Tw`, `Tz`, `Ts` afectam a posição
real — não são opcionais nem mero diagnóstico. O intérprete **deve reconhecer** `Tc`, `Tw`,
`Tz` e `Ts`. Se esses operadores aparecerem e **não forem aplicados** no cálculo de posição, o
resultado recebe o diagnóstico `TextStateNotFullyApplied` e a geometria deve ser tratada como
aproximada — não silenciosamente exacta. Se a implementação os aplicar, o diagnóstico **não**
é emitido.

### Diagnósticos

```rust
pub enum PageDiagnostic {
    ImageOnlyPage,              // página sem operadores de texto, com XObject de imagem
    NoTextOperators,            // página sem operadores de texto
    UnmappedGlyphs,             // há glifos sem mapeamento ToUnicode
    TextStateNotFullyApplied,   // Tc/Tw/Tz/Ts presentes mas não processados
    FormXObjectNotTraversed,    // Do com Form XObject de conteúdo detectado e não percorrido (v1)
}
```

### Limitação explícita da v1 (Form XObjects)

Somente content streams directos da página são processados. Form XObjects invocados por `Do`
não são percorridos recursivamente; texto que exista apenas dentro deles não é extraído. Cada
`Do` com XObject de conteúdo gera o diagnóstico `FormXObjectNotTraversed`. (Premissa: Typst
não usa Form XObjects para texto de corpo — validação pendente no ADR 0002.)

## Critérios de verificação

Dado um content stream `BT /f0 12 Tf 100 700 Td (A) Tj ET` com CMap mapeando o código de "A"
Quando o intérprete corre
Então emite um `GlyphInstance` com `codepoints == Some(['A'])`, `mapping_status == Mapped`,
posição derivada de CTM × Tm e normalizada

Dado um `TJ` com ajuste numérico entre dois glifos
Quando o intérprete corre
Então a posição do segundo glifo reflecte o ajuste (avanço = largura do glifo + ajuste)

Dado um content stream com `Tw` (ou `Tc`/`Tz`/`Ts`) e a v1 sem processamento desse operador
Quando o intérprete corre
Então o resultado inclui `PageDiagnostic::TextStateNotFullyApplied`

Dado um content stream com `Do` invocando um Form XObject
Quando o intérprete corre
Então não emite glifos desse XObject e inclui `PageDiagnostic::FormXObjectNotTraversed`

Dado um glifo cujo código não tem entrada no `ToUnicode`
Quando o intérprete corre
Então `mapping_status == Unmapped`, `codepoints == None`, o glifo **não** é descartado e o
diagnóstico `UnmappedGlyphs` é registado

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| 2026-08-12 | Criação — decisão do dono: `GlyphInstance` átomo, `TextSpan` removido da fronteira (`TextRun` derivado, adiado); limitação Form XObjects com diagnóstico; absorve `entities/glyph-instance.md` | `glyph_instance.rs` (revisão), `text_interpreter.rs` (novo) |
| 2026-08-12 | Precisão do comportamento de `Tc`/`Tw`/`Tz`/`Ts` (reconhecer obrigatório; diagnóstico só se não aplicados); sequência Unicode inválida tratada como `Unmapped`; `f64` confirmado como tipo canónico (decisão do dono, ver ADR 0003) | — |
