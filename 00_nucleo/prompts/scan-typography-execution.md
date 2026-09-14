# Prompt: testes de execução da tipografia scan→digital

**Camada**: fronteira externa do Caso 2
**Depende de**: `scan-typographic-profile.md`

## Obrigação

Exercitar os programas como processos reais, usando arquivos JSON e PDF em disco. O corpus deve
cobrir rasterização bem-sucedida de uma fixture real, falha explícita do renderizador e emissão de
testemunha para mutação da forma da tinta. Não substituir `pdftocairo`, parsing de argumentos,
serialização ou códigos de saída por chamadas internas.

O corpus controlado usa a mesma frase, página e tamanho em DejaVu Serif como referência e troca
somente a família para Bitstream Vera Serif, Liberation Serif, DejaVu Sans e DejaVu Sans Mono.
Bitstream Vera Serif é o controle de equivalência visual: seus glifos latinos desta amostra têm
tinta idêntica e devem permanecer `preserved`, sem alegação de identidade nominal. Liberation
Serif é o ataque de família visualmente semelhante, mas distinguível. Cada mutação observável é
rejeitada quando ao menos uma palavra produz testemunha tipográfica `violated`. O gate exige
`mutation_score = 1.0`.

O segundo estrato mantém DejaVu Serif e altera isoladamente peso para negrito, estilo para
itálico ou aplica escala horizontal de 75%. As três mutações também integram o denominador do
`mutation_score`.

O terceiro estrato ataca diferenças sutis de composição: tamanho de 19,5 pt ou 20,5 pt contra
20 pt, tracking de 0,25 pt, espaço entre palavras acrescido de 3 pt e baseline deslocada em
2,5 pt. Cada caso deve ser rejeitado por geometria ou perfil tipográfico observado, inclusive
quando a forma normalizada dos glifos permanece semelhante.

Um corpus multilinha separado mantém texto, fonte e tamanho, alterando o leading em 2,5 pt ou
forçando uma linha observada a ocupar várias linhas candidatas. O primeiro ataque deve expor
delta de baseline; o segundo deve produzir testemunha de reflow com mais de um `candidate_line_id`.

## Robustez do scan

Repetir a matriz com a referência reduzida a 50% e reamostrada, blur gaussiano de 0,8 px, ruído
gaussiano determinístico de desvio 4, compressão JPEG com qualidade 45 e rotação de 0,35 grau.
Cada controle degradado deve continuar `preserved` e cada estrato deve manter
`mutation_score = 1.0`. Degradação que elimine evidência produz `unknown`, nunca preservação
implícita.

## Vereditos

- execução válida termina com código 0 e JSON parseável em stdout;
- erro de entrada/renderização termina com código 2 e diagnóstico em stderr;
- mutação tipográfica produz `typography_status: violated` com distância e limiar;
- ausência de evidência nunca é aceita como preservação.
