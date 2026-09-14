# Prompt: comparação scan→digital por palavra

**Camada**: adaptador externo do Caso 2
**Depende de**: `scan-word-geometry.md`, `scan-pixel-to-point.md`, `candidate-font-evidence.md`

## Obrigação

Comparar segmentos de palavra observados no scan com palavras reconstruídas dos glifos do PDF
candidato. Somente correspondência textual exata e única é comparável. Medir em X o início, fim
e largura, e em Y a baseline observada contra a posição dos glifos candidatos. A tolerância é o
máximo entre valor absoluto e fracção do tamanho da fonte candidata. A comparação vertical exige
confiança mínima explícita da estimativa de baseline.

Uma linha do scan cujas palavras correspondem a mais de uma linha candidata é `violated` por
reflow. Tipografia permanece `unknown`: a fonte declarada pelo candidato não prova a fonte dos
pixels.

## Vereditos

- `preserved`: correspondência única, os três deltas X e o delta de baseline estão dentro da tolerância;
- `violated`: correspondência única com delta X ou baseline fora da tolerância, ou reflow demonstrado;
- `unknown`: texto ausente/repetido, geometria ausente ou evidência insuficiente.

Cobertura é sempre apresentada nos dois sentidos: segmentos comparáveis sobre o total do scan e
palavras candidatas correspondidas sobre o total candidato. Palavras candidatas sem segmento
correspondente são listadas como testemunhas; isso torna omissões do OCR observáveis sem classificá-las
falsamente como divergência geométrica. Cada violação inclui palavra, caixas, deltas e limiar.
`Unknown` ou cobertura incompleta nunca são convertidos em sucesso agregado.
