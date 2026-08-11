# Prompt: motor de emparelhamento e comparação

**Camada**: L1 — `01_core`
**Arquivo gerado**: `01_core/src/engine/compare.rs` + testes no mesmo ficheiro

## Contexto

Dado dois `DocumentGeometry` (já normalizados), emparelhar os `GlyphInstance` correspondentes e
calcular o delta de posição de cada par, usando a `MeasurementResolution` para decidir o que conta
como divergência.

## Lições do protótipo Python (P948, projecto irmão) a preservar no desenho

1. **Emparelhamento por conteúdo + ordem de leitura, não por posição absoluta** — a ordem de
   emissão no content stream pode divergir entre dois PDFs (peças de assembly, por exemplo) mesmo
   quando o resultado visual é equivalente. Ordenar por posição (y depois x, "ordem de leitura")
   antes de emparelhar, e usar os `codepoints` como âncora principal do emparelhamento (algo
   equivalente a `difflib.SequenceMatcher` do protótipo — confirmar a biblioteca Rust equivalente,
   por exemplo `similar` ou implementação própria de LCS, na Fase A da implementação).

   **Normalização de ligaduras** (ADR 0001): antes de emparelhar, expandir a sequência textual de
   cada lado a nível de codepoint — um glifo de ligadura contribui os seus vários codepoints
   (`GlyphInstance.codepoints`), dois glifos simples contribuem um cada. Assim "fi" (1 glifo no
   original) e "f"+"i" (2 glifos no gerado) emparelham a nível de texto; o par registado pode ser
   1-para-N glifos, e o delta de posição usa a posição do primeiro glifo de cada lado (a
   diferença de largura entre ligadura e expandido não é divergência de posição a reportar —
   documentar esta escolha no doc-comment).
2. **Delta relativo à origem do cluster, não da página inteira** — comparar dois documentos cuja
   página tem tamanhos diferentes (legítimo, ver `PageGeometry`) produz deltas grandes e
   sem sentido se a comparação for feita em coordenadas absolutas de página. Agrupar glifos em
   clusters (candidato inicial: por proximidade vertical, mesmo critério "linhas" que P948 usou) e
   medir o delta relativo à origem de cada cluster.
3. **A métrica de triagem robusta é a mediana, não o máximo** — P948 confirmou empiricamente que
   `max|Δ|` tem artefactos (pares espúrios, clusters de composição diferente deslocando a origem)
   que a mediana não tem. O resultado agregado por documento/secção deve expor os dois, mas
   recomendar a mediana como sinal primário de triagem.
4. **Glifos sem par de um dos lados não são erro** — documentar explicitamente na struct de
   resultado quantos glifos de cada lado ficaram sem par (pode ser sintoma real — conteúdo a mais/
   a menos — ou limitação do emparelhamento; não decidir qual sem inspecção humana).

## Instrução (esqueleto, refinar na implementação)

```rust
struct GlyphPair<'a> {
    a: &'a GlyphInstance,
    b: &'a GlyphInstance,
    delta: (f64, f64),
    within_resolution: bool,
}

struct ComparisonReport<'a> {
    pairs: Vec<GlyphPair<'a>>,
    unmatched_a: Vec<&'a GlyphInstance>,
    unmatched_b: Vec<&'a GlyphInstance>,
    median_abs_dx: f64,
    median_abs_dy: f64,
    max_abs_dx: f64,
    max_abs_dy: f64,
}

fn compare(
    a: &DocumentGeometry,
    b: &DocumentGeometry,
    resolution: &MeasurementResolution,
) -> ComparisonReport;
```

## Ligação ao Caso 2 (scan→digital)

O mesmo motor serve o Caso 2 sem alteração estrutural: o lado do scan entra como um
`DocumentGeometry` produzido por extracção externa (ver
`00_nucleo/prompts/case2-scan-to-digital.md`). O que muda por caso de uso é **parâmetro, não
algoritmo**: a `MeasurementResolution` passada pelo chamador (tolerância mais larga no Caso 2)
e a relevância da normalização de ligaduras (item 1). O motor não sabe nem precisa de saber de
que caso de uso está a servir — documentar isto no doc-comment de `compare`.

## Critérios de verificação

Dado dois `DocumentGeometry` idênticos (mesmos glifos, mesmas posições)
Quando `compare` é chamado
Então todos os pares têm `delta == (0.0, 0.0)`, `unmatched_a`/`unmatched_b` vazios,
`median_abs_dx == median_abs_dy == 0.0`

Dado dois `DocumentGeometry` com um glifo deslocado 5pt em x, dentro de um cluster com outros
glifos inalterados
Quando `compare` é chamado
Então só esse par tem `delta.0 ≈ 5.0`; os restantes pares do mesmo cluster continuam com delta
~0 (confirma que a origem do cluster não foi contaminada pelo deslocamento de um único glifo)

Dado o documento A com um glifo de ligadura cujos `codepoints` são `['f','i']` na posição x=100,
e o documento B com dois glifos simples `['f']` e `['i']` a começar na mesma posição
Quando `compare` é chamado
Então a sequência textual emparelha (sem divergência de conteúdo) e nenhum dos glifos fica em
`unmatched_*` — a expansão de ligadura impede falsa divergência (ADR 0001)

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| 2026-08-11 | Criação inicial + primeira geração de `01_core` | `compare.rs` |
| 2026-08-11 | ADR 0001: normalização de ligaduras (âncora passa a ser `codepoints`); secção de ligação ao Caso 2; novo critério de verificação de ligadura | — |
