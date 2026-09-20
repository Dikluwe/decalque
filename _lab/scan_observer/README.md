# Laboratório `scan_observer`

Este diretório preserva os experimentos Python de OCR, reconstrução, tipografia e
materialização anteriormente mantidos em `05_scan`. Ele não é uma camada Tekt nem integra o
produto Rust do Decalque.

A responsabilidade deste laboratório termina na produção de evidência externa. Para consumo
pelo Decalque, um produtor deve emitir `decalque.scan-observation` versão 1, conforme
`00_nucleo/prompts/scan-observation-model.md`. Formatos de fornecedor e os JSONs históricos
descritos abaixo são experimentais: não constituem contrato do produto. O PDF candidato não
pode participar da observação nem da calibração do raster de origem.

As especificações experimentais ficam em `specs/`; o histórico da mudança física e dos antigos
cabeçalhos de quinta camada está em `MIGRATION.md`.

## Ambiente Python

O ambiente é local à máquina, mas fica fora do repositório. Todos os comandos deste documento
partem da raiz do Decalque. Para usar outro diretório externo, defina `SCAN_OBSERVER_VENV` antes
destes comandos.

```sh
export SCAN_OBSERVER_VENV="${SCAN_OBSERVER_VENV:-../.venvs/decalque-scan-observer}"
export SCAN_OBSERVER_PYTHON="$SCAN_OBSERVER_VENV/bin/python"
python3 -m venv "$SCAN_OBSERVER_VENV"
"$SCAN_OBSERVER_PYTHON" -m pip install -r _lab/scan_observer/requirements.txt
```

## Produtor contratual Paddle → `ScanObservation v1`

O primeiro produtor conforme usa diretamente as linhas observadas pelo PaddleOCR clássico. Ele
preserva texto e score de reconhecimento, publica o polígono da linha, deriva a bbox do envelope
desse polígono e deixa baseline e mapeamento físico como `Unknown`. Não cria palavras, glifos nem
usa o PDF candidato.

```sh
"$SCAN_OBSERVER_PYTHON" \
  _lab/scan_observer/paddle_scan_observation.py pagina.png \
  --output pagina.scan-observation.json \
  --page-index 0 \
  --run-id livro-p0001 \
  --lang pt

CARGO_TARGET_DIR=/dev/shm/decalque-target cargo run --bin decalque -- \
  validate-scan-observation pagina.scan-observation.json \
  --raster pagina.png
```

O validador Rust confere o contrato estrito e a identidade do raster (SHA-256, tipo e dimensões).
Ele não afirma que o OCR está correto. A obrigação completa do produtor está em
`specs/scan-observation-exporter.md`.

## Adaptador PaddleOCR-VL + LM Studio

## Cadeias programaveis por blocos

`block_ocr_pipeline.py` executa experimentos OCR como grafos declarativos. O exemplo
`config/ovis-first-page.json` normaliza o formato fisico sem reamostrar, observa a pagina
com OvisOCR2, normaliza o esquadro, projeta as caixas visuais e segmenta a tinta localmente.
Valide a cadeia sem executar:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/block_ocr_pipeline.py _lab/scan_observer/config/ovis-first-page.json \
  --output output/block-runs/altov-011 --dry-run
```

Remova `--dry-run` para executar. Cada bloco recebe um diretorio proprio e a raiz da
execucao ganha um `manifest.json` com ordem, duracao e caminhos dos artefatos.

`ovis-layout` mantém `response.txt` e também uma sequência lossless em `regions.json`, que
preserva na ordem tanto texto livre quanto tags visuais. Essa saída continua sidecar
experimental: Markdown livre do Ovis não é promovido silenciosamente a claims v1.

Este adaptador usa o detector de layout do pipeline PaddleOCR-VL e delega o reconhecimento
visual ao modelo `paddleocr-vl` carregado no LM Studio. A saída v2 é JSON com regiões, linhas,
tokens e hipóteses tipográficas; mensagens do fornecedor são enviadas para stderr.

O detector atual fornece geometria de região. Linhas e tokens derivados da transcrição são
emitidos com geometria `null`, e a fonte fica `unknown`, até outro estágio apresentar evidência
visual. Isso impede que a precisão aparente do formato seja maior que a do fornecedor.

## Preparação do provedor

```sh
lms server start -p 1234
```

Carregue no LM Studio um modelo identificado como `paddleocr-vl`. Na primeira execução, o
PaddleOCR baixa os modelos locais de detecção de layout.

## Execução

```sh
"$SCAN_OBSERVER_PYTHON" \
  _lab/scan_observer/paddle_lmstudio_adapter.py documento.png
```

Também são aceitos PDFs. Use `--base-url`, `--model` e `--device` para substituir os padrões.
O processo retorna código 2 quando faltam dependências ou quando o provedor falha.

## Geometria real de linhas

O detector clássico produz polígonos observados para cada linha. Salve essa saída e associe-a
ao JSON do VLM:

```sh
"$SCAN_OBSERVER_PYTHON" \
  _lab/scan_observer/paddle_line_detector.py documento.png > lines.json
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/line_geometry_matcher.py scan.json lines.json > scan-with-lines.json
```

O detector roda em CPU com MKL-DNN desativado por compatibilidade com Paddle 3.3.1. As caixas
de palavra estimadas pelo Paddle não são solicitadas. Tokens apenas recebem
`line_geometry_ref` quando aparecem em uma única linha detectada; sua caixa continua `null`.

## Geometria de palavras pelos pixels

Depois da associação de linhas, segmente as lacunas reais de tinta na imagem:

```sh
"$SCAN_OBSERVER_PYTHON" \
  _lab/scan_observer/word_geometry_detector.py scan-with-lines.json documento.png > scan-with-words.json
```

O estágio só aceita uma linha quando a quantidade de segmentos observados coincide com a
quantidade de palavras reconhecidas. Ele não usa a subdivisão proporcional do Paddle. Tokens
recebem caixa apenas em correspondência textual exata e única; todos os segmentos permanecem
disponíveis em `word_segments`, mesmo quando o VLM diverge.

## Conversão para pontos PDF

> **Experimento legado:** este estágio usa dimensões do candidato e, portanto, não participa do
> produtor contratual nem da calibração oficial de `ScanObservation v1`.

O catálogo v2 inclui o tamanho visual da página candidata. Depois de associar as linhas,
converta a geometria observada de pixels para pontos:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/coordinate_transform.py scan-with-words.json candidate-fonts.json > scan-points.json
```

A transformação só é criada quando a proporção da imagem e a do PDF diferem no máximo 0,5%.
Ela preserva os campos em pixels e acrescenta `bbox_pt`/`polygon_pt`. Proporção divergente,
crop desconhecido ou dimensões inválidas deixam a transformação e as coordenadas em pontos
como `unknown`/`null`.

## Perfil tipográfico observado

Com a geometria em pontos, derive medidas conservadoras do envelope de tinta:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/typographic_profile.py scan-points.json > scan-profile.json
```

O perfil registra densidade, centroide e projeções normalizadas da tinta, além de altura-x apenas
em palavras sem acentos compostas por letras de altura-x,
altura ascendente quando o texto contém ascendentes e profundidade descendente quando contém
descendentes. Medidas sem sustentação ficam `null`; o nome da fonte candidata não é usado como
prova visual.

O perfil raster do candidato também pode ser executado isoladamente:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/candidate_raster_profile.py scan-profile.json candidate-fonts.json candidato.pdf > candidate-profile.json
```

O corpus controlado de fontes difíceis compila e compara PDFs reais, incluindo Liberation Serif
como troca visualmente semelhante e Bitstream Vera Serif como controle de tinta idêntica:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/typography_corpus.py
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/layout_corpus.py
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/ocr_error_corpus.py
```

O JSON resultante inclui os vereditos por palavra e o `mutation_score` das trocas de família,
peso, estilo, largura, tamanho, tracking, espaço entre palavras e baseline, tanto na referência
limpa quanto sob baixa resolução, blur, ruído, compressão JPEG e rotação leve.
O segundo comando exercita PDFs multilinha e rejeita mudanças de leading e reflow.
O terceiro executa erros controlados de OCR e expõe tanto segmentos desconhecidos quanto palavras
do PDF candidato que ficaram sem correspondência.

## Evidência de fonte do PDF candidato

Depois de salvar a saída OCR em `scan.json`, exporte as fontes e glifos estruturais do PDF
candidato e enriqueça os tokens alinhados:

```sh
CARGO_TARGET_DIR=/dev/shm/decalque-target cargo run -q \
  --bin decalque-font-catalog -- candidato.pdf > candidate-fonts.json
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/candidate_font_matcher.py scan.json candidate-fonts.json > enriched.json
```

A família e o tamanho só são preenchidos quando o token inteiro coincide com glifos de uma
única fonte e tamanho. O valor descreve a fonte declarada pelo PDF candidato como hipótese;
não prova sozinho que os pixels do scan foram impressos com a mesma fonte.

Runs puramente RTL, como árabe isolado, são alinhados em ordem Unicode lógica. Texto bidi
misto e fontes sem `/BaseFont` (observado em emoji colorido) permanecem sem inferência segura.

## Comparação scan → digital por palavra

Com geometria em pontos e o catálogo candidato, gere o primeiro relatório comparativo:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/scan_word_compare.py scan-points.json candidate-fonts.json > comparison.json
```

O relatório compara início, fim, largura horizontal e baseline, inclui cobertura e testemunhas e
detecta reflow quando uma linha observada corresponde a várias linhas candidatas. O perfil
tipográfico observado é comparado com o mesmo perfil extraído de uma rasterização do PDF na
resolução do scan. O `typography_status` cobre essas métricas visuais, não prova identidade
absoluta da família da fonte; palavras sem medida homóloga permanecem `unknown`.
O campo `verdict` agrega conteúdo, geometria e tipografia com precedência conservadora:
`violated` domina, seguido de `unknown`; somente evidência completa pode produzir `preserved`.

## Execução integrada

## Reconstrução em Typst

## Descoberta de fontes no Google Fonts

Antes da consulta externa, classifique fontes já disponíveis no sistema via Fontconfig:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/local_font_discovery.py --evidence font-evidence.json \
  --pt-per-px 0.4 --threshold 0.70 --limit 30 > local-font-candidates.json
```

Para pesquisar somente um banco privado ou cache específico, informe um ou mais diretórios. A
presença de `--font-dir` desativa a enumeração automática do Fontconfig, mantendo o escopo
explícito:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/local_font_discovery.py --evidence font-evidence.json \
  --font-dir "$HOME/.local/share/decalque/fonts" \
  --font-dir output/google-fonts-cache \
  --threshold 0.70 > local-font-candidates.json
```

O comparador é offline, percorre `.ttf`, `.otf`, `.ttc` e `.otc` recursivamente, deduplica o
conteúdo por SHA-256 e preserva metadados declarados de licença e origem. `matched` significa que
o primeiro candidato atingiu o limiar; `fallback_required` autoriza o orquestrador a considerar
uma pesquisa externa, mas não realiza rede; `unknown` preserva ausência de candidato comparável.
`--pt-per-px` habilita a estimativa do corpo em pontos; por exemplo, uma rasterização a 180 DPI
usa `72 / 180 = 0.4 pt/px`. Para cada evidência, o relatório estima corpo em pixels/pontos,
tracking residual em `em` e escala horizontal sem tracking. As métricas internas preservam
`units_per_em` e avanço do `M` em `em`; a largura do glifo não é confundida com o quadratim.

## Páginas-mestras espelhadas do miolo

Depois de medir caixas de texto, linhas e paginação em algumas páginas representativas, consolide
dois perfis reutilizáveis — verso/esquerda e reto/direita:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/book_page_masters.py page-observations.json > book-page-masters.json
```

A posição horizontal da paginação é a evidência prioritária do lado da página. Se ela estiver
ausente ou central, usa-se primeiro a paridade do número impresso e só então a alternância física.
Assim, capa e frontispício não fazem o índice do PDF passar por número impresso. Cada mestre contém
medianas em pontos para margens interna/externa, coluna, topo, base, recuo, entrelinha e centro da
paginação; campos sem evidência continuam `null`. Páginas especiais, figuras e aberturas de capítulo
podem manter regiões próprias sobre o mestre sem provocar uma nova calibração do miolo.

O exemplo de miolo possui um ciclo próprio de realimentação raster. Ele varia corpo, corpo do
título, entrelinha, espaçamento entre parágrafos, largura da coluna, topo e recuo, preservando somente candidatos cujo objetivo
combinado diminui. A contagem de zonas do corpo impede que linhas sobrepostas pareçam melhores:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/book_spread_feedback.py ocr.json book-page-masters.json livro.pdf \
  --pages 13 14 --font Suranna --font-path output/google-fonts-cache \
  --output-dir output/pdf/book-feedback > output/pdf/book-feedback-report.json
```

Depois de estabilizar a página, decomponha a diferença por linha e palavra:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/line_diff_analyzer.py reference.png candidate.png \
  --text-file recognized-lines.txt > line-diff.json
```

O alinhamento aceita cardinalidades diferentes e registra `reflow` em vez de deslocar
silenciosamente as associações. Pares observados são classificados como posição,
tracking/largura, tamanho/peso ou diferença localizada. Uma palavra só entra em
`ocr_review_queue` quando a segmentação de tinta e a quantidade de palavras reconhecidas
coincidem; o relatório pede revisão, mas nunca altera o OCR automaticamente. Ornamentos e
zonas sem associação textual continuam explícitos.

## Fonte derivada do scan

O construtor experimental recebe recortes de glifos já associados explicitamente a caracteres,
alinha as ocorrências pelo baseline, consolida amostras repetidas e gera uma TTF:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/scan_font_builder.py glyph-samples.json output/book-derived.ttf \
  > output/book-derived-report.json
```

O manifesto v1 declara `family`, `style`, `metrics.ascender_px`, `metrics.descender_px` e
`samples`. Cada amostra contém `char`, `path`, `baseline_px`, `advance_px` e, opcionalmente,
`left_bearing_px`. Somente rótulos Unicode unitários são aceitos: sequências ambíguas como `rn`
não são promovidas a um glifo. A implementação atual vetoriza tiras horizontais da tinta,
preserva avanços e ainda não deriva kerning. Ela é adequada para validar se métricas extraídas do
livro reduzem reflow antes de investir em contornos suavizados.

Quando houver ocorrências alinhadas por linha de base, o modo SDF combina campos de distância
por mediana, rejeita uma ocorrência aberrante diante de duas concordantes e extrai contornos
subpixel preservando contraformas:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/scan_font_builder.py glyph-samples.json output/book-derived-sdf.ttf \
  --vectorization sdf-contour > output/book-derived-sdf-report.json
```

O relatório registra tinta agregada, quantidade de contornos e pontos por glifo. O modo não
reconhece caracteres nem inventa métricas; amostras sem interior/exterior ou sem isolinha fechada
falham explicitamente. `--vectorization scanline` permanece como baseline e fallback.

Com uma chave da Google Fonts Developer API, classifique variantes do catálogo contra um recorte
de uma única linha. A API fornece o catálogo; o Decalque baixa uma lista limitada e mede a forma
renderizada localmente:

```sh
export GOOGLE_FONTS_API_KEY='...'
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/google_fonts_discovery.py innovation.png \
  --text Innovation --category serif --variant 700 --limit 30 > font-candidates.json
```

A chave não é persistida no relatório. A ausência da chave falha explicitamente, e fontes sem a
variante pedida ou arquivos ilegíveis aparecem em `rejected`.

Para combinar glifos especialmente discriminatórios e permitir variantes diferentes por zona:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/google_fonts_discovery.py --evidence font-evidence.json \
  --category serif --limit 30 > font-candidates.json
```

Cada evidência define texto, peso, dimensões (`family`, `weight`, `style`) e variantes aceitas.
O relatório v2 separa os três escores e preserva a contribuição de cada recorte.

O objetivo principal do Caso 2 inclui materializar uma página digital nativa e fechar o ciclo de
comparação. Um manifesto inspecionável descreve página, textos, fontes, caixas e imagens; o
pipeline gera Typst, compila PDF e rasteriza referência e candidato no mesmo DPI:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/typst_reconstruction_pipeline.py page-manifest.json scan.pdf \
  --source-page 1 --output-dir output/pdf/page-001 > output/pdf/page-001/report.json
```

As métricas raster são auxiliares e ficam fora de `01_core`; o PDF gerado continua disponível
para a comparação estrutural do Decalque. Falhas e campos ambíguos não são promovidos a paridade.

Depois da classificação editorial, a página também pode ser organizada por componentes comuns.
O compositor preserva cada quebra física, mantém `unknown` explícito e reserva integralmente as
caixas de figuras e tabelas; quando há imagem-fonte, o recorte visual ocupa essa mesma reserva:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/editorial_typst_composer.py editorial/page-018.json page-018.png \
  --font /caminho/para/fonte.ttf --page-number 18 \
  --output-dir output/pdf/editorial/page-018
```

O diretório contém `manifest.json`, a biblioteca reutilizável `editorial-blocks.typ`, a página
Typst, o PDF pesquisável, o render e a diferença raster. `page.side` distingue automaticamente
páginas esquerdas e direitas pela paridade informada.

Para um livro inteiro, o executor associa `page-NNN.json` a `page-NNN.png`, trabalha em paralelo,
retoma páginas que ainda não têm relatório completo, mede cada etapa e une os PDFs:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/editorial_book_composer.py editorial/ rendered-pages/ \
  --font /caminho/para/fonte.ttf --output-dir output/pdf/editorial-book \
  --workers 8 --resume --merge
```

`book-report.json` separa falhas de geração de páginas apenas sinalizadas para revisão textual.
Uma divergência textual não descarta um PDF válido nem impede a montagem do livro.

Perfis tipográficos documentais seguem outro ciclo: primeiro a página é colocada no esquadro,
depois métricas de páginas representativas são agregadas e o perfil recebe estado `frozen` e um
hash. A auditoria posterior apenas cria marcações; não reajusta linhas nem permite escala
horizontal de glifos. Uma amostra pode ser produzida com:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/typography_profile_trial.py livro.pdf \
  --pages 10,11,12,13,14 --width-pt 461.18 --height-pt 675.75 \
  --output-dir output/pdf/typography-trial
```

Para fechar o ciclo com limites definidos por eixo:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/typst_feedback_optimizer.py page-manifest.json scan.pdf \
  --output-dir output/pdf/page-001-feedback \
  --horizontal-threshold 0.002 --vertical-threshold 0.002 --max-cycles 8 \
  > output/pdf/page-001-feedback/report.json
```

Cada ciclo preserva manifesto, PDF e métricas. O modo padrão segmenta zonas horizontais de tinta,
mede centros, alturas e lacunas e move cada região separadamente. `--feedback-mode global` mantém
a correção por correlação da página inteira como fallback explícito. Cardinalidade zonal ambígua,
estagnação, dimensões incompatíveis ou orçamento esgotado terminam como `not_converged`.

Para comparar diretamente os dois modelos visuais do LM Studio e materializar as regiões de
imagem indicadas pelo OvisOCR2:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/dual_lmstudio_ocr.py pagina.png \
  --asset-dir output/pagina-assets > output/pagina-dual-ocr.json
```

O OvisOCR2 é preservado como testemunha de layout, o PaddleOCR-VL como testemunha de texto, e
`comparison` explicita concordância ou divergência. Cada marcador Ovis
`images/bbox_X0_Y0_X1_Y1` (coordenadas normalizadas de 0 a 1000) gera um PNG recortado e uma
caixa em pixels. Nenhuma das transcrições é sobrescrita por consenso implícito.
Os recortes recebem margem superior/lateral de 1,5% da página e margem inferior menor para
compensar caixas apertadas sem invadir legendas (`--crop-padding` altera a base). Símbolos recorrentes, inclusive espelhados, podem
ser agrupados pela forma da tinta:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/visual_symbol_catalog.py output/paginas/page-*.json > symbols.json
```

Se o VLM omitir o marcador de uma ocorrência pequena, use um recorte confirmado como molde;
o buscador compara componentes de tinta em escala normalizada e considera reflexão horizontal:

```sh
"$SCAN_OBSERVER_PYTHON" _lab/scan_observer/recurring_symbol_finder.py folha.png pagina-*.png \
  --asset-dir output/folha-ocorrencias > occurrences.json
```

Com o LM Studio ativo e o catálogo compilado, todo o fluxo de uma página pode ser executado
por um único comando. A comparação tipográfica também requer `pdftocairo` disponível no PATH:

```sh
CARGO_TARGET_DIR=/dev/shm/decalque-target cargo build --bin decalque-font-catalog
"$SCAN_OBSERVER_PYTHON" \
  _lab/scan_observer/scan_compare_pipeline.py documento.png candidato.pdf \
  --catalog-bin /dev/shm/decalque-target/debug/decalque-font-catalog > result.json
```

`result.json` contém tanto a observação enriquecida quanto o relatório comparativo. Logs dos
provedores continuam em stderr, deixando stdout como JSON puro.

## Livro escaneado em sequência

Para um PDF escaneado multipágina, rasterize e compare uma página por vez, mantendo o índice da
página candidata sincronizado:

```sh
"$SCAN_OBSERVER_PYTHON" \
  _lab/scan_observer/scan_compare_document.py livro-scan.pdf livro-digital.pdf \
  --catalog-bin /dev/shm/decalque-target/debug/decalque-font-catalog > result.json
```

O comando usa 200 DPI por padrão (`--dpi` altera), mantém somente a página corrente em diretório
temporário e escreve progresso em stderr. Se qualquer página falhar, termina com código 2 sem
publicar JSON parcial. O resultado completo preserva a ordem das páginas e agrega seus vereditos.

## Testes do adaptador

```sh
"$SCAN_OBSERVER_PYTHON" -m unittest discover -s _lab/scan_observer/tests -p 'test_*.py' -v
```
