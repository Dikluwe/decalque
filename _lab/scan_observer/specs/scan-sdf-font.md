# Reconstrução de glifos por campos de distância

**Contexto:** `scan-derived-font.md`

## Obrigação

O construtor experimental de fontes deve poder combinar várias ocorrências alinhadas de um
caractere sem borrar suas bordas. Para isso, cada máscara de tinta é convertida em signed
distance field (SDF), os campos são agregados por mediana e a isolinha zero é convertida em
contornos TrueType.

## Contrato observável

1. A CLI oferece explicitamente os modos `scanline` e `sdf-contour`; `scanline` permanece
   compatível com o comportamento anterior.
2. `sdf-contour` exige SciPy, mas não OpenCV, scikit-image, rede ou modelo treinado.
3. Amostras são alinhadas pela linha de base e pelo `left_bearing_px` antes da agregação.
4. Uma ocorrência aberrante não pode dominar duas ou mais ocorrências concordantes do mesmo
   glifo; a agregação deve ser uma mediana de SDFs.
5. Contornos externos e contraformas internas permanecem fechados e renderizáveis no TTF.
6. O relatório identifica modo, quantidade de amostras, tinta agregada, contornos e pontos.
7. Ausência de isolinha válida para um caractere visível falha explicitamente, sem publicar
   uma fonte aparentemente válida.
8. A saída continua preservando avanço, baseline, cobertura Unicode e compatibilidade com
   renderização por FreeType/Pillow.

## Limites

- Este módulo não reconhece nem segmenta caracteres.
- Métricas e kerning continuam sendo observações separadas da geometria do contorno.
- A suavização não autoriza inventar serifas ou corrigir silenciosamente a forma impressa.
