# Prompt: classificação visual de peso e inclinação por linha

**Camada**: adaptador externo do Caso 2
**Depende de**: `page-zone-consensus.md`, `scan-typographic-profile.md`

## Obrigação

Classificar o estilo tipográfico dominante de uma região observada comparando a tinta de suas
linhas com renderizações do mesmo texto e geometria. Cada linha é evidência ruidosa, não uma
decisão tipográfica independente. A classe não pode ser inferida do tipo semântico da região.
Candidatos mínimos: light, regular, italic, bold e bold-italic.

## Observáveis

- todos os candidatos usam o mesmo texto e são normalizados para a mesma caixa de tinta;
- inclinação e peso têm resultados e confiança separados;
- o relatório preserva o erro de todos os candidatos, não apenas o vencedor;
- regiões agregam somente linhas observadas e registram a distribuição bruta de classes;
- todas as linhas de uma região recebem a decisão consolidada da região; não se alterna estilo
  linha a linha apenas porque os erros dos candidatos oscilaram;
- a evidência bruta de cada linha é preservada separadamente da decisão consolidada;
- tinta isolada que desloque uma caixa para fora da margem predominante da região é descartada
  somente quando existir um grande vazio entre ela e o início do texto; a caixa original e a
  justificativa geométrica são preservadas;
- depois de reparar linhas, a caixa textual da região é recalculada a partir das caixas limpas;
  a caixa agregada anterior também é preservada;
- a imagem original não é alterada.

## Política de desconhecido

Texto vazio, caixa inválida, tinta ausente, fonte ausente ou margem insuficiente entre candidatos
produz `status: unknown`. Estilo misto numa linha não deve ser apresentado como identificação de
cada trecho; o resultado é apenas o estilo dominante da linha.

Uma linha desconhecida isolada dentro de uma região que tenha consenso observado pode herdar a
classe regional com `status: inferred`. A causa original do desconhecido deve permanecer em
`line_evidence`; uma região sem nenhuma linha observada continua desconhecida.
