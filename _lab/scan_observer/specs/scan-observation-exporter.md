# Exportador externo `ScanObservation v1`

## Estatuto

Este contrato pertence ao laboratório `scan_observer`; ele não cria uma quinta camada Tekt.
O produtor Python observa um raster de uma página e termina ao emitir evidência no contrato
`decalque.scan-observation` versão 1. O Rust continua sendo a autoridade que valida o envelope,
vincula-o ao raster e compara a observação com o PDF digital.

**Depende de**: `00_nucleo/adr/0004-fronteira-observacao-scan.md` e
`00_nucleo/prompts/scan-observation-model.md`.

## Primeiro produtor conforme

O primeiro corte usa as linhas reais de `paddle_line_detector.py`. Ele não combina Paddle com
Ovis, não executa reconstrução editorial e não importa módulos `candidate_*`, Typst,
`coordinate_transform.py` ou comparadores históricos.

Interface de execução viva:

```text
python3 _lab/scan_observer/paddle_scan_observation.py <raster> \
  --output <observacao.json> \
  --page-index <inteiro-zero-based> \
  --run-id <id> \
  [--lang <idioma>] [--ocr-version <versao>] [--device <dispositivo>]
```

- o raster é exatamente o arquivo entregue ao Paddle e depois identificado em `source.raster`;
- `--page-index` e `--run-id` são obrigatórios e não têm default temporal;
- a execução aceita exatamente uma página de saída do fornecedor;
- não existe argumento de PDF candidato, catálogo de fontes, DPI do candidato, Typst ou
  normalização textual;
- erro do fornecedor, dependência ausente, envelope multipágina ou entrada inconsistente retorna
  código 2, deixa stdout sem JSON de sucesso e não cria/substitui o arquivo final;
- a escrita do resultado é atômica.

O construtor puro pode receber uma resposta Paddle congelada nos testes, mas esse atalho não é
uma opção pública capaz de alegar que outro raster foi observado.

## Identidade do raster

O exportador calcula sobre os bytes do arquivo realmente entregue ao OCR:

- SHA-256 hexadecimal minúsculo;
- media type suportado;
- largura e altura decodificadas;
- `artifact_id` determinístico derivado do digest.

Antes de publicar a identidade, o exportador deve forçar a decodificação integral dos pixels.
Inspecionar apenas cabeçalho, dimensões, marcadores, CRC ou metadados não basta. PNG com fluxo
IDAT truncado e PGM com menos amostras que o declarado falham de modo atômico, sem observação
parcial, ainda que o contêiner externo pareça consistente. Essa barreira local usa a semântica do
decodificador Pillow e não transforma o produtor em autoridade: reparos permissivos de JPEG que
o fornecedor aceite ainda precisam ser recusados pela validação Rust vinculada, que trata
warnings do codec como erro.

O frame v1 é `scan-px`, `px`, origem superior esquerda, eixos para direita/baixo e coordenadas em
bordas de pixels. Pré-processamento, quando existir, produz um novo raster: é esse novo arquivo,
e não a entrada anterior, que deve ser observado, hasheado e declarado.

`page_mapping` fica `Unknown/not-observed` neste primeiro corte. Dimensões do candidato nunca
podem calibrá-lo.

## Tradução conservadora das linhas Paddle

Cada entrada devolvida pelo detector produz exatamente uma unidade top-level `line`, inclusive
quando o texto está ausente, vazio, tem pontuação pura ou baixa confiança. IDs seguem a ordem
estável `line-000001`, `line-000002`, sem caminho, relógio ou `hash()` do processo.

- `reading_order` é `Known/inferred` a partir da sequência do fornecedor, com confiança
  `Unknown/not-observed`;
- texto não vazio é preservado byte semanticamente, sem trim, casefold, colapso de espaço,
  normalização Unicode ou remoção de pontuação, como `Known/inferred`;
- `rec_score` finito em `[0,1]` é confiança apenas do texto; ausente ou inválido deixa a confiança
  textual `Unknown`, nunca `0` ou `1` por default;
- texto ausente/vazio é `Unknown/not-observed`; a unidade geométrica não é descartada;
- `span_in_parent` é `Unknown/not-observed`, pois a linha não tem pai;
- nenhum token, palavra, glifo ou `GlyphInstance` é criado;
- baseline é `Unknown/not-observed`.

O polígono válido do detector sustenta `geometry.polygon: Known/inferred`. A bbox publicada é o
envelope matemático desse polígono e usa uma proveniência de derivação própria; `rec_boxes` não
vence nem corrige o polígono. O score de reconhecimento não é confiança geométrica.

Coordenada não finita, fora do raster, polígono degenerado ou envelope impossível não é
clampado, arredondado nem invertido. A geometria afetada se torna `Unknown/invalid`, com
diagnóstico inspecionável, ou a execução falha se o envelope do fornecedor estiver truncado. Uma
linha continua materializada mesmo quando texto e geometria são independentes e parcialmente
desconhecidos.

## Proveniência e determinismo

Claims conhecidas referenciam registros distintos para reconhecimento textual, detecção
geométrica e derivação da bbox. Toda raiz referencia o `artifact_id` do raster. O hash de
parâmetros é calculado sobre JSON canônico contendo a etapa e a configuração efetiva relevante;
alterar idioma, versão, dispositivo ou método altera o hash correspondente, enquanto reordenar
chaves não altera.

Para raster, resposta congelada e `run_id` iguais, o JSON v1 é byte a byte determinístico em
processos, diretórios, locales, fusos e `PYTHONHASHSEED` diferentes. O artefato não inclui caminho
absoluto, duração, timestamp, logs do fornecedor ou segredos. Esse determinismo cobre o
exportador, não a execução viva do modelo.

## Ovis como evidência auxiliar

`ovis-layout` continua experimental e não é promovido silenciosamente a produtor v1: sua saída
livre em Markdown não oferece gramática estável para associar texto e geometria. Ainda assim, o
bloco deve preservar em `regions.json` uma sequência lossless e ordenada de segmentos textuais e
tags visuais, além do SHA-256 da resposta bruta. Concatenar os segmentos deve reproduzir
exatamente `response.txt` antes da quebra final acrescentada pelo armazenamento.

As regiões visuais legadas podem permanecer para compatibilidade. A ausência de tag não significa
ausência de texto, e o sidecar Ovis não cria claims v1, palavras, caixas textuais ou consenso com
Paddle.

## Critérios de verificação

1. O mesmo raster e resposta Paddle congelada produzem bytes idênticos com `run_id` fixo.
2. Alterar um byte do raster altera digest e `artifact_id`; dimensões declaradas são as reais.
3. Nenhuma opção, importação ou provenance do produtor referencia candidato, fonte ou Typst.
4. Texto com espaços, pontuação, `Straße`, combinação Unicode e emoji permanece exato.
5. Linha vazia, score ausente/NaN e baixa confiança não desaparecem nem recebem defaults.
6. Polígono rotacionado produz bbox pelo envelope; bbox contraditória do fornecedor não vence.
7. Coordenadas inválidas não são clampadas e aparecem como `Unknown`/diagnóstico ou erro fatal.
8. Uma linha com várias palavras continua uma única unidade `line`, sem geometria infantil.
9. Todas as claims conhecidas têm evidência e toda raiz de provenance referencia o raster.
10. Falha total não publica observação vazia de sucesso nem sobrescreve resultado anterior.
11. A saída do construtor é aceita pelo parser Rust e pelo comando de validação vinculado.
12. O sidecar Ovis reconstrói integralmente a resposta, em ordem, sem elevá-la a claim v1.
13. Raster que o decodificador local não consegue carregar integralmente é rejeitado antes de
    qualquer identidade ser publicada; toda saída ainda passa pela validação Rust estrita.
