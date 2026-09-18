# Prompt: materialização Typst a partir de zonas confirmadas

**Camada**: adaptador externo do Caso 2
**Depende de**: `page-zone-consensus.md`, `scan-typst-materialization.md`

## Obrigação

Converter uma página marcada em um manifesto Typst comum. Cada linha física confirmada torna-se
uma região textual independente; o Typst não pode recalcular suas quebras. Regiões visuais são
recortadas da página e preservadas como imagens independentes.

## Observáveis

- dimensões físicas da página são preservadas;
- ordem, texto, início e fim horizontal de cada linha são preservados no manifesto;
- a escala horizontal é calculada pela largura observada, sem alterar a sequência textual;
- texto inclinado recebe estilo sintético explícito, sem alegar identificação exata da fonte;
- o PDF resultante permanece pesquisável;
- render, comparação absoluta e relatório são sempre produzidos.

## Política de desconhecido

Caixa degenerada, fonte ausente, linha sem texto ou escala extrema interrompe a materialização.
Regiões visuais podem permanecer raster; isso deve constar no relatório e não conta como texto
nativo reconstruído.
