# Prompt: geometria de palavras por segmentação de tinta

**Camada**: adaptador externo do Caso 2
**Depende de**: `paddle-line-geometry.md`

## Obrigação

Dentro de cada bbox de linha observada, binarizar os pixels e localizar intervalos horizontais
com tinta. Componentes separados por lacunas menores que um limiar relativo à altura da linha
pertencem à mesma palavra; lacunas iguais ou maiores separam palavras.

O texto reconhecido apenas nomeia segmentos geométricos já observados. A associação só é válida
quando a quantidade de palavras não vazias reconhecidas coincide com a quantidade de segmentos.
Tokens sem correspondência textual única não herdam a caixa.

## Observáveis

- caixas de palavra são limites mínimos da tinta do segmento, no espaço original da imagem;
- linha e pixels originais não são alterados;
- cada segmento registra limiar usado, método e confiança de associação separada;
- pontuação adjacente permanece no mesmo segmento quando não há lacuna de palavra;
- ordem horizontal é preservada; suporte RTL depende da ordem textual da linha.

## Política de desconhecido

Imagem ilegível, bbox inválida, ausência de tinta, contagem divergente, texto vazio ou associação
ambígua gera `word_geometry_status: unknown`. Não repartir a largura da linha proporcionalmente.
