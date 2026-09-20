# Prompt: busca discreta de corpo e tracking Typst por linhas

## Intenção

Materializar o primeiro refinamento automático do Decalque. Uma grade explícita de corpos e
trackings é avaliada integralmente pelo ciclo `ScanObservation -> Typst -> PDF -> comparação`.
Somente evidência estrutural comparável pode selecionar uma hipótese; ausência de evidência,
empate ou execução parcial nunca são convertidos em vencedor.

## Pré-requisitos

A busca reutiliza o planejamento por linhas, a emissão Typst física, a compilação limitada, o PDF
em memória, a comparação tri-state e a publicação atômica. A grade explícita acrescenta seleção
automática sem alterar essas fronteiras.

## Interface observável

```text
decalque fit-scan-lines OBSERVATION.json
    --raster PAGE
    --output-pdf WINNER.pdf
    --font-family FAMILY
    --font-size-pt NUMBER [--font-size-pt NUMBER ...]
    [--font-weight regular|bold]
    [--font-style normal|italic|oblique]
    [--tracking-pt NUMBER ...]
    --horizontal-tolerance-pt NUMBER
    --baseline-tolerance-pt NUMBER
    [--min-text-confidence NUMBER]
    [--min-geometry-confidence NUMBER]
    [--typst-bin PATH]
```

`--font-size-pt` e `--tracking-pt` são as únicas opções repetíveis. Tracking ausente equivale à
lista explícita `[0]`. Os demais defaults coincidem com `evaluate-scan-lines`.

O parser completo roda antes de I/O e deve:

1. exigir ao menos um corpo em `[4, 96]`;
2. exigir cada tracking em `[-2, 2]`;
3. rejeitar `NaN`, infinito, overflow textual e valores não UTF-8 onde texto é obrigatório;
4. normalizar zero assinado, ordenar por `total_cmp` e rejeitar duplicatas;
5. construir a grade corpo-major/tracking-minor;
6. rejeitar produto vazio, overflow de multiplicação ou mais de 32 hipóteses;
7. preservar caminhos nativos e tratar `--typst-bin` como um único executável.

Não existe flag de granularidade: o contrato v1 compara linhas.

## Ordem causal obrigatória

L4 executa:

1. parse e fechamento integral da grade;
2. observação JSON estrita;
3. vinculação integral do raster;
4. pré-condições de planejamento independentes do candidato;
5. identificação limitada do compilador uma única vez;
6. para cada hipótese canônica, sem feedback entre tentativas:
   - planejamento L1;
   - fonte Typst determinística;
   - compilação limitada em memória;
   - PDF estrutural de exatamente uma página;
   - `PageSource -> DocumentGeometry`;
   - comparação por linha;
   - evidência de ajuste L1;
7. seleção somente depois da última tentativa;
8. se única, repetição integral da hipótese selecionada e prova de igualdade;
9. serialização completa;
10. publicação exclusiva do PDF confirmado;
11. stdout.

Corpo, tracking, quantidade e ordem das hipóteses são imutáveis antes do passo 2. O PDF de uma
tentativa não pode alterar observação, mapping, política, outra hipótese ou seus bytes Typst.

## Limites

- no máximo 32 tentativas da grade;
- no máximo uma confirmação;
- uma invocação de versão por busca;
- execução sequencial;
- mesmos limites por compilação da avaliação v1: fonte 16 MiB, PDF 128 MiB, stderr 1 MiB e 30 s;
- versão: 64 KiB e 30 s;
- nenhum limite pode ser elevado pela CLI.

Pelo teto fechado, versão, grade e confirmação somam no máximo 34 invocações limitadas do
executável. Falha em qualquer uma encerra a busca; tentativas concluídas não autorizam resultado
parcial.

## Contrato puro de seleção

### Elegibilidade

Uma tentativa compete somente se:

- `total_scan > 0`;
- `content_status == Preserved`;
- `matched_scan == total_scan` e `matched_candidate == total_candidate`;
- `unmatched_scan` e `unmatched_candidate` estão vazios;
- os contadores de cobertura correspondem a `matches` e às listas de não associados;
- cada `scan_unit_id` é não vazio e único na tentativa;
- todo match tem os três resíduos horizontais finitos;
- o estado horizontal de todo match é conhecido.

O conjunto ordenado de `scan_unit_id` horizontais e o conjunto de baselines conhecidas são
publicados como suporte. Suportes diferentes entre tentativas elegíveis produzem
`incomparable-support` para a busca inteira.

Baseline `Unknown` exige `baseline_delta == None` e não entra na chave. Baseline conhecida exige
delta finito e estado conhecido. Combinações contraditórias são erro de evidência.

### Quantização e chave

Cada absoluto é convertido por arredondamento ties-to-even:

```text
q(x) = round_ties_even(abs(x) * 1024)
```

`q` deve caber em `u64`. Para cada match:

```text
H_i = max(q(dx_start), q(dx_end), q(width_delta))
B_i = q(baseline_delta), quando conhecida
```

`H` e `B` são ordenados em ordem decrescente. A chave, minimizada lexicograficamente, é:

```text
(horizontal_violations, H, baseline_violations, B)
```

O estado `overall` não é reinterpretado. Tolerâncias não mudam. Resíduo ausente nunca recebe
zero. A chave não contém índice, família, corpo, tracking, hash, tamanho ou ordem de entrada.

### Seleção

- nenhum elegível: `inconclusive/no-eligible-trial`;
- suportes divergentes: `inconclusive/incomparable-support`;
- menor chave única: `selected`;
- menor chave compartilhada: `tied`, listando todos os índices.

`selected` publica o PDF apenas depois da confirmação. `tied` e `inconclusive` retornam JSON com
código zero e não criam, removem nem alteram o destino.

Se nenhuma baseline for conhecida no suporte comum, `evidence_scope` é `horizontal-only`. Caso
contrário, é `horizontal-and-observed-baseline`; isso não transforma baselines restantes em
conhecidas.

## Confirmação

A hipótese selecionada é planejada, emitida, compilada, materializada e comparada novamente. A
confirmação precisa repetir exatamente:

- bytes e hash da fonte;
- bytes e hash do PDF;
- `ScanComparisonReport`;
- suporte e chave.

Qualquer diferença é erro de execução `non-deterministic-winner`, deixa stdout vazio e não publica.

## Relatório v1

O JSON externo usa schema `decalque.scan-typography-search`, versão 1, ordem fixa e contém:

- `source.page_index` e `source.raster_sha256`;
- `search_space` com família, peso, estilo, listas canônicas e quantidade;
- `compiler_version`;
- `trials`, na ordem canônica, cada qual com hipótese completa, hashes/tamanhos de fonte e PDF,
  estados e cobertura originais, elegibilidade/motivo, suporte e chave quantizada;
- `selection` com status, motivo ou índices, `evidence_scope` e `artifact_published`;
- `winner`, nulo salvo em `selected`; quando presente, incorpora integralmente
  `decalque.scan-reconstruction-evaluation` da confirmação.

Em ordem canônica, a forma v1 é:

1. `schema`, `schema_version`;
2. `source` com `page_index`, `raster_sha256`;
3. `search_space` com `font_family`, `font_weight`, `font_style`, `font_sizes_pt`,
   `trackings_pt`, `hypothesis_count`;
4. `compiler_version`;
5. `trials`;
6. `selection`;
7. `winner`.

Cada item de `trials` contém, nesta ordem, `index`, `hypothesis`, `source_sha256`,
`source_size_bytes`, `pdf_sha256`, `pdf_size_bytes`, `content_status`, `geometry_status`,
`overall_status`, `coverage`, `eligibility`, `support` e `score`. `eligibility.status` é
`eligible` ou `ineligible`; somente o segundo acrescenta `reason`, escolhido entre
`empty-scan-scope`, `content-not-preserved`, `partial-coverage`, `unmatched-units` e
`missing-horizontal-evidence`. `support` contém `horizontal_scan_unit_ids` e
`known_baseline_scan_unit_ids`. `score` contém `horizontal_violations`,
`horizontal_residuals_desc`, `baseline_violations` e `baseline_residuals_desc`. `support` e
`score` são `null` para tentativa inelegível.

`selection` usa exatamente `status` seguido de: `selected_index` em `selected`, `indices` em
`tied`, ou `reason` (`no-eligible-trial` ou `incomparable-support`) em `inconclusive`; depois
`evidence_scope` e `artifact_published`. O escopo é `horizontal-only` ou
`horizontal-and-observed-baseline` para seleção ou empate comparável, e `null` para
inconclusão. `artifact_published` é verdadeiro somente após seleção confirmada. Não existem os
aliases `winner_index` ou `indices` para uma seleção única. Campos duplicados e bytes após o
objeto JSON são inválidos.

Relatório não contém timestamps, caminhos, diretório, duração ou stderr arbitrário. Mesma entrada,
ambiente de fontes e compilador produz os mesmos bytes.

## Estados de processo

| Resultado | Exit | stdout | PDF final |
|---|---:|---|---|
| `selected` confirmado | 0 | relatório | criado em caminho novo |
| `tied` | 0 | relatório | ausente/intocado |
| `inconclusive` | 0 | relatório | ausente/intocado |
| erro de uso/execução/não determinismo/publicação | 2 | vazio | ausente/intocado |

Um destino preexistente nunca é sobrescrito. Em `selected`, erro de publicação ocorre depois da
serialização e antes de stdout. PDFs intermediários jamais ganham nome no filesystem.

## Distribuição Tekt

### L1

- tipos de suporte, chave, tentativa e seleção;
- validação de coerência do relatório;
- quantização e ordenação puras;
- nenhuma hipótese gerada, I/O, JSON, Typst ou dependência externa.

### L2

- parser completo e canonicalização da grade;
- limites fechados;
- renderizador determinístico do relatório de busca;
- nenhuma abertura de arquivo ou execução.

### L3

- sessão identificada do compilador: versão uma vez, compilações limitadas repetíveis;
- reutilização dos adaptadores PDF em memória e publicação atômica;
- nenhuma pontuação ou seleção.

### L4

- ordem causal, enumeração exaustiva, confirmação e publicação;
- memória limitada a artefatos correntes e ao melhor provisório, sem acumular 32 PDFs máximos;
- categorias de erro distintas e stdout apenas no fim.

## Testes comportamentais

### L1

1. resíduo `None` nunca equivale a zero e torna a tentativa inelegível ou a baseline ausente;
2. cobertura parcial com resíduo zero não compete com cobertura integral;
3. suporte de ids diferente torna a busca inconclusiva, não desempata por contagem;
4. fronteiras de meio passo usam ties-to-even e valores que quantizam para o mesmo inteiro
   empatam;
5. a comparação usa primeiro violações, depois o pior resíduo, sem soma dependente de ordem;
6. empate retorna todos os co-vencedores e não usa índice nem hipótese;
7. baseline inteiramente desconhecida permite somente escopo horizontal;
8. `NaN`, infinito, delta parcial ou estado contraditório é erro de evidência.

### L2

1. permutar corpos/trackings gera a mesma grade e os mesmos índices canônicos;
2. zero assinado é normalizado; duplicata, faixa inválida, overflow e 33 hipóteses falham antes de
   I/O;
3. somente as duas opções publicadas podem repetir;
4. relatório mantém ordem, inteiros quantizados e `winner: null` em empate/inconclusão.

### L3/L4

1. versão é consultada uma vez para múltiplas compilações e os argumentos continuam fixos;
2. a fixture real com corpos `10, 18, 30` e trackings `-0.15, 0` seleciona unicamente
   `18 pt / -0.15 pt`, confirma o mesmo PDF e publica somente esse artefato;
3. duas execuções em destinos novos produzem JSON e PDF byte-idênticos;
4. compilador que emite o mesmo PDF para hipóteses distintas produz `tied` e nenhum PDF;
5. falha na última tentativa descarta todo resultado: stdout vazio e nenhum PDF;
6. observação/raster/plano inconclusivo falham antes da versão do compilador;
7. candidato algum altera a grade, a política ou a fonte das tentativas seguintes;
8. destino preexistente fica intacto; nenhum PDF intermediário é publicado;
9. confirmação divergente é erro e não publica;
10. `evaluate-scan-lines`, `reconstruct-scan-lines` e contratos 0004–0008 permanecem idênticos.

## Fora de escopo

- escolher ou provar família, peso ou estilo;
- busca adaptativa, contínua, paralela ou distribuída;
- tolerâncias como variável de ajuste;
- posição, escala, leading, parágrafos, imagens ou composição multipágina;
- pixels como função-objetivo autoritativa;
- alterar a observação ou completar `Unknown`.
