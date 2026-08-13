# ADR 0003 — Convenções transversais do projeto

Estado: aceito
Data: 2026-08-12

Tipo: documento normativo
Aplica-se a: prompts de `00_nucleo/prompts/` e código gerado em `01_core`, `02_shell`,
`03_infra` e `04_wiring`, conforme camada.

Este documento não é um prompt de geração de componente.

Ele não deve ser usado como valor de `@prompt` em arquivos de código de componente.

Cada componente deve apontar para seu prompt específico em `00_nucleo/prompts/`.

Se uma regra deste documento precisar ser usada em um prompt, o prompt deve citar este
documento no contexto. Ele não substitui o campo `@prompt`.

## Precedência

As regras deste documento são transversais.

Um prompt específico pode detalhar uma regra deste documento.

Um prompt específico não pode contrariar este documento sem registro explícito.

Exceções a este documento exigem:

1. justificativa explícita no prompt;
2. registro no histórico do prompt;
3. ADR próprio quando a exceção alterar convenção arquitetural.

Exceções ao tipo numérico canônico exigem ADR próprio.

## Tipo numérico canônico

O tipo numérico padrão para quantidades reais contínuas é `f64`.

Aplica-se a:

- coordenadas;
- largura;
- altura;
- avanço de glifo;
- matrizes;
- deltas;
- `size_delta`;
- `user_unit`;
- retângulos;
- transformações de texto.

Não se aplica a:

- identificadores;
- índices;
- contadores;
- números de página;
- códigos de glifo;
- rotações inteiras;
- valores enum.

Não usar `f32` em `01_core`.

## Conversão de tipos

Entrada externa:

```text
biblioteca externa -> números brutos
03_infra -> converte para f64
01_core -> usa f64
```

Saída externa:

- `02_shell` converte de `f64` para outro formato quando a saída for apresentação.
- `03_infra` converte de `f64` para outro formato quando a saída for persistência ou
  serialização externa.
- `01_core` não muda para `f32` para satisfazer consumo externo.

## Motivos da decisão de `f64`

Decisão registrada em 2026-08-12:

- o núcleo já usa `f64`;
- matrizes de texto acumulam multiplicações e somas;
- PDF pode ter coordenadas grandes ou transformações encadeadas;
- não há requisito de tempo real que justifique `f32`.

## Linhagem

Todo arquivo de código e teste gerado a partir de um prompt deve conter o cabeçalho
`Crystalline Lineage`.

Formato (renderização em Rust):

```rust
//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/<nome-do-prompt>.md
//! @layer L<n>
//! @updated YYYY-MM-DD
```

O cabeçalho usa `//!` (doc-comment **interno**, de módulo), não `/** */`.

Motivo, verificado em 2026-08-12: em Rust, `/** */` é doc-comment **externo** — documenta o
item que se segue, não o arquivo. Aplicado no topo de um arquivo, o cabeçalho de linhagem
passa a ser a documentação pública do primeiro item do arquivo, e aparece como tal no
`cargo doc`. `//!` é a forma que a linguagem tem para cabeçalho de arquivo/módulo.

Esta é uma regra de renderização, não de conteúdo: os três campos continuam obrigatórios.

Regras:

- `@prompt` deve apontar para um prompt existente.
- `@layer` deve indicar a camada do arquivo gerado.
- `@updated` deve usar data no formato `YYYY-MM-DD`.
- Não adicionar `@prompt` apontando para prompt inexistente.
- Se o prompt não existe, criar o prompt antes de gerar o código.
- Linhagem falsa é pior que ausência de linhagem.

Regra para código gerado de spec arquivada:

Quando um arquivo implementa uma spec que já foi movida para `00_nucleo/prompts/_deprecated/`
e ainda não foi revisto conforme a spec sucessora, `@prompt` aponta para o **caminho
arquivado** — a spec que o código de fato implementa — e o cabeçalho registra a sucessora em
nota de estado.

Apontar para a spec sucessora antes da revisão do código seria linhagem falsa: declararia
conformidade com um contrato que o arquivo não cumpre. O caminho arquivado é verdadeiro e
verificável; a nota de estado é o que sinaliza a dívida.

Exceção — arquivos sem prompt de origem:

- Código em `_lab` não exige linhagem obrigatória.
- Código em `_lab` só recebe linhagem quando migrado para o sistema principal por meio de
  novo prompt.
- Arquivos de agregação e composição que nenhum prompt gera — `lib.rs`, `mod.rs`, `main.rs`
  e stubs de camada ainda não implementada — não levam `@prompt`. Não há prompt de origem
  para apontar, e inventar um seria linhagem falsa. Estes arquivos levam apenas doc-comment
  descritivo. Quando um prompt passar a especificar um deles, o arquivo entra na regra geral.

Este documento não deve ser usado como `@prompt` de componente.

Exemplo incorreto (ADR como `@prompt`):

```rust
//! Crystalline Lineage
//! @prompt 00_nucleo/adr/0003-convencoes-transversais.md
//! @layer L1
//! @updated 2026-08-12
```

Exemplo incorreto (doc-comment externo — documenta o item seguinte, não o arquivo):

```rust
/**
 * Crystalline Lineage
 * @prompt 00_nucleo/prompts/page-geometry-model.md
 * @layer L1
 * @updated 2026-08-12
 */
```

Exemplo correto:

```rust
//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/page-geometry-model.md
//! @layer L1
//! @updated 2026-08-12
```

## Verificação

Critérios para validar este documento:

1. Nenhum arquivo de `01_core` usa `f32` para quantidade real contínua.
2. Nenhum componente usa este ADR como `@prompt`.
3. Todo arquivo gerado a partir de um prompt, fora de `_lab`, possui cabeçalho de linhagem
   válido em `//!`, com os três campos.
4. Todo caminho em `@prompt` corresponde a um arquivo existente em `00_nucleo/prompts/`
   (incluindo `_deprecated/`).
5. Nenhum arquivo de agregação/composição (`lib.rs`, `mod.rs`, `main.rs`, stubs) declara
   `@prompt`.
6. Conversões de entrada para `f64` ocorrem em `03_infra`.
7. Exceções a este documento possuem registro em prompt e ADR quando aplicável.

## Histórico de Revisões

| Data | Motivo | Arquivos afetados |
|---|---|---|
| 2026-08-12 | Criação da regra de `f64` e da regra de linhagem (como `prompts/convencoes.md`). | `00_nucleo/prompts/convencoes.md` |
| 2026-08-12 | Revisão do dono: escopo transversal, natureza normativa declarada, exceção para `_lab`, regra de precedência com ADR para exceções, limitação de `f64` a quantidades reais contínuas, conversão de saída por camada; movido de `prompts/` para ADR por não gerar código nem testes. | `00_nucleo/adr/0003-convencoes-transversais.md` |
| 2026-08-12 | Renderização do cabeçalho passa de `/** */` para `//!` (verificado: `/** */` é doc-comment externo e publica a linhagem como documentação do primeiro item no `cargo doc`) — os três campos continuam obrigatórios; regra explícita para código gerado de spec arquivada (`@prompt` aponta para `_deprecated/` + nota de estado, em vez de apontar para a sucessora não implementada); exceção para arquivos de agregação/composição sem prompt de origem; critérios de verificação 3–5 reescritos. Aplicação: linhagem de `01_core` saneada — 3 `@prompt` órfãos (`glyph_instance.rs`, `cartesian_origin.rs`, `page_geometry.rs`) reapontados para `_deprecated/`. | `00_nucleo/adr/0003-convencoes-transversais.md`, `01_core/src/**` |
