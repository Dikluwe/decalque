# Prompt: composição editorial canônica em Typst

**Camada**: modelo editorial e adaptador externo do Caso 2
**Depende de**: `editorial-region-classification.md`, `zone-typst-materialization.md`

## Obrigação

Converter regiões observadas numa página em blocos editoriais canônicos sem perder a geometria
das linhas físicas. O manifesto separa a função editorial (`role`) da estratégia de composição
(`text`, `reserved_visual` ou `unknown`) e a fonte Typst usa uma biblioteca comum de componentes.

As funções mínimas são `heading`, `body_paragraph`, `epigraph`, `verse_or_title_list`,
`block_quote`, `biographical_note`, `figure_caption`, `page_number`, `visual` e `unknown`.
Funções especializadas descobertas posteriormente podem ser preservadas sem serem silenciosamente
rebaixadas a corpo de texto.

## Observáveis

- tamanho físico e lado da página;
- caixa de cada bloco e caixa de cada linha física;
- texto, estilo, peso e ordem de leitura;
- espaços anteriores e posteriores observados, sem introduzir espaço editorial arbitrário;
- caixas de figuras e tabelas, mesmo quando o conteúdo ainda não foi reconstruído;
- associação entre legenda e visual por identificadores explícitos;
- PDF pesquisável para todos os blocos textuais materializados.

## Política de desconhecido

Região textual sem papel editorial observado torna-se `unknown`, continua visível no manifesto e
não é convertida implicitamente em parágrafo. Uma região visual sem arquivo raster torna-se uma
reserva geométrica rotulada; sua ausência não autoriza o texto vizinho a ocupar a caixa.

## Critérios de aceitação

1. Linhas físicas não sofrem refluxo nem mudança de ordem.
2. O gerador importa uma biblioteca editorial comum, em vez de codificar estilos por página.
3. Figura ou tabela mantém sua caixa integral com ou sem conteúdo disponível.
4. Blocos contíguos preservam o intervalo vertical observado entre suas caixas.
5. O manifesto e o PDF são reproduzíveis a partir da página classificada e da imagem original.
