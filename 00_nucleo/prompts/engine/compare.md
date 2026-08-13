# Prompt: motor de emparelhamento e comparação

**Camada**: L1 — `01_core`
**Arquivo gerado**: `01_core/src/engine/compare.rs` + testes no mesmo ficheiro
**Depende de**: `00_nucleo/prompts/document-geometry.md` (`DocumentGeometry`), `00_nucleo/prompts/content-stream-text-model.md` (`GlyphInstance`), `00_nucleo/prompts/entities/measurement-resolution.md` (`MeasurementResolution`)

## Contexto

Dado dois `DocumentGeometry` (posições já normalizadas pelo intérprete via
`normalize_to_top_left` — ver `coordinate-normalization.md`), emparelhar os `GlyphInstance`
correspondentes e calcular o delta de posição de cada par, usando a `MeasurementResolution`
para decidir o que conta como divergência.

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
2. **Delta relativo ao cluster, não à página inteira** — comparar dois documentos cuja
   página tem tamanhos diferentes (legítimo, ver `PageGeometry`) produz deltas grandes e
   sem sentido se a comparação for feita em coordenadas absolutas de página. Agrupar glifos em
   clusters (candidato inicial: por proximidade vertical, mesmo critério "linhas" que P948 usou) e
   medir o delta relativo ao cluster.

   **A referência do cluster é o deslocamento mediano dos seus pares, não o primeiro glifo**
   (decisão do dono, 2026-08-12). Tomar o primeiro glifo em ordem de leitura como origem tem
   uma falha de atribuição: se for esse o glifo deslocado, o par dele mede `0.0` e todos os
   vizinhos inocentes da linha medem o simétrico do deslocamento real — o relatório aponta o
   glifo errado, e aponta vários em vez de um. A mediana dos offsets não se move com um glifo
   isolado, pelo que o culpado mede o deslocamento e os vizinhos medem zero. É a mesma
   escolha de estatística robusta da lição 3, aplicada à referência em vez de ao agregado.
3. **A métrica de triagem robusta é a mediana, não o máximo** — P948 confirmou empiricamente que
   `max|Δ|` tem artefactos (pares espúrios, clusters de composição diferente deslocando a origem)
   que a mediana não tem. O resultado agregado por documento/secção deve expor os dois, mas
   recomendar a mediana como sinal primário de triagem.
4. **Glifos sem par de um dos lados não são erro** — documentar explicitamente na struct de
   resultado quantos glifos de cada lado ficaram sem par (pode ser sintoma real — conteúdo a mais/
   a menos — ou limitação do emparelhamento; não decidir qual sem inspecção humana).
5. **Uma medição sem pares não é uma medição** (decisão do dono, 2026-08-12) — com `pairs`
   vazio, mediana e máximo de `f64` saem `0.0`, e o sinal primário de triagem (lição 3)
   declara paridade perfeita exactamente no pior caso possível. O mesmo sintoma aparece,
   mais silenciosamente, no caso **esparso**: poucos pares entre muitos glifos também
   produzem mediana excelente. As métricas passam a `Option<f64>` (o tipo obriga o
   consumidor a tratar a ausência) **e** o relatório passa a expor cobertura por lado (o
   número que denuncia o caso esparso, que o `Option` não apanha).

## Como o motor usa (e não usa) os campos do novo `GlyphInstance`

O `GlyphInstance` actual (`content-stream-text-model.md`) tem mais campos que o da primeira
geração. Decisões do dono (2026-08-12):

- **`mapping_status`**: glifos `Mapped` emparelham por âncora textual (lição 1). Glifos
  `Unmapped` não têm âncora textual — são emparelhados **posicionalmente**: depois do
  emparelhamento textual, dentro de cada cluster, os `Unmapped` restantes de cada lado são
  ordenados por x e emparelhados 1-para-1 nessa ordem; excedentes vão para `unmatched_*`.
  Dois `Unmapped` nunca são considerados "o mesmo" por `glyph_code` — códigos de glifo não
  são comparáveis entre documentos produzidos por compiladores/subsets de fonte diferentes.
- **`glyph_code`**: não usado no emparelhamento (ver acima); fica no glifo apenas para
  diagnóstico/rastreio.
- **`advance`**: **não usado na v1** — o delta é de posição apenas. Uma métrica futura de
  "avanço divergente" (largura de texto diferente com posição inicial igual) fica registada
  como extensão possível, não implementar agora.
- **`font_size_pt`**: usado no limiar de clusterização (novo cluster quando
  `|Δy| > 0.5 × font_size_pt` do glifo) e na `MeasurementResolution::RelativeToEm`.
- **`font_ref`**: **não usado para agrupar na v1** — nomes de fonte divergem legitimamente
  entre compiladores; agrupar por fonte esconderia divergências de conteúdo.
- **`render_mode`**: **não usado na v1** (decisão do dono, 2026-08-12). Glifos invisíveis
  (`Tr 3`) participam da comparação como quaisquer outros — é o que permite comparar um
  scan pesquisável (camada de OCR) contra um digital nativo sem tratamento especial. Filtrar
  por modo de renderização, se um dia for desejado, é política de relatório (`02_shell`) ou
  parâmetro do caso de uso, não algoritmo do motor. O diagnóstico que sinaliza a presença de
  texto invisível é `TextInterpreterDiagnostic::InvisibleTextPresent`
  (`content-stream-text-model.md`), não sai do motor.

## Instrução (esqueleto, refinar na implementação)

```rust
struct GlyphPair<'a> {
    a: &'a GlyphInstance,
    b: &'a GlyphInstance,
    delta: (f64, f64),          // offset do par menos o deslocamento mediano do cluster
    within_resolution: bool,
}

struct ClusterShift {
    index: usize,               // ordem vertical do par de clusters
    shift: (f64, f64),          // deslocamento mediano da linha, em pontos
    pairs: usize,               // pares que sustentam esta mediana
}

struct Coverage {
    matched_a: usize,           // glifos de A que ficaram emparelhados
    total_a: usize,
    matched_b: usize,
    total_b: usize,
}

struct ComparisonReport<'a> {
    pairs: Vec<GlyphPair<'a>>,
    unmatched_a: Vec<&'a GlyphInstance>,
    unmatched_b: Vec<&'a GlyphInstance>,
    median_abs_dx: Option<f64>, // None quando pairs está vazio
    median_abs_dy: Option<f64>,
    max_abs_dx: Option<f64>,
    max_abs_dy: Option<f64>,
    coverage: Coverage,
    cluster_shifts: Vec<ClusterShift>,
}

fn compare(
    a: &DocumentGeometry,
    b: &DocumentGeometry,
    resolution: &MeasurementResolution,
) -> ComparisonReport;
```

### Ordem de cálculo dentro de cada par de clusters

A referência mediana obriga a emparelhar **antes** de medir (a versão anterior calculava a
origem primeiro, porque era a posição de um glifo conhecido):

1. emparelhar os glifos do par de clusters (âncora textual, depois posicional);
2. offset bruto de cada par: `o_i = posição_b_i − posição_a_i`;
3. deslocamento do cluster: `shift = (mediana(o_i.x), mediana(o_i.y))` — mediana por eixo,
   independentemente;
4. delta de cada par: `delta_i = o_i − shift`;
5. `within_resolution` de cada par, com o `font_size_pt` do glifo de A.

Regras de fronteira:

- **Cluster sem pares** (todos os glifos de um lado ou do outro ficaram sem par): não produz
  entrada em `cluster_shifts` e não contribui deltas. Não inventar `shift = (0,0)` — não há
  medição.
- **Cluster com um único par**: `shift` é o próprio offset e o `delta` é necessariamente
  `(0.0, 0.0)`. É inerente à medição relativa, não um defeito: com uma só referência não há
  como distinguir "linha deslocada" de "linha correcta". O `ClusterShift` correspondente
  (`pairs: 1`) é o que torna o caso inspeccionável. Documentar no doc-comment.
- **`cluster_shifts` nunca é somado aos deltas dos glifos** — é informação paralela. Um
  deslocamento sistemático de linha ou de página é visível ali e só ali; os deltas por glifo
  continuam a medir divergência *dentro* da linha, que é o que a lição 2 pede.

### Cobertura

- `total_a`/`total_b`: número de glifos de cada `DocumentGeometry`.
- `matched_a`/`matched_b`: glifos que ficaram emparelhados — incluindo os que a expansão de
  ligadura cobriu e que não são o primeiro glifo do par (um par 1-para-N marca N glifos desse
  lado como emparelhados, mas conta como **um** par).
- Invariante verificável: `matched_a + unmatched_a.len() == total_a`, idem para B.
- `pairs.len()` **não** é uma medida de cobertura, precisamente por causa dos pares 1-para-N.
- Regra para `02_shell` (registar no doc-comment): a mediana nunca é apresentada sem a
  cobertura ao lado. Uma mediana de `0.02pt` sobre 3 de 400 glifos não é paridade — é uma
  medição que quase não aconteceu.

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
`median_abs_dx == median_abs_dy == Some(0.0)`, `coverage.matched_a == coverage.total_a` e
todos os `cluster_shifts` têm `shift == (0.0, 0.0)`

Dado dois `DocumentGeometry` com um glifo deslocado 5pt em x, dentro de um cluster com outros
glifos inalterados, **não sendo esse o primeiro glifo da linha**
Quando `compare` é chamado
Então só esse par tem `delta.0 ≈ 5.0`; os restantes pares do mesmo cluster continuam com delta
~0

Dado uma linha de três glifos em que **o primeiro em ordem de leitura** é o deslocado 5pt em x
Quando `compare` é chamado
Então o par desse glifo tem `delta.0 ≈ 5.0` e os outros dois têm `delta.0 ≈ 0.0`

(É o critério que o desenho anterior falhava: com a origem no primeiro glifo, os deltas saíam
`0.0, −5.0, −5.0` — o culpado inocentado e dois inocentes acusados. Nenhum critério anterior
punha o glifo deslocado em primeiro lugar, e foi por isso que a falha passou despercebida.)

Dado uma linha inteira deslocada 5pt em x (todos os glifos, mesma quantidade)
Quando `compare` é chamado
Então todos os pares dessa linha têm `delta ≈ (0.0, 0.0)` e o `ClusterShift` correspondente
regista `shift.0 ≈ 5.0` (o deslocamento sistemático não desaparece: muda de sítio)

Dado um par de clusters com um único par de glifos
Quando `compare` é chamado
Então `delta == (0.0, 0.0)` e o `ClusterShift` correspondente tem `pairs == 1` (limitação
inerente da medição relativa, tornada inspeccionável)

Dado dois `DocumentGeometry` sem nenhum par emparelhado (conteúdo totalmente divergente)
Quando `compare` é chamado
Então `median_abs_dx`, `median_abs_dy`, `max_abs_dx` e `max_abs_dy` são `None`,
`cluster_shifts` está vazio, e `coverage.matched_a == 0` com `total_a` a reflectir os glifos
existentes — nunca `Some(0.0)`, que se leria como paridade perfeita

Dado um documento A com 400 glifos e um B em que só 3 emparelham, todos na mesma posição
Quando `compare` é chamado
Então `median_abs_dx == Some(0.0)` **e** `coverage` regista `matched_a == 3`, `total_a == 400`
(a mediana sozinha diria paridade; a cobertura é o que revela que quase nada foi medido)

Dado qualquer comparação
Quando `compare` é chamado
Então `coverage.matched_a + unmatched_a.len() == coverage.total_a`, idem para B (invariante de
contagem, incluindo pares 1-para-N de ligadura)

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
| 2026-08-12 | `render_mode` acrescentado à secção de uso dos campos: não usado na v1: glifos invisíveis (`Tr 3`, camada de OCR) comparam como quaisquer outros; filtrar por modo é política de relatório, não algoritmo | — |
| 2026-08-12 | Decisão do dono (achado 6): a referência do cluster passa do primeiro glifo em ordem de leitura para o **deslocamento mediano dos pares do cluster**. Com a origem no primeiro glifo, um deslocamento nesse glifo media `0.0` no par dele e o simétrico do deslocamento em todos os vizinhos — atribuição invertida, um culpado inocentado e vários inocentes acusados. Ordem de cálculo passa a emparelhar → offsets → mediana → deltas. Regras de fronteira para cluster sem pares e com um único par. Critério novo com o glifo deslocado em primeiro lugar — nenhum critério anterior o punha lá, e foi por isso que a falha passou | `compare.rs` (revisão) |
| 2026-08-12 | Decisão do dono (achado 6, consequência): `cluster_shifts: Vec<ClusterShift>` exposto no relatório. O deslocamento mediano fica calculado de qualquer forma, e sem o expor um deslocamento sistemático de linha ou de página deixa de ser observável em qualquer sítio. Nunca somado aos deltas dos glifos | `compare.rs` (revisão) |
| 2026-08-12 | Decisão do dono (achado 5): métricas agregadas passam a `Option<f64>` (`None` com `pairs` vazio) e o relatório ganha `Coverage` por lado. Com `f64`, um relatório sem pares declarava mediana e máximo `0.0` — paridade perfeita no pior caso possível — e o caso esparso (poucos pares em muitos glifos) tinha o mesmo sintoma sem que o tipo o denunciasse. Invariante `matched + unmatched == total`; `pairs.len()` declarado inválido como medida de cobertura (pares 1-para-N); regra para `02_shell` de nunca apresentar mediana sem cobertura | `compare.rs` (revisão) |
| 2026-08-12 | Revisão do dono: secção "Como o motor usa os campos do novo `GlyphInstance`" — `Unmapped` emparelha posicionalmente por x dentro do cluster (nunca por `glyph_code`); `advance` e `font_ref` fora da v1; `font_size_pt` no cluster e na resolução; `Depende de` com `document-geometry.md`; referência à normalização corrigida para `normalize_to_top_left` | — |
