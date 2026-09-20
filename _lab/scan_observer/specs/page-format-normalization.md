# Normalizacao fisica do formato de pagina

## Intencao

Antes de inferir margens e tipografia, comparar o tamanho fisico observado da pagina com
formatos conhecidos. Quando houver correspondencia forte, ajustar somente o canvas ao
formato nominal. O conteudo nao pode ser escalado nem deformado.

## Contrato observavel

- A entrada contem uma imagem e suas dimensoes fisicas observadas em pontos.
- Formatos ISO, norte-americanos e editoriais sao comparados nas duas orientacoes.
- A correspondencia usa o maior desvio relativo entre largura e altura.
- Acima da tolerancia, o resultado e `unknown` e a imagem permanece inalterada.
- Abaixo da tolerancia, o tamanho nominal determina o canvas em pixels usando a resolucao
  observada mediana dos dois eixos.
- O ajuste usa exclusivamente corte e margem; nunca reamostragem.
- Em pagina direita, a borda interna esquerda e preservada e a diferenca horizontal fica
  na borda externa direita. Em pagina esquerda ocorre o espelhamento. Sem lateralidade,
  o ajuste e centralizado.
- O relatorio registra formato, confianca, dimensoes, DPI, cortes/margens e translacao da
  origem para que caixas posteriores possam ser projetadas.

## Politica de incerteza

Dimensoes ausentes, tolerancia excedida ou DPI inconsistente produzem `unknown`; nunca se
escolhe silenciosamente o formato apenas pela proporcao da imagem.
