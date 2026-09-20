# Prompt: transformação de pixels do scan para pontos PDF

**Camada**: adaptador externo do Caso 2
**Depende de**: `00_nucleo/prompts/scan-observation-model.md`,
`00_nucleo/prompts/candidate-font-evidence.md`

## Obrigação

Transformar geometria observada em pixels YDown para pontos YDown do espaço visual da página
candidata. O catálogo deve fornecer dimensões visuais em pontos depois da rotação PDF. A página
do scan fornece dimensões da imagem processada.

A transformação é uma escala afim sem translação, independente em X e Y. Ela só é válida quando
dimensões são positivas e finitas e o erro relativo entre proporções é no máximo 0,5%. O recibo
registra escalas, erro de proporção, limiar e origem da evidência.

## Observáveis

- coordenadas originais em pixels não são alteradas;
- bbox e polígono em pontos são derivados apenas quando a transformação é válida;
- rotação usa o tamanho visual exportado pelo núcleo, sem nova heurística;
- a transformação alcança regiões, linhas detectadas e qualquer linha/token com geometria real;
- repetição produz os mesmos valores.

## Política de desconhecido

Dimensão ausente, zero, negativa, não finita ou proporção incompatível gera transformação
`unknown`; campos `bbox_pt` e `polygon_pt` permanecem `null`. Não corrigir crop, margem ou
rotação por ajuste implícito.
