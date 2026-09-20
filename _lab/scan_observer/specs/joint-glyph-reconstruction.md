# Reconstrução conjunta de glifos por realimentação de linhas

**Contexto:** reconstrução experimental de uma fonte a partir de páginas escaneadas cuja
transcrição é conhecida.

## Obrigação

Dado um conjunto de imagens de linhas, suas transcrições, linhas de base e uma fonte semelhante
usada somente como guia geométrica, aprender um atlas raster compartilhado por caractere. O atlas
deve explicar linhas completas; recortes individuais são evidência provisória, não verdade.

## Contrato observável

1. A CLI recebe um corpus JSON, fonte-guia, diretório de saída e quantidade máxima de iterações.
2. O corpus contém ao menos duas linhas e separa explicitamente linhas de treino e validação.
3. A fonte-guia fornece posições, avanços e uma forma inicial para atribuição de tinta, mas seus
   contornos não podem aparecer como evidência no atlas produzido.
4. Ocorrências da mesma letra alimentam uma máscara canônica compartilhada, alinhada por baseline
   e origem tipográfica.
5. Pixels próximos de glifos vizinhos recebem responsabilidade probabilística; não são incluídos
   integralmente em todas as letras sobrepostas.
6. Cada iteração recompõe as linhas completas, mede distância simétrica entre tinta observada e
   recomposta e registra erro de treino e validação.
7. Uma atualização que piora validação não substitui o melhor atlas. A execução para por
   convergência ou pelo limite solicitado.
8. O relatório registra cobertura, quantidade de ocorrências, erros, iteração escolhida,
   parâmetros geométricos, tinta não explicada e limitações.
9. O diretório contém PNG canônico por glifo, reconstruções das linhas e manifesto diretamente
   conversível pelo construtor de fontes existente.
10. Entradas inválidas, fonte ausente, validação vazia e linhas sem tinta falham sem relatório de
    sucesso.

## Métrica

O critério principal é distância Chamfer simétrica entre máscaras de tinta. Diferença de projeções
horizontal e vertical e proporção de tinta não explicada aparecem como diagnósticos separados.
Fundo branco não pode dominar a pontuação.

## Limites

- A primeira versão trabalha com escrita horizontal e um único estilo/tamanho aproximadamente
  uniforme por corpus.
- OCR e classificação de estilo permanecem etapas anteriores.
- O atlas raster deve ser validado antes de qualquer alegação sobre qualidade vetorial.
