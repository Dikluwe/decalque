# Prompt: testes de execução da tipografia scan→digital

**Camada**: fronteira externa do Caso 2
**Depende de**: `scan-typographic-profile.md`

## Obrigação

Exercitar os programas como processos reais, usando arquivos JSON e PDF em disco. O corpus deve
cobrir rasterização bem-sucedida de uma fixture real, falha explícita do renderizador e emissão de
testemunha para mutação da forma da tinta. Não substituir `pdftocairo`, parsing de argumentos,
serialização ou códigos de saída por chamadas internas.

O corpus controlado usa a mesma frase, página e tamanho em DejaVu Serif como referência e troca
somente a família para DejaVu Sans e DejaVu Sans Mono. O controle deve ser `preserved`; cada
mutação é rejeitada quando ao menos uma palavra produz testemunha tipográfica `violated`. O gate
exige `mutation_score = 1.0`.

O segundo estrato mantém DejaVu Serif e altera isoladamente peso para negrito, estilo para
itálico ou aplica escala horizontal de 75%. As três mutações também integram o denominador do
`mutation_score`.

## Vereditos

- execução válida termina com código 0 e JSON parseável em stdout;
- erro de entrada/renderização termina com código 2 e diagnóstico em stderr;
- mutação tipográfica produz `typography_status: violated` com distância e limiar;
- ausência de evidência nunca é aceita como preservação.
