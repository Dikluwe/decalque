# Reconstrução híbrida de texto nativo e resíduos raster

**Contexto:** reconstrução fiel de páginas escaneadas sem exigir a criação de uma fonte nova.

## Obrigação

Dada uma linha escaneada, sua transcrição e uma fonte local semelhante, ajustar a renderização
tipográfica à tinta e produzir um plano híbrido: caracteres confiáveis permanecem texto nativo;
intervalos incompatíveis preservam recortes da imagem original.

## Contrato observável

1. A CLI recebe imagem de linha, transcrição, baseline, fonte, diretório e limiar de confiança.
2. O ajuste estima tamanho, escala horizontal, tracking e posição usando a linha completa.
3. Cada ocorrência recebe erro geométrico local, cobertura de tinta, caixa e decisão `native` ou
   `raster`; espaços permanecem avanços nativos.
4. Decisões raster adjacentes são agrupadas em intervalos mínimos, sem alterar a ordem Unicode.
5. Todo intervalo raster é recortado exclusivamente da entrada e registra o texto que substitui.
6. A saída contém plano JSON, renderização nativa, composição híbrida, mapa de resíduos e métricas
   comparáveis por distância Chamfer, projeções e tinta não explicada.
7. A composição híbrida não pode piorar a distância Chamfer em relação à renderização somente
   nativa; caso isso ocorra, o intervalo responsável não é aceito.
8. Entrada inválida, fonte ausente, linha sem tinta ou baseline impossível falha sem plano de
   sucesso.
9. Um ajuste opcional de peso pode afinar a tinta nativa sem alterar posições, avanços, escala ou
   caixas tipográficas; intensidade zero preserva exatamente a renderização original e o valor
   aplicado fica registrado no plano.

## Limites

- A primeira versão trabalha com uma linha horizontal e um único estilo.
- O plano é evidência para posterior materialização Typst/PDF; não promete que Typst suporte toda
  transformação por glifo sem agrupamentos adicionais.
- Resíduo raster é fallback explícito, não evidência tipográfica reutilizável.
