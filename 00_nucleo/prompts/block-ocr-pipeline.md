# Pipeline OCR programavel por blocos

## Intencao

Permitir que o Decalque experimente cadeias diferentes de observacao de pagina sem
acoplar a ordem dos algoritmos ao codigo. Uma cadeia e um grafo aciclico de blocos
tipados. Cada bloco consome artefatos nomeados, produz artefatos nomeados e registra
duracao, parametros e estado no manifesto da execucao.

## Contrato observavel

- A configuracao JSON declara `version`, `inputs` e uma lista `blocks`.
- Referencias usam `input:<nome>` ou `<bloco>.<porta>`.
- A ordem textual dos blocos nao determina a execucao; dependencias determinam.
- Ciclos, referencias inexistentes, identificadores repetidos e tipos desconhecidos
  falham antes de executar qualquer bloco.
- Cada bloco escreve apenas dentro de seu diretorio de execucao.
- Uma falha interrompe descendentes e nao e convertida em sucesso parcial.
- O manifesto final registra a cadeia resolvida e todos os artefatos produzidos.
- `--dry-run` valida e mostra a ordem sem chamar OCR nem criar resultados de blocos.

## Blocos iniciais

- `normalize-format`: ajuste nao destrutivo do canvas a um formato fisico conhecido.
- `ovis-layout`: observacao semantica da pagina bruta pelo OvisOCR2.
- `normalize-page`: correcao de esquadro com transformacao registrada.
- `transform-regions`: projecao das caixas Ovis para a imagem normalizada.
- `segment-ink`: observacao geometrica local independente do OCR.

O resultado de um observador e evidencia, nao verdade. Ausencia de caixas textuais no
Ovis permanece observavel e nao deve ser preenchida silenciosamente.
