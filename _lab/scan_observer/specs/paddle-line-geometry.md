# Prompt: geometria de linhas por PaddleOCR clássico

**Camada**: adaptador externo do Caso 2
**Depende de**: `00_nucleo/prompts/scan-observation-model.md`

## Obrigação

Executar a detecção e o reconhecimento clássico do PaddleOCR sobre a mesma imagem usada pelo
adaptador VLM e preservar polígonos reais de linhas. Associar cada linha a uma região de layout
somente quando seu centro estiver dentro da região e houver compatibilidade textual mínima.

`return_word_box` não deve ser usado como evidência geométrica: no PaddleOCR ele subdivide a
linha reconhecida por estimativa, não por nova observação dos pixels. Tokens podem referenciar
uma linha detectada quando sua ocorrência textual nessa linha for única, mas sua própria caixa
permanece `null`.

## Observáveis

- polígono, bbox, texto e confiança de reconhecimento da linha são preservados separadamente;
- confiança de associação é distinta da confiança OCR;
- linhas não associáveis ficam em `unassigned_lines`;
- associação não altera texto, quebra, span ou geometria preexistentes;
- uma única linha semântica não herda caixa quando duas linhas visuais a suportam.

## Política de desconhecido

Região ausente, incompatibilidade textual, token repetido ou associação múltipla não escolhe
um vencedor implícito. A geometria correspondente permanece ausente.
