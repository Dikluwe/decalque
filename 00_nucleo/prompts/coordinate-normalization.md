# Prompt: normalização de coordenadas de página

**Camada**: L1 — `01_core`
**Arquivo gerado**: `01_core/src/geometry/coordinate_normalization.rs` (novo) + testes no mesmo ficheiro; **remove** `01_core/src/entities/cartesian_origin.rs` na revisão de código correspondente
**Substitui**: `00_nucleo/prompts/_deprecated/cartesian-origin.md` (absorvido; a heurística de detecção é removida em favor da regra da especificação PDF)
**ADR**: `00_nucleo/adr/0003-convencoes-transversais.md` (`f64`)

## Contexto

Segunda pergunta do pipeline ("onde fica o ponto (0,0)?"). O espaço de usuário padrão do PDF
(ISO 32000-1) tem origem no canto inferior esquerdo e eixo Y crescendo para cima (YUp) —
isto é **regra do formato, não heurística de produtor**. Typst emite YUp; scans emitem YUp.
A única forma de um PDF ter coordenadas YDown no content stream é o produtor aplicar uma
matriz `cm` que inverte o eixo (ex.: `[1 0 0 -1 0 height]`) — e essa matriz é processada
pela máquina de estado do intérprete (`content-stream-text-model.md`), que mantém a CTM.
Logo, a posição que o intérprete produz **já está no espaço de usuário final da página**
(YUp, origem inferior esquerda).

A detecção heurística de orientação do prompt antigo era um artefato do protótipo do projeto
irmão (P948), que não rastreava a CTM completa. Com CTM rastreada, não há o que detectar:
resta apenas a **convenção de saída** do Decalque — origem no canto superior esquerdo, eixo
Y crescendo para baixo (YDown), mais intuitiva para quem lê o relatório ("y maior = mais para
baixo na página impressa"). A conversão é matemática pura e determinística.

## Restrições estruturais

- L1: zero I/O, função pura.
- Não depende de `03_infra`.
- **Não tenta adivinhar a orientação do PDF** — assume o espaço de usuário padrão (YUp) como
  entrada. Se um caso real de coordenadas fora do espaço padrão aparecer, é bug do
  intérprete (CTM), não desta função.
- Tipo numérico: `f64` (ADR 0003).

## Instrução

```rust
pub fn normalize_to_top_left(
    point: (f64, f64),
    page_geometry: &PageGeometry,
) -> (f64, f64);
```

Regras:

- `x_normalizado = point.0 - page_geometry.origin.0`;
- `y_normalizado = page_geometry.height - (point.1 - page_geometry.origin.1)`;
- A origem da caixa efectiva **é subtraída** (decisão do dono, 2026-08-12). A caixa efectiva
  não tem de começar em `(0,0)`: com uma `CropBox` deslocada, o canto superior esquerdo
  visual é `(x0, y1)`, não `(0, height)`. Ignorar a origem desloca todas as coordenadas do
  relatório por `(x0, y0)`. O desvio cancela-se nos deltas do motor (que mede relativo ao
  deslocamento mediano de cada cluster) mas **não** se cancela quando os dois lados têm
  caixas de origem diferente — o cenário do Caso 2. Ver `page-geometry-model.md`, regra 2b;
- Para o caso comum `origin == (0.0, 0.0)` as fórmulas reduzem-se a `x' = x` e
  `y' = height - y`;
- Se o PDF aplicou uma matriz `cm` global que inverte o Y, o intérprete de texto já terá
  processado isso — a posição que chega aqui está no espaço de usuário final da página;
- **A função não aplica clamp.** Coordenadas fora da caixa da página são normalizadas
  matematicamente e podem resultar em valores negativos ou maiores que
  `page_geometry.height` (ex.: `point.1 > height` → `y_normalizado < 0.0`). Glifos fora da
  página existem em PDFs reais; clipping é regra do domínio do relatório, não do núcleo —
  o consumidor decide como tratar (decisão do dono);
- Documentar a convenção de saída (YDown, origem canto superior esquerdo) no doc-comment —
  escolha explícita, não implícita.

Nota sobre rotação: `page_geometry.height` é a altura da caixa efectiva **antes** da rotação
(`page-geometry-model.md`). Se a normalização de páginas com `Rotate != 0` precisar de
tratamento adicional, isso será decidido em revisão deste prompt com um caso real — registar
como limitação no doc-comment.

## Resultado esperado

- `01_core/src/geometry/coordinate_normalization.rs`:
  - `normalize_to_top_left`
  - testes inline (`#[cfg(test)]`) cobrindo os critérios
- `01_core/src/entities/cartesian_origin.rs` removido; re-exports em `lib.rs`/`entities/mod.rs`
  actualizados.

## Critérios de verificação

Dado um `PageGeometry` com `height = 800.0`
E um ponto no espaço do PDF `(100.0, 700.0)` (próximo ao topo visual da página)
Quando `normalize_to_top_left` é chamado
Então devolve `(100.0, 100.0)` (próximo ao topo na convenção de saída)

Dado um ponto `(0.0, 0.0)` (canto inferior esquerdo do espaço PDF)
Quando `normalize_to_top_left` é chamado com `height = 800.0`
Então devolve `(0.0, 800.0)` (canto inferior esquerdo na convenção de saída)

Dado um ponto `(50.0, 400.0)` (meio da página em Y)
Quando `normalize_to_top_left` é chamado com `height = 800.0`
Então devolve `(50.0, 400.0)` (o meio é invariante; X nunca é alterado)

Dado um ponto `(100.0, 900.0)` (fora da página, acima do topo no espaço PDF)
Quando `normalize_to_top_left` é chamado com `height = 800.0`
Então devolve `(100.0, -100.0)` (valor negativo na convenção de saída, sem clamp)

Dado um `PageGeometry` com `origin = (10.0, 20.0)`, `height = 792.0` (caixa deslocada)
E o ponto `(10.0, 812.0)` — o canto superior esquerdo da caixa efectiva
Quando `normalize_to_top_left` é chamado
Então devolve `(0.0, 0.0)` (o canto superior esquerdo da caixa é a origem da saída)

Dado o mesmo `PageGeometry` com `origin = (10.0, 20.0)`, `height = 792.0`
E o ponto `(10.0, 20.0)` — o canto inferior esquerdo da caixa efectiva
Quando `normalize_to_top_left` é chamado
Então devolve `(0.0, 792.0)`

Dado um `PageGeometry` com `origin = (0.0, 0.0)` e `height = 800.0`
Quando `normalize_to_top_left` é chamado com qualquer ponto
Então o resultado é idêntico ao da fórmula sem origem (o caso comum não regride)

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| 2026-08-12 | Criação — decisão do dono: espaço de usuário PDF é YUp por especificação (ISO 32000-1); a inversão YDown só existe via `cm`, já processada pela CTM do intérprete; a heurística de detecção de `03_infra` é removida; a struct `CartesianOrigin` é substituída por função pura; absorve `_deprecated/cartesian-origin.md` | `coordinate_normalization.rs` (novo), `cartesian_origin.rs` (removido) |
| 2026-08-12 | Revisão do dono: regra explícita de não-clamp para coordenadas fora da página (clipping é domínio do relatório) + critério correspondente | — |
| 2026-08-12 | Decisão do dono: a origem da caixa efectiva passa a ser subtraída (`x' = x − origin.0`, `y' = height − (y − origin.1)`), consumindo o campo `origin` novo de `PageGeometry` (`page-geometry-model.md`, regra 2b). A versão anterior assumia caixa com origem `(0,0)` e deslocava todo o relatório por `(x0, y0)` numa `CropBox` deslocada — desvio que cancela nos deltas relativos ao cluster mas não quando os dois lados têm origens diferentes (Caso 2). Critérios de caixa deslocada e de não-regressão do caso comum acrescentados | `coordinate_normalization.rs` (novo), `page_geometry.rs` (revisão) |
