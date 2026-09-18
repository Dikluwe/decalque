# Prompt: segmentação editorial de livro por capítulos

**Camada**: organização de artefatos do Caso 2
**Depende de**: `editorial-typst-composition.md`

## Obrigação

Dividir um livro já materializado em intervalos editoriais explícitos, contíguos e sem
sobreposição. Cada segmento deve gerar PDF independente e relatório com páginas físicas,
título, identificador estável e páginas que ainda exigem revisão.

## Observáveis

- a união dos intervalos cobre exatamente todas as páginas declaradas;
- nenhum número de página pertence a dois capítulos;
- limites têm evidência textual registrada (`heading`, `toc` ou decisão editorial);
- cada PDF tem a quantidade de páginas do intervalo correspondente;
- erros e alertas do livro são atribuídos ao respectivo capítulo;
- uma correção pode regenerar apenas um capítulo sem recompor os demais.

## Política de desconhecido

Um limite sem cabeçalho ou decisão editorial explícita permanece inválido. Material preliminar,
aberturas de seção, apêndices e matéria posterior podem formar segmentos próprios; não devem ser
forçados para dentro do capítulo vizinho apenas para reduzir a quantidade de arquivos.
