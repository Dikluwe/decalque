# Materialização scan → Typst → PDF

**Estado**: ativo
**ADR**: `00_nucleo/adr/0001-escopo-dois-casos-de-uso.md`,
`00_nucleo/adr/0003-convencoes-transversais.md`

## Obrigação

O Caso 2 deve produzir, a partir de uma observação inspecionável de uma página escaneada, uma
fonte Typst determinística e um PDF digital nativo. A mesma execução deve rasterizar referência
e candidato com o mesmo rasterizador e DPI e publicar métricas visuais auxiliares. A comparação
visual não entra em `01_core` e não substitui o veredito geométrico estrutural.

## Contrato de entrada

O manifesto JSON contém:

- `schema_version: 1`;
- `page`: `width_pt`, `height_pt`, `source_width_px`, `source_height_px`;
- `regions`, em ordem de pintura;
- região de texto: `kind: "text"`, `text`, `bbox_px`, `font.family`, `font.size_pt` e opções
  `weight`, `style`, `align`, `leading_pt`, `tracking_em`;
- região de imagem: `kind: "image"`, `path`, `bbox_px`.

Caixas usam `[x0, y0, x1, y1]`, origem superior esquerda. Campos obrigatórios ausentes,
dimensões não positivas, caixas degeneradas ou regiões desconhecidas são erro; não são
convertidos em sucesso parcial.

## Materialização

1. Converter pixels para pontos com escalas independentes derivadas da página.
2. Emitir página Typst sem margens e cada região com posicionamento absoluto.
3. Escapar texto e caminhos para não permitir que o conteúdo observado injete sintaxe Typst.
4. Imagens mantêm a caixa observada; texto recebe largura explícita para preservar reflow.
5. A fonte Typst e o manifesto permanecem como artefatos auditáveis.

## Execução integrada

O processo recebe manifesto, scan e diretório de saída; gera `.typ`, compila com `typst`,
rasteriza scan e candidato com `pdftoppm` no mesmo DPI e publica JSON com caminhos, dimensões e:

- `mean_absolute_error` em cinza normalizado;
- `different_pixel_ratio` para diferença absoluta maior que 16/255;
- `status: comparable` somente quando as imagens têm a mesma dimensão.

Falha de geração, compilação ou rasterização encerra com código 2 e não publica um relatório
final enganoso. Métrica visual baixa é evidência auxiliar, nunca prova isolada de identidade.

## Realimentação por projeções

O otimizador executa ciclos sobre o mesmo manifesto e mede a tinta acumulada por eixo:

- `horizontal_line_error`: erro absoluto médio entre projeções por linha Y, normalizado pela
  largura da página;
- `vertical_line_error`: erro absoluto médio entre projeções por coluna X, normalizado pela
  altura da página.

Os limites dos dois eixos são parâmetros obrigatoriamente publicados no relatório. Em cada
ciclo, a correlação das projeções estima `shift_x_px` e `shift_y_px` dentro de um deslocamento
máximo configurável. O deslocamento é convertido para o espaço de pixels do manifesto e aplicado
a todas as caixas ainda ajustáveis. Cada ciclo preserva manifesto, PDF, raster, métricas e ajuste.

O estado final é:

- `converged`: ambos os erros são menores ou iguais aos limites;
- `not_converged`: esgotou ciclos, não existe ajuste diferente de zero, ou duas revisões
  consecutivas não reduziram nenhum dos dois erros.

`not_converged` nunca é promovido a sucesso. A versão inicial ajusta translação global; tamanho,
tracking e parâmetros por região permanecem para refinamentos posteriores e são explicitados
como limitações no relatório.

### Zonas e espaçamento interno

A projeção global não basta para páginas compostas. O modo zonal deve segmentar sequências
contíguas de linhas com tinta, unir intervalos separados por no máximo `zone_merge_gap_px` e
descrever cada zona por `top`, `bottom`, `center`, `height` e intervalo horizontal de tinta.
Zonas de referência e candidato são emparelhadas em ordem vertical somente quando têm a mesma
cardinalidade; caso contrário, o ciclo fica `zones_unknown` e não inventa correspondência.

Para cada par, publicar `delta_x_px`, `delta_y_px`, `height_ratio` e, entre pares consecutivos,
`reference_gap_px`, `candidate_gap_px` e `gap_delta_px`. O ajuste zonal move separadamente a
região do manifesto associada por ordem de pintura. Convergência zonal exige todos os centros
dentro de `zone_position_threshold_px`, todos os espaçamentos dentro de
`zone_gap_threshold_px` e os erros globais por eixo dentro dos limites já definidos.

O `height_ratio` realimenta o tamanho da fonte (e sua entrelinha explícita) ou a dimensão da
imagem, limitado a ±15% por ciclo para evitar instabilidade. A família e o peso não são trocados
automaticamente. Região e zona com cardinalidades incompatíveis deixam o ajuste zonal `unknown`,
permitindo fallback global apenas quando solicitado explicitamente.

## Critérios de verificação

1. Um processo real materializa manifesto com texto e imagem, compila e gera PDF de texto nativo.
2. Texto contendo `#`, colchetes, barras e aspas não executa código Typst.
3. Caixa em pixels converte para a posição e dimensão esperadas em pontos.
4. Comando externo que falha produz código 2 e não produz JSON de sucesso.
5. Comparação de raster idêntico dá erro médio zero; dimensões distintas ficam `not_comparable`.
6. Referência deslocada converge por ciclos e registra redução nos erros horizontal e vertical.
7. Limite impossível ou estagnação termina como `not_converged`, sem loop ilimitado.
8. Uma página com translação global correta e espaçamentos internos alterados é corrigida apenas
   pelo modo zonal; o histórico expõe a redução dos `gap_delta_px`.

## Limites atuais

O manifesto é a fronteira humana/máquina: OCR, classificação, estimativa de fonte e recortes
podem preenchê-lo, mas inferências ambíguas devem continuar explícitas. A primeira materialização
real usa a capa do livro TRIZ como caso de aceitação; ela não generaliza por si só para o livro.
