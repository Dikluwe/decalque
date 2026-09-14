# Adaptador PaddleOCR-VL + LM Studio

Este adaptador usa o detector de layout do pipeline PaddleOCR-VL e delega o reconhecimento
visual ao modelo `paddleocr-vl` carregado no LM Studio. A saída v2 é JSON com regiões, linhas,
tokens e hipóteses tipográficas; mensagens do fornecedor são enviadas para stderr.

O detector atual fornece geometria de região. Linhas e tokens derivados da transcrição são
emitidos com geometria `null`, e a fonte fica `unknown`, até outro estágio apresentar evidência
visual. Isso impede que a precisão aparente do formato seja maior que a do fornecedor.

## Preparação

```sh
python3 -m venv .venv-paddle
.venv-paddle/bin/pip install -r 05_scan/requirements.txt
lms server start -p 1234
```

Carregue no LM Studio um modelo identificado como `paddleocr-vl`. Na primeira execução, o
PaddleOCR baixa os modelos locais de detecção de layout.

## Execução

```sh
.venv-paddle/bin/python 05_scan/paddle_lmstudio_adapter.py documento.png
```

Também são aceitos PDFs. Use `--base-url`, `--model` e `--device` para substituir os padrões.
O processo retorna código 2 quando faltam dependências ou quando o provedor falha.

## Geometria real de linhas

O detector clássico produz polígonos observados para cada linha. Salve essa saída e associe-a
ao JSON do VLM:

```sh
.venv-paddle/bin/python 05_scan/paddle_line_detector.py documento.png > lines.json
python3 05_scan/line_geometry_matcher.py scan.json lines.json > scan-with-lines.json
```

O detector roda em CPU com MKL-DNN desativado por compatibilidade com Paddle 3.3.1. As caixas
de palavra estimadas pelo Paddle não são solicitadas. Tokens apenas recebem
`line_geometry_ref` quando aparecem em uma única linha detectada; sua caixa continua `null`.

## Geometria de palavras pelos pixels

Depois da associação de linhas, segmente as lacunas reais de tinta na imagem:

```sh
.venv-paddle/bin/python 05_scan/word_geometry_detector.py scan-with-lines.json documento.png > scan-with-words.json
```

O estágio só aceita uma linha quando a quantidade de segmentos observados coincide com a
quantidade de palavras reconhecidas. Ele não usa a subdivisão proporcional do Paddle. Tokens
recebem caixa apenas em correspondência textual exata e única; todos os segmentos permanecem
disponíveis em `word_segments`, mesmo quando o VLM diverge.

## Conversão para pontos PDF

O catálogo v2 inclui o tamanho visual da página candidata. Depois de associar as linhas,
converta a geometria observada de pixels para pontos:

```sh
python3 05_scan/coordinate_transform.py scan-with-words.json candidate-fonts.json > scan-points.json
```

A transformação só é criada quando a proporção da imagem e a do PDF diferem no máximo 0,5%.
Ela preserva os campos em pixels e acrescenta `bbox_pt`/`polygon_pt`. Proporção divergente,
crop desconhecido ou dimensões inválidas deixam a transformação e as coordenadas em pontos
como `unknown`/`null`.

## Perfil tipográfico observado

Com a geometria em pontos, derive medidas conservadoras do envelope de tinta:

```sh
python3 05_scan/typographic_profile.py scan-points.json > scan-profile.json
```

O perfil registra densidade, centroide e projeções normalizadas da tinta, além de altura-x apenas
em palavras sem acentos compostas por letras de altura-x,
altura ascendente quando o texto contém ascendentes e profundidade descendente quando contém
descendentes. Medidas sem sustentação ficam `null`; o nome da fonte candidata não é usado como
prova visual.

O perfil raster do candidato também pode ser executado isoladamente:

```sh
python3 05_scan/candidate_raster_profile.py scan-profile.json candidate-fonts.json candidato.pdf > candidate-profile.json
```

O corpus controlado de fontes difíceis compila e compara PDFs reais em serifada, sans e mono:

```sh
python3 05_scan/typography_corpus.py
```

O JSON resultante inclui os vereditos por palavra e o `mutation_score` das trocas de família,
peso, estilo e largura, tanto na referência limpa quanto sob baixa resolução, blur, ruído,
compressão JPEG e rotação leve.

## Evidência de fonte do PDF candidato

Depois de salvar a saída OCR em `scan.json`, exporte as fontes e glifos estruturais do PDF
candidato e enriqueça os tokens alinhados:

```sh
cargo run -q --bin decalque-font-catalog -- candidato.pdf > candidate-fonts.json
python3 05_scan/candidate_font_matcher.py scan.json candidate-fonts.json > enriched.json
```

A família e o tamanho só são preenchidos quando o token inteiro coincide com glifos de uma
única fonte e tamanho. O valor descreve a fonte declarada pelo PDF candidato como hipótese;
não prova sozinho que os pixels do scan foram impressos com a mesma fonte.

Runs puramente RTL, como árabe isolado, são alinhados em ordem Unicode lógica. Texto bidi
misto e fontes sem `/BaseFont` (observado em emoji colorido) permanecem sem inferência segura.

## Comparação scan → digital por palavra

Com geometria em pontos e o catálogo candidato, gere o primeiro relatório comparativo:

```sh
python3 05_scan/scan_word_compare.py scan-points.json candidate-fonts.json > comparison.json
```

O relatório compara início, fim, largura horizontal e baseline, inclui cobertura e testemunhas e
detecta reflow quando uma linha observada corresponde a várias linhas candidatas. O perfil
tipográfico observado é comparado com o mesmo perfil extraído de uma rasterização do PDF na
resolução do scan. O `typography_status` cobre essas métricas visuais, não prova identidade
absoluta da família da fonte; palavras sem medida homóloga permanecem `unknown`.

## Execução integrada

Com o LM Studio ativo e o catálogo compilado, todo o fluxo de uma página pode ser executado
por um único comando. A comparação tipográfica também requer `pdftocairo` disponível no PATH:

```sh
cargo build --bin decalque-font-catalog
.venv-paddle/bin/python 05_scan/scan_compare_pipeline.py documento.png candidato.pdf > result.json
```

`result.json` contém tanto a observação enriquecida quanto o relatório comparativo. Logs dos
provedores continuam em stderr, deixando stdout como JSON puro.

## Testes do adaptador

```sh
python3 -m unittest discover -s 05_scan/tests -p 'test_*.py' -v
```
