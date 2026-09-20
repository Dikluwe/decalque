# Decalque

Sistema de reconstrução tipográfica em Rust para transformar imagens de páginas de livros em
documentos digitais que preservem seu conteúdo e sua geometria. O Decalque observa a página,
formula um documento Typst candidato, materializa o PDF e usa comparação estrutural como sinal
de convergência. Pixels podem fornecer evidência ao observador, mas não substituem a estrutura do
PDF gerado nem autorizam completar medidas desconhecidas.

A arquitetura Tekt do produto possui somente as camadas `00` a `04`; não existe uma quinta camada
`05_scan`. OCR, modelos de visão, pré-processamento, busca de fontes e o executável Typst podem
integrar o produto como provedores ou processos substituíveis, mas permanecem fora do núcleo puro.

## Casos de uso

### 1. PDF digital → PDF digital

O Decalque materializa `DocumentGeometry` para a referência e para o candidato, emparelha glifos
por conteúdo e ordem e mede deltas geométricos segundo uma política explícita de tolerância.
Esse é o fluxo estrutural original e está disponível pela CLI:

```sh
cargo run --bin decalque -- referencia.pdf candidato.pdf [--page indice]
```

### 2. Página de livro observada → documento tipográfico digital

Um provedor observa o raster sem consultar o candidato e emite exatamente uma página no contrato
JSON versionado `decalque.scan-observation`, versão 1. A observação, uma hipótese tipográfica e
uma política editorial formam um plano; esse plano produz Typst, e o PDF compilado volta ao
pipeline Rust como candidato estrutural.

```text
raster -> provedor substituível -> ScanObservation v1
ScanObservation + hipóteses -> ReconstructionPlan -> Typst -> PDF candidato

PDF candidato
    -> DocumentGeometry

ScanObservation × DocumentGeometry × ScanComparisonPolicy
    -> ScanComparisonReport
```

```sh
cargo run --bin decalque -- validate-scan-observation observacao.json \
  --raster pagina.png

cargo run --bin decalque -- reconstruct-scan-lines observacao.json \
  --raster pagina.png \
  --font-family "Fonte candidata" \
  --font-size-pt 10.5 > pagina.typ

cargo run --bin decalque -- evaluate-scan-lines observacao.json \
  --raster pagina.png \
  --output-pdf candidato.pdf \
  --font-family "Fonte candidata" \
  --font-size-pt 10.5 \
  --granularity line \
  --horizontal-tolerance-pt 0.5 \
  --baseline-tolerance-pt 0.5 > iteracao.json

cargo run --bin decalque -- fit-scan-lines observacao.json \
  --raster pagina.png \
  --output-pdf vencedor.pdf \
  --font-family "Fonte candidata" \
  --font-size-pt 10 --font-size-pt 12 --font-size-pt 14 \
  --tracking-pt -0.15 --tracking-pt 0 \
  --horizontal-tolerance-pt 0.5 \
  --baseline-tolerance-pt 0.5 > busca.json

cargo run --bin decalque -- scan-observation observacao.json candidato.pdf \
  --raster pagina.png \
  --granularity word \
  --horizontal-tolerance-pt 0.5 \
  --baseline-tolerance-pt 0.5
```

O primeiro comando prova isoladamente o contrato e vincula SHA-256, tipo e dimensões aos bytes
do raster; ele não prova exatidão do OCR. O segundo comando emite isoladamente a fonte do primeiro
estágio de reconstrução: um fac-símile por linhas em pontos físicos, com hipótese de fonte
explícita e sem inventar baseline ou parágrafos. `evaluate-scan-lines` fecha uma iteração no
próprio Decalque: emite a mesma fonte, executa Typst com limites, lê o PDF de uma página em memória,
compara-o e só então publica o candidato sem sobrescrever um arquivo existente. Seu JSON registra
a hipótese, a versão do compilador, hashes dos dois artefatos e o relatório estrutural integral.
`fit-scan-lines` transforma essas medidas em parâmetros Typst por experimento reproduzível: fecha
uma grade explícita de corpos e trackings, compila e mede todas as hipóteses, seleciona pela pior
divergência estrutural quantizada e recompila o vencedor antes de publicar. Empate, cobertura
insuficiente ou suportes geométricos incomparáveis ficam explícitos e não produzem PDF.
O último comando permite comparar separadamente qualquer PDF candidato e repete obrigatoriamente
a vinculação antes de abri-lo. `ScanObservation`, `ReconstructionPlan` e
`DocumentGeometry` são contratos distintos.
Região, linha ou palavra OCR não viram `GlyphInstance` por aproximação. Claims incertas continuam
`Unknown`; ausência de transformação física não autoriza derivá-la do PDF candidato. Pixels e
formatos de fornecedor não entram em `01_core`.

As decisões completas estão na
[ADR 0004](00_nucleo/adr/0004-fronteira-observacao-scan.md), que protege a fronteira da
observação, e na
[ADR 0005](00_nucleo/adr/0005-reconstrucao-tipografica-no-produto.md), que define a reconstrução
como objetivo do produto, e na
[ADR 0006](00_nucleo/adr/0006-avaliacao-interna-do-candidato-typst.md), que define a compilação e
avaliação interna do candidato, e na
[ADR 0007](00_nucleo/adr/0007-busca-discreta-de-parametros-typst.md), que define a busca discreta
mensurável. Os contratos protegidos incluem:

- [Caso 2](00_nucleo/prompts/case2-scan-to-digital.md);
- [ScanObservation v1](00_nucleo/prompts/scan-observation-model.md);
- [adaptador JSON](00_nucleo/prompts/scan-observation-json-adapter.md);
- [comparador](00_nucleo/prompts/scan-observation-compare.md);
- [validador vinculado](00_nucleo/prompts/cli-scan-observation-validate.md);
- [CLI do Caso 2](00_nucleo/prompts/cli-scan-observation-compare.md);
- [reconstrução Typst por linhas](00_nucleo/prompts/scan-typst-line-reconstruction.md);
- [avaliação do candidato Typst](00_nucleo/prompts/scan-typst-candidate-evaluation.md);
- [busca de corpo e tracking](00_nucleo/prompts/scan-typst-parameter-search.md).

### Como as medidas chegam ao Typst

O `page_mapping` transforma coordenadas do raster em pontos físicos da página. Neste primeiro
estágio, a tradução é deliberadamente direta:

| Evidência ou hipótese | Plano | Typst |
|---|---|---|
| largura e altura do frame físico | `page.extent` | `page(width:, height:)` |
| canto superior esquerdo da bbox da linha | `target_bbox.x0/y0` | `place(dx:, dy:)` |
| família, corpo, peso, estilo e tracking | `TypographyHypothesis` explícita | `text(font:, size:, weight:, style:, tracking:)` |
| largura e altura da bbox | alvo geométrico | não são forçadas; o PDF compilado é medido |
| baseline observada | alvo opcional | não é inventada quando estiver `Unknown` |

Assim, medidas observadas não são promovidas silenciosamente a parâmetros tipográficos. O
comparador mede os resíduos do PDF compilado — início, fim, largura, posição vertical e baseline
quando disponível — e esses resíduos classificam uma grade fechada antes da execução, sem feedback
entre tentativas. A primeira otimização já pertence ao Decalque: `fit-scan-lines` faz busca discreta
exaustiva sobre o ciclo verificável, em vez de aplicar uma fórmula como “altura da bbox = tamanho da
fonte”.

Cada unidade `line` é composta em uma caixa finita com sua largura tipográfica natural. Se a
hipótese ultrapassar a bbox-alvo ou a página, ela continua sendo uma única linha e o excesso aparece
como resíduo; o emissor não quebra, comprime, recorta nem reduz o corpo para fazê-la caber. A
busca atual varia somente corpo e tracking para uma família, peso e estilo fixos. Ela não afirma
ter identificado a fonte e não converte baseline desconhecida em zero.

### Atestação estrutural da fonte candidata

No fluxo de reconstrução, OCR e modelos de visão continuam sendo observadores: suas caixas, textos
e níveis de confiança entram como evidência em `ScanObservation`, e lacunas permanecem `Unknown`.
`reconstruct-scan-lines` transforma essa evidência e uma hipótese tipográfica em código-fonte Typst
editável; o PDF compilado é um candidato que volta ao Decalque para inspeção estrutural. O candidato
não realimenta a observação nem o planejamento.

`attest-scan-font` avalia uma única hipótese de família, corpo, peso, estilo e tracking. As entradas
são a observação, o raster vinculado, o destino do PDF, a hipótese tipográfica, as tolerâncias de
comparação e, opcionalmente, limiares de confiança e o executável Typst:

```sh
cargo run --bin decalque -- attest-scan-font observacao.json \
  --raster pagina.png \
  --output-pdf candidato-atestado.pdf \
  --font-family "Fonte candidata" \
  --font-size-pt 10.5 \
  --font-weight regular \
  --font-style normal \
  --tracking-pt 0 \
  --horizontal-tolerance-pt 0.5 \
  --baseline-tolerance-pt 0.5 > atestacao.json
```

A granularidade é fixa em `line`; não existe opção para alterá-la. A fonte Typst estrita proíbe
fallback (`fallback: false`), e o PDF de uma página é auditado glifo a glifo: a sequência Unicode e
todos os recursos de fonte efetivamente usados precisam sustentar a família e a face solicitadas.
O relatório separa três estados:

- `preserved`: texto e recursos usados sustentam a hipótese. O Decalque serializa o relatório e
  publica o PDF compilado sem sobrescrever o destino.
- `violated`: há incompatibilidade demonstrável de texto, família ou variante. O comando emite o
  relatório JSON, mas não publica PDF.
- `unknown`: falta evidência necessária, como Unicode de um glifo, recurso inequívoco, `/BaseFont`
  ou nome classificável. O comando emite o relatório JSON, mas não publica PDF.

Falha operacional deixa stdout vazio e não cria um novo artefato. Essa verificação não identifica
a fonte histórica do scan, não pesquisa famílias e não
prova identidade visual ou binária da fonte. O contrato completo está na
[ADR 0008](00_nucleo/adr/0008-atestacao-estrutural-de-fonte-typst.md) e no
[prompt da atestação estrutural](00_nucleo/prompts/scan-typst-font-attestation.md).

## Arquitetura

| Diretório | Papel | Limite principal |
|---|---|---|
| `00_nucleo/` | Semente normativa | ADRs e prompts dos contratos Rust |
| `01_core/` | L1 — domínio puro | tipos, invariantes e comparação; sem I/O, JSON, OCR ou rede |
| `02_shell/` | L2 — política | argumentos, tolerâncias e apresentação |
| `03_infra/` | L3 — fronteiras | parsing de PDF/entradas, compilador limitado e publicação de artefatos |
| `04_wiring/` | L4 — composição | liga adaptadores, domínio, política e binários |
| `_lab/` | incubação | provas e mecanismos ainda não graduados para contratos suportados em `00`–`04` |

A implementação histórica de OCR e reconstrução foi preservada em
[`_lab/scan_observer/`](_lab/scan_observer/README.md), junto de suas specs experimentais e do
[registro de migração](_lab/scan_observer/MIGRATION.md). Suas capacidades pertencem ao objetivo
do Decalque, mas só passam a ser suportadas quando graduadas para contratos e adaptadores do
produto; seus formatos intermediários não são API.

## Estado atual

- O Caso 1 possui modelo de página e fonte, parser `ToUnicode`/CMap, intérprete de content stream,
  diagnósticos, adaptador `lopdf`, motor de comparação, políticas e CLI digital.
- A fronteira do Caso 2 está materializada em Rust: modelo e validação de domínio, adaptador JSON
  estrito, vinculação ao raster, planejamento geométrico por linhas, emissão Typst determinística,
  compilação limitada, materialização do PDF em memória, comparador tri-state e CLIs
  `validate-scan-observation`, `reconstruct-scan-lines`, `evaluate-scan-lines`, `fit-scan-lines` e
  `scan-observation`. O núcleo consome evidência e planeja; OCR e processos concretos permanecem
  em adaptadores compostos.
- O laboratório Python foi reorganizado sem apagar código, testes, fixtures, configurações ou
  aprendizado documental. Seu produtor conservador Paddle emite uma unidade `line` por detecção,
  sem fabricar palavras ou glifos.

## Desenvolvimento

Execute os gates do produto Rust na raiz:

```sh
cargo test --workspace
```

A suíte do produtor externo é independente:

```sh
python3 -m unittest discover -s _lab/scan_observer/tests -p 'test_*.py' -v
```

Novas capacidades do produto devem começar em `00_nucleo/` e respeitar a direção de dependência
`L4 -> L3/L2 -> L1`. Experimentos do produtor externo permanecem em `_lab/scan_observer/`.
