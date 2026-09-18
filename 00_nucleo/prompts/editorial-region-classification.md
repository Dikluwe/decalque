# Prompt: classificação editorial de regiões textuais

**Camada**: interpretação estrutural posterior à OCR e à tipografia
**Depende de**: `page-zone-consensus.md`, `line-font-style-classification.md`

## Obrigação

Classificar a função editorial de uma região textual usando primeiro evidências geométricas e
tipográficas consolidadas. As classes mínimas são `heading`, `body_paragraph`, `epigraph`,
`verse_or_title_list`, `block_quote`, `biographical_note`, `page_number`, `visual` e `unknown`.
`figure_caption` também deve ser emitido quando um bloco curto e destacado estiver associado à
geometria de uma figura.

## Observáveis

- número, largura e preenchimento das linhas;
- estabilidade da margem esquerda e variação dos comprimentos;
- posição vertical, recuo e separação dos blocos vizinhos;
- peso e inclinação consolidados da região;
- sinais textuais somente como evidência auxiliar, nunca como única causa da classe;
- relatório com características, regras acionadas e confiança.

## Restrições

- não classificar cada linha separadamente;
- não substituir o tipo geométrico/OCR original; preservá-lo como `source_region_type`;
- não chamar uma região de verso/lista apenas por estar em itálico;
- regiões sem linhas ou estilo suficiente permanecem `unknown`, exceto visuais observados.
