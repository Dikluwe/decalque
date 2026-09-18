# Prompt: âncoras de palavras nas margens da coluna

**Camada**: adaptador externo do Caso 2
**Depende de**: `scan-word-geometry.md`

## Obrigação

Para cada linha de texto observada, analisar separadamente faixas nos lados esquerdo e direito.
O primeiro intervalo de tinta até a primeira lacuna de palavra ancora a primeira palavra; o
último intervalo após a última lacuna ancora a última palavra. Essas duas âncoras definem os
extremos horizontais observados da caixa de texto mesmo quando o miolo não pode ser segmentado.

O texto OCR somente nomeia a primeira e a última palavra já observadas. Não repartir a largura
da linha proporcionalmente nem usar métricas da fonte como observação de tinta.

## Observáveis

- `left` contém a primeira palavra, sua caixa mínima de tinta e a lacuna que a encerra;
- `right` contém a última palavra, sua caixa mínima de tinta e a lacuna que a antecede;
- `anchored_text_bbox` começa na tinta esquerda e termina na tinta direita da linha;
- o método funciona quando a segmentação integral de palavras permanece `unknown`;
- linhas de uma palavra produzem duas âncoras coincidentes;
- coordenadas são preservadas no espaço original da página.

## Política de desconhecido

Texto vazio, ausência de tinta, bbox inválida ou falta de uma lacuna qualificadora na faixa de
margem produz `margin_anchor_status: unknown`. Uma âncora ausente não é inventada a partir da
largura esperada da fonte.
