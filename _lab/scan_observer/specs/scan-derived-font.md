# Fonte derivada de tinta observada

Construa uma fonte experimental a partir de amostras raster associadas explicitamente a
caracteres Unicode. Consolide múltiplas ocorrências no mesmo sistema de baseline, preserve
avanços observados e gere uma TTF válida. Amostras ausentes nunca devem ser inventadas como se
fossem observadas; podem ser representadas apenas por fallback declarado em etapa posterior.
O relatório deve registrar cobertura, quantidade de amostras, escala e métricas. A geração não
autoriza corrigir OCR nem inferir silenciosamente a identidade de caracteres ambíguos.
