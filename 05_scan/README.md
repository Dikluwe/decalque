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

## Testes do adaptador

```sh
python3 -m unittest discover -s 05_scan/tests -p 'test_*.py' -v
```
