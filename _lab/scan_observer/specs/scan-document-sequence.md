# Prompt: execução sequencial de documento scan→digital

**Camada**: fronteira externa do Caso 2
**Depende de**: `scan-word-compare.md`

## Obrigação

Comparar um PDF escaneado multipágina com o PDF digital candidato sem carregar todas as páginas
simultaneamente. Descobrir a quantidade de páginas do scan, rasterizar uma página por vez em
ordem crescente e executar o pipeline de página com o mesmo índice zero-based no candidato.

Arquivos rasterizados são temporários. Progresso e diagnósticos pertencem a stderr; stdout contém
somente um documento JSON completo. Falha de rasterização ou de uma página interrompe a sequência,
termina com código 2 e identifica a página humana one-based. Resultado parcial nunca é apresentado
como comparação completa.

## Saída

O JSON registra total de páginas, DPI, resultados na ordem original e um veredito agregado. Uma
página `violated` torna o documento `violated`; na ausência de violação, qualquer página `unknown`
torna o documento `unknown`; somente todas as páginas `preserved` produzem `preserved`.
