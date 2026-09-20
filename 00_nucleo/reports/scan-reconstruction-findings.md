# Conclusões preservadas da reconstrução de scans

Este resumo conserva somente resultados operacionais úteis obtidos durante a implementação.

## Configuração utilizada

A execução Typst do produto usa:

- fonte de entrada: até 16 MiB;
- PDF em stdout: até 128 MiB;
- stderr por invocação: até 1 MiB;
- versão em stdout: até 64 KiB;
- tempo por invocação: 30 segundos;
- compilação direta, sem shell, com
  `compile --format pdf --creation-timestamp 0 - -`;
- publicação em caminho novo, sem sobrescrever.

A emissão estrutural de fonte usa `fallback: false`.

## Conclusões

- Uma linha observada deve permanecer uma linha física de largura natural. Forçar a largura da bbox
  no container Typst permitia wrapping e escondia o resíduo que o comparador deveria medir.
- O PDF candidato deve ser analisado em memória e conter exatamente uma página antes da comparação.
- Filhos do processo Typst podem herdar stdout/stderr após o processo direto encerrar. Sem encerrar
  e colher a árvore inteira, um timeout de 30 s chegou a retornar após aproximadamente 35,01 s ou
  poderia bloquear indefinidamente.
- Com a correção de encerramento da árvore, um ensaio com limite de 120 ms retornou em cerca de
  130 ms e não deixou processo descendente.
- Na fixture de referência da avaliação, a fonte emitida teve 416 bytes e o PDF aproximadamente
  5,6 KiB. Esses números descrevem somente a fixture; não são metas de desempenho.
- Ausência de baseline, mapeamento ou recurso de fonte permanece `unknown`; não é convertida em
  zero nem em correspondência.

Os testes funcionais que protegem essas conclusões ficam nas fronteiras de reconstrução, execução
Typst, comparação e verificação estrutural.
