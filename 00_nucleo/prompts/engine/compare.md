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
| 2026-08-12 | `render_mode` acrescentado à secção de uso dos campos: não usado na v1: glifos invisíveis (`Tr 3`, camada de OCR) comparam como quaisquer outros; filtrar por modo é política de relatório, não algoritmo | — |
| 2026-08-12 | Revisão do dono: secção "Como o motor usa os campos do novo `GlyphInstance`" — `Unmapped` emparelha posicionalmente por x dentro do cluster (nunca por `glyph_code`); `advance` e `font_ref` fora da v1; `font_size_pt` no cluster e na resolução; `Depende de` com `document-geometry.md`; referência à normalização corrigida para `normalize_to_top_left` | — |
