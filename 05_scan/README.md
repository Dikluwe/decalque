# Adaptador PaddleOCR-VL + LM Studio

Este adaptador usa o detector de layout do pipeline PaddleOCR-VL e delega o reconhecimento
visual ao modelo `paddleocr-vl` carregado no LM Studio. A saída é JSON com texto e geometria;
mensagens do fornecedor são enviadas para stderr.

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

## Testes do adaptador

```sh
python3 -m unittest discover -s 05_scan/tests -p 'test_*.py' -v
```
