# Realimentação de páginas-mestras de livro

Otimize uma reconstrução rasterizada de páginas de miolo por busca coordenada determinística.
Cada ciclo deve avaliar alterações reais de corpo, entrelinha, espaçamento entre parágrafos,
largura da coluna, topo e recuo,
e deve comparar altura, largura e densidade da zona de título para escolher seu corpo e peso
independentemente do corpo do texto,
preservar o melhor candidato observado e registrar parâmetros, objetivo e métricas por página.
Sucesso exige redução objetiva em relação ao ciclo inicial; ausência de melhora é `not_converged`,
nunca sucesso implícito. O PDF final deve corresponder exatamente ao melhor ciclo registrado.
