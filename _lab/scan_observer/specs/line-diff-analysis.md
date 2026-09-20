# Análise localizada de diferenças scan para digital

Alinhe linhas de tinta da referência e do candidato sem exigir cardinalidade igual. Preserve
inserções e remoções como reflow explícito. Para cada par, meça posição, baseline aproximada,
largura, altura, lacuna seguinte e diferença raster normalizada; classifique deslocamento,
tracking/largura, tamanho/peso, quebra/reflow ou diferença localizada. Segmente palavras somente
quando a geometria da tinta sustentar a cardinalidade textual. Diferenças localizadas devem gerar
itens `review_required`, nunca correção automática do OCR. Ornamentos e regiões sem associação
textual permanecem `non_text_or_unassigned`.
