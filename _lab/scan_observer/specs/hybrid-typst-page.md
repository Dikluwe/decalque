# Materialização de página híbrida em Typst/PDF

**Contexto:** planos produzidos por `hybrid-text-reconstruction.md`.

## Obrigação

Converter uma página escaneada e seus planos de linha em Typst e PDF: texto confiável deve ser
texto PDF visível; resíduos raster cobrem apenas trechos incompatíveis, mantendo por trás a
transcrição completa para pesquisa e cópia.

## Contrato observável

1. A CLI recebe manifesto de página, diretório de saída e caminho de fontes; produz `.typ` e PDF.
2. O tamanho PDF reproduz a caixa física informada, convertendo pixels para pontos.
3. A página original pode permanecer como fundo das zonas ainda não materializadas. Cada linha
   materializada apaga a tinta original em sua caixa antes de pintar texto e resíduos.
4. Tamanho, escala horizontal, tracking, posição e baseline vêm do plano de linha.
5. A transcrição completa de cada linha permanece como texto PDF, mesmo sob intervalos raster.
6. Cada intervalo raster usa exclusivamente o recorte registrado no plano e sua caixa observada.
7. Texto extraído do PDF contém as linhas na ordem do manifesto; imagem renderizada tem as mesmas
   dimensões da referência no DPI declarado.
8. Fonte ausente, plano inconsistente, caixa degenerada, compilação ou rasterização falha encerram
   sem relatório de sucesso.

## Limites

- A primeira versão usa a página escaneada como fundo para zonas não materializadas.
- O afinamento morfológico do protótipo raster não tem equivalente vetorial direto em Typst; peso
  visual é registrado como aproximação até existir uma transformação de contorno apropriada.
