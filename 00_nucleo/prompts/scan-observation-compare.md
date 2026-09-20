# Prompt: comparação `ScanObservation` → PDF digital

**Camada**: L1 — `01_core`
**Arquivo gerado futuramente**: `01_core/src/engine/scan_compare.rs`
**ADR**: `00_nucleo/adr/0004-fronteira-observacao-scan.md`
**Depende de**: `scan-observation-model.md`, `document-geometry.md`,
`content-stream-text-model.md`

## Obrigação

Comparar uma `ScanObservation` validada com o `DocumentGeometry` de uma página candidata sem
converter observações OCR em glifos. O motor constrói uma vista de linhas/palavras do lado
digital, associa texto exato e único, mede somente claims geometricamente comparáveis e produz
cobertura e vereditos trivalentes.

Este componente é distinto de `engine/compare.md`:

```text
engine/compare:
    DocumentGeometry × DocumentGeometry -> ComparisonReport

scan_compare:
    ScanObservation × DocumentGeometry × ScanComparisonPolicy
        -> ScanComparisonReport
```

## Restrições de camada

- L1: zero I/O, zero JSON, zero OCR, zero subprocessos.
- A observação já foi validada pelo contrato L1 após parsing L3.
- A política é valor explícito construído por L2; não existem tolerâncias globais escondidas.
- O motor não modifica nenhuma entrada e não completa claims ausentes.
- Fonte declarada, família, estilo, peso e aparência raster não participam da v1.

## Política de entrada

```rust
pub enum ScanGranularity { Line, Word }

pub enum ConfidenceRequirement {
    Any,
    KnownAtLeast(f64),
}

pub enum TextNormalization {
    Exact,
}

pub struct ScanComparisonPolicy {
    pub granularity: ScanGranularity,
    pub horizontal_tolerance_pt: f64,
    pub baseline_tolerance_pt: f64,
    pub text_confidence: ConfidenceRequirement,
    pub geometry_confidence: ConfidenceRequirement,
    pub text_normalization: TextNormalization,
}
```

Regras:

- tolerâncias são finitas e não negativas;
- limiar de confiança conhecido está em `[0,1]`;
- a única normalização v1 é `Exact`: sequência de valores escalares Unicode idêntica, incluindo
  caixa, espaços e pontuação;
- claim `Known` que não satisfaz a política de confiança é tratada, no relatório, como
  `Unknown(BelowPolicyThreshold)`; a entrada não é alterada.

## Vista digital candidata

O motor deriva uma vista comparável sem I/O:

1. Ordenar glifos estavelmente por Y e depois X.
2. Formar linhas pelo mesmo critério geométrico documentado no motor digital: um novo cluster
   começa quando a diferença vertical excede `0.5 × font_size_pt` do glifo considerado.
3. Dentro da linha, ordenar por X e concatenar `codepoints`.
4. Cada valor escalar preserva referência ao `GlyphInstance` que o produziu; uma ligadura pode
   mapear vários escalares para o mesmo glifo.
5. Um glifo `Unmapped` torna desconhecido o texto integral da linha para associação exata. Ele
   permanece contabilizado na cobertura e nos diagnósticos.
6. Palavras candidatas são intervalos não vazios entre caracteres com a propriedade Unicode
   `White_Space`; os separadores não são palavras.
7. A geometria horizontal de um intervalo candidato é o envelope de, para cada glifo coberto,
   `position.x` e `position.x + advance`.
8. A baseline candidata é a mediana dos valores `position.y` dos glifos cobertos.

Uma fronteira de palavra que cair dentro da sequência de codepoints de uma única ligadura não
divide o glifo; a geometria da unidade fica `unknown` se a atribuição não puder ser feita sem
duplicar o mesmo glifo entre unidades.

## Associação por linha

Uma linha scan é elegível quando:

- `kind == line`;
- texto é `Known` e satisfaz a política de confiança;
- ordem, se usada para desambiguação futura, é preservada mas não torna uma ocorrência repetida
  única na v1.

Associação v1:

- comparar texto exato contra todas as linhas candidatas de texto conhecido;
- exatamente uma ocorrência → linha associada;
- zero ocorrências → linha não associada, conteúdo `unknown`;
- mais de uma ocorrência → `unknown(ambiguous)`;
- linha candidata com texto desconhecido nunca é escolhida por posição.

## Associação por palavra

Na granularidade `Word`, uma palavra scan é elegível quando:

- `kind == word`;
- possui pai `line`;
- texto e `span_in_parent` são `Known` e satisfazem a política;
- a linha pai foi associada exatamente uma vez;
- o span corresponde ao texto da palavra, já garantido pelo contrato de observação.

O span em valores escalares é aplicado à sequência candidata da linha associada. A geometria usa
os glifos que cobrem esse intervalo. Não procurar a palavra isoladamente na página depois de a
linha ter sido associada; isso evitaria desambiguação falsa de palavras repetidas.

Se a linha não associar, o motor pode procurar **apenas para testemunhar reflow**: quando todas
as palavras conhecidas da linha scan possuem correspondência textual individual única e, em
ordem, ocupam duas ou mais linhas candidatas, o resultado é `violated` com os índices dessas
linhas. Qualquer palavra repetida, ausente ou ambígua impede essa prova e mantém `unknown`.

## Projeção geométrica

Geometria scan só é comparável quando:

1. a claim geométrica exigida é `Known` e satisfaz confiança;
2. `page_mapping` é `Known` e satisfaz confiança;
3. bbox/baseline usam o frame raster declarado;
4. a página candidata tem `PageRotation::Deg0` e `user_unit == 1.0` na v1.

Rotação diferente ou `UserUnit` diferente de 1.0 produz diagnóstico
`UnsupportedCandidateCoordinateMapping` e geometria `unknown`; não aplicar a fórmula de
`Deg0` silenciosamente.

A homografia transforma todos os pontos relevantes. Para bbox, transformar os quatro cantos e
recalcular o envelope. Para baseline, transformar todos os pontos e usar a mediana dos Y
resultantes como baseline observada v1.

O tamanho da página candidata nunca entra na homografia do scan. Diferença entre o extent físico
da referência e a página candidata pode ser reportada como contexto, mas não é corrigida por
escala implícita.

## Medidas

Para cada unidade associada com geometria suficiente:

```text
dx_start = candidate.x0 - scan.x0
dx_end = candidate.x1 - scan.x1
d_width = (candidate.x1 - candidate.x0) - (scan.x1 - scan.x0)
dy_baseline = candidate.baseline_y - scan.baseline_y
```

- horizontal é `preserved` quando `|dx_start|`, `|dx_end|` e `|d_width|` são menores ou iguais a
  `horizontal_tolerance_pt`;
- qualquer um acima do limite torna a geometria da unidade `violated`;
- baseline é `preserved` quando `|dy_baseline| <= baseline_tolerance_pt`;
- claim necessária ausente, confiança insuficiente ou mapeamento sem suporte torna o componente
  `unknown`;
- nenhum delta desconhecido recebe valor zero.

## Resultado observável

```rust
pub enum EvidenceStatus { Preserved, Violated, Unknown }

pub struct ScanCoverage {
    pub matched_scan: usize,
    pub total_scan: usize,
    pub matched_candidate: usize,
    pub total_candidate: usize,
}

pub struct ScanMatch {
    pub scan_unit_id: String,
    pub candidate_line_index: usize,
    pub candidate_scalar_range: (usize, usize),
    pub dx_start: Option<f64>,
    pub dx_end: Option<f64>,
    pub width_delta: Option<f64>,
    pub baseline_delta: Option<f64>,
    pub horizontal_status: EvidenceStatus,
    pub baseline_status: EvidenceStatus,
}

pub struct ReflowWitness {
    pub scan_line_id: String,
    pub candidate_line_indices: Vec<usize>,
    pub scan_word_ids: Vec<String>,
}

pub struct ScanComparisonReport {
    pub content_status: EvidenceStatus,
    pub geometry_status: EvidenceStatus,
    pub overall_status: EvidenceStatus,
    pub coverage: ScanCoverage,
    pub matches: Vec<ScanMatch>,
    pub unmatched_scan: Vec<String>,
    pub unmatched_candidate: Vec<CandidateUnitRef>,
    pub reflow: Vec<ReflowWitness>,
    pub diagnostics: Vec<ScanComparisonDiagnostic>,
}
```

`Option<f64>` é `None` quando a medida não aconteceu; nunca representa zero implícito.

## Cobertura

- `total_scan`: todas as unidades da granularidade selecionada, inclusive texto/geometria
  desconhecidos.
- `matched_scan`: unidades textualmente associadas de forma única.
- `total_candidate`: todas as unidades candidatas derivadas na granularidade selecionada,
  inclusive linhas/palavras de texto desconhecido.
- `matched_candidate`: unidades candidatas cobertas por associações únicas.
- invariantes: `matched_* <= total_*`; listas de não associados explicam a diferença.
- `matches.len()` não substitui cobertura: uma palavra/ligadura pode abranger vários glifos.
- `total_scan == 0 && total_candidate == 0` define **escopo vazio**. A igualdade
  `matched == total == 0` é vacuosa e não satisfaz completude nem constitui evidência de
  preservação.

## Agregação trivalente

### Conteúdo

- `violated`: existe reflow demonstrado;
- `unknown`: sem violação, mas o escopo é vazio, há associação ambígua/ausente, texto
  desconhecido ou cobertura incompleta em qualquer lado;
- `preserved`: o escopo é não vazio nos dois lados e todas as unidades de ambos os lados estão
  associadas, sem reflow.

Texto não associado sozinho não é declarado violação na v1 porque pode ser erro OCR; ele reduz
cobertura e produz `unknown`.

### Geometria

- `violated`: ao menos uma medida conhecida excede a tolerância;
- `unknown`: sem violação, mas o escopo é vazio, `page_mapping` é `Unknown`, ou alguma unidade
  associada não possui todos os componentes geométricos obrigatórios;
- `preserved`: `page_mapping` é `Known` e satisfaz a política, o escopo é não vazio, todas as
  unidades associadas têm horizontal e baseline preservados e a cobertura de conteúdo é
  completa.

Em particular, ausência de medidas não prova geometria preservada. `page_mapping: Unknown`
impede `geometry_status: preserved` mesmo em `0/0`; nenhum default, identidade implícita ou
verdade por vacuidade é permitido.

### Geral

```text
se conteúdo ou geometria == violated -> violated
senão, se conteúdo ou geometria == unknown -> unknown
senão -> preserved
```

## Diagnósticos mínimos

- `TextClaimUnknown`
- `TextConfidenceBelowPolicy`
- `AmbiguousTextMatch`
- `UnmatchedText`
- `CandidateUnmappedGlyph`
- `GeometryClaimUnknown`
- `GeometryConfidenceBelowPolicy`
- `PageMappingUnknown`
- `UnsupportedCandidateCoordinateMapping`
- `ReflowDetected`
- `EmptyComparisonScope`

Diagnósticos identificam a unidade e não alteram a precedência dos vereditos.

## Critérios de verificação

1. Linha exacta e única é associada independentemente de sua posição absoluta.
2. Duas linhas candidatas com o mesmo texto tornam a associação `unknown`.
3. Texto sem match reduz cobertura e não vira violação automaticamente.
4. Glifo candidato `Unmapped` torna a linha textual desconhecida, sem uso de `glyph_code`.
5. Ligadura `fi` mapeia dois escalares e cobre corretamente um span dentro da palavra.
6. Span que exigiria dividir uma ligadura entre palavras produz geometria `unknown`.
7. Palavra repetida dentro de linha já associada usa seu span, não busca global.
8. Palavras únicas da mesma linha scan encontradas em duas linhas candidatas produzem reflow
   `violated`.
9. Uma palavra ambígua impede prova de reflow e mantém `unknown`.
10. Homografia 0.5 leva bbox `[100,200,300,400]` a `[50,100,150,200]`.
11. Bbox projetiva é calculada pelos quatro cantos, não escalando apenas largura/altura.
12. `page_mapping: Unknown` preserva associação textual, produz deltas `None` e impede
    `geometry_status: preserved`, inclusive sem unidades associadas.
13. Candidato rotacionado ou com `user_unit != 1.0` não produz deltas pela fórmula comum.
14. `dx_start`, `dx_end`, largura e baseline exatamente no limiar são preservados.
15. Qualquer delta conhecido acima do limiar produz testemunha `violated`.
16. Baseline desconhecida não é substituída pela borda da bbox.
17. Métricas zero com cobertura parcial produzem `overall_status: unknown`.
18. Uma violação conhecida domina outras unidades desconhecidas.
19. `preserved` exige cobertura completa e não vazia nos dois lados e todas as medidas
    obrigatórias.
20. Alterar dimensões do candidato não altera `page_mapping` nem as coordenadas scan projetadas.
21. O motor não cria nem devolve `GlyphInstance` para unidade OCR.
22. Repetir a comparação com as mesmas entradas e política produz relatório semanticamente
    idêntico.
23. Observação sem unidades e candidato sem unidades na granularidade selecionada produzem
    cobertura `0/0`, diagnóstico `EmptyComparisonScope` e os três status `unknown`.
24. `preserved` exige, além de completude, ao menos uma unidade em cada lado; `0/0` nunca é
    promovido por igualdade vacuosa.

## Histórico de Revisões

| Data | Motivo |
|---|---|
| 2026-09-18 | Contrato inicial do comparador Rust dedicado ao Caso 2, separado do motor por glifo. |
| 2026-09-18 | Refinamento pós-verificação: escopo vazio é `unknown`, `page_mapping: Unknown` veta preservação geométrica e o diagnóstico de ausência de evidência torna-se observável. |
