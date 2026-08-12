
# ADR 0002 — lopdf como backend de leitura estrutural de PDF em `03_infra`

Estado: aceito (revisado)  
Data: 2026-08-12  
Substitui: versão de 2026-08-11

Revisões posteriores:

- 2026-08-12 — `TextSpan` removido da fronteira `03_infra`→`01_core` (o átomo é `GlyphInstance`;
  agrupamento textual, se necessário, é `TextRun` derivado — não implementado); geometria passa
  a dois tipos (`PageBoxModel` bruto + `PageGeometry` resolvido); exemplos alterados para `f64`
  (tipo numérico canónico do núcleo); diagnóstico obrigatório ao detectar `Do` com Form XObject.
- 2026-08-12 — Fronteira sintaxe/semântica clarificada: `03_infra` decodifica as operações do
  content stream via `lopdf::content::Content::decode` e entrega `ContentOperation` (tipo de
  `01_core`); `01_core` interpreta semântica, nunca sintaxe de bytes. Modelo de fonte
  (larguras + decodificação de códigos de glifo) especificado em
  `00_nucleo/prompts/pdf-font-model.md` — `ToUnicode` não fornece larguras nem divisão da
  string em códigos.

## Contexto

`03_infra` precisa ler PDFs reais e fornecer dados estruturados para `01_core`.

Os dados necessários são:

- conteúdo de páginas;
- operadores de texto e posicionamento;
- geometria de página;
- recursos de fonte;
- streams `ToUnicode`;
- diagnóstico de páginas sem texto.

Os requisitos fixados são:

1. Não renderizar páginas.
2. Não gerar imagens.
3. Não reparar PDFs corrompidos.
4. Não escrever PDF.
5. Não mesclar, dividir ou reorganizar objetos.
6. Não descriptografar.
7. PDF criptografado deve falhar com erro claro.
8. O artefato final deve ser autocontido.
9. Bibliotecas dinâmicas externas devem ser evitadas.
10. O backend deve ser implementado em Rust, sem FFI para PDFium, MuPDF ou QPDF.

Definição de “Rust puro” para esta ADR:

- O backend principal não usa FFI para bibliotecas PDF externas.
- Dependências de crates são permitidas, mas devem ser auditadas.
- Se a restrição final for ausência total de código nativo em dependências transitivas, isso deve ser verificado antes da implementação.

## Candidatos

### lopdf

Biblioteca Rust para leitura de estrutura PDF.

Pontos relevantes:

- não exige biblioteca dinâmica externa;
- permite acesso a objetos, streams, páginas e recursos;
- pode decodificar streams com `FlateDecode`;
- não implementa extração completa de texto;
- não implementa parser de CMap/`ToUnicode`;
- não é rasterizador.

### PDFium

Motor PDF com foco em renderização.

Rejeitado porque:

- o projeto não precisa renderizar páginas;
- adiciona dependência externa;
- aumenta tamanho do artefato;
- parsing estrutural não é o foco principal.

### MuPDF

Motor PDF com foco em renderização rápida.

Rejeitado porque:

- o projeto não precisa renderizar páginas;
- adiciona dependência externa;
- a licença AGPL é incompatível com projetos proprietários sem licença comercial;
- parsing estrutural não é o foco principal.

### QPDF

Biblioteca C++ robusta para manipulação estrutural de PDF.

Rejeitado porque:

- exige FFI/C++;
- é mais útil para escrita, reparo, mesclagem e descriptografia;
- o projeto atual é somente leitor;
- conflita com o objetivo de binário/biblioteca autocontido em Rust.

## Decisão

Usar `lopdf` 0.44 como backend de leitura estrutural em `03_infra`.

O `lopdf` será responsável por:

- abrir o PDF;
- ler trailer, root e catálogo;
- localizar páginas;
- acessar recursos herdados de página;
- acessar `Contents`;
- decodificar streams;
- acessar dicionários de fonte;
- acessar streams `ToUnicode`;
- detectar criptografia;
- converter dados PDF para tipos de domínio definidos em `01_core`.

O `lopdf` não será responsável por:

- calcular geometria final de texto;
- interpretar regras de domínio do Decalque;
- decidir comportamento de relatório;
- implementar parser completo de CMap;
- renderizar páginas;
- executar OCR.

## Regra de camada

`01_core` não pode importar `lopdf`.

`03_infra` pode importar `lopdf`.

`03_infra` deve converter dados do `lopdf` para tipos de `01_core`.

Direção permitida:

```text
03_infra -> lopdf
03_infra -> 01_core
```

Direção proibida:

```text
01_core -> lopdf
```

Se `01_core` usar tipos como `lopdf::Document`, `lopdf::Object` ou `lopdf::Stream`, a separação arquitetural está violada.

A separação baseada em `DocumentGeometry`, registrada na ADR 0001, só é válida se `DocumentGeometry` for um tipo de `01_core`.

## Escopo de leitura estrutural

O backend deve ler:

- catálogo raiz;
- árvore de páginas;
- dicionário de página;
- `MediaBox`;
- `CropBox`;
- `Rotate`;
- `UserUnit`, se existir;
- recursos da página;
- dicionário `/Font`;
- `/Contents` como stream ou array de streams;
- streams de fonte;
- streams `ToUnicode`;
- XObjects relevantes para diagnóstico de página.

## Geometria de página

A geometria da página deve considerar:

```text
MediaBox
CropBox
Rotate
UserUnit
```

Regras:

1. Se `CropBox` existir, ele define a área visível.
2. Se `CropBox` não existir, `MediaBox` é usado.
3. `Rotate` deve ser considerado ao calcular coordenadas finais.
4. `UserUnit` pode ser tratado como fator de escala quando presente.
5. Se `UserUnit` não for necessário para o requisito atual, isso deve ser registrado explicitamente.

`01_core` deve possuir tipos próprios para geometria: `PageBoxModel` (bruto, produzido por
`03_infra`) e `PageGeometry` (resolvido, calculado por função pura `resolve_page_geometry`).

Exemplo:

```rust
pub struct PageBoxModel {
    pub media_box: Rect,
    pub crop_box: Option<Rect>,
    pub rotate: Option<i32>,
    pub user_unit: Option<f64>,
}

pub struct PageGeometry {
    pub width: f64,
    pub height: f64,
    pub rotation: PageRotation,
    pub user_unit: f64,
}
```

Tipo numérico canónico de `01_core` para geometria PDF: **`f64`** (coordenadas, dimensões,
matrizes, deltas, `user_unit`, transformações). `03_infra` converte números externos para
`f64` antes de entregar a `01_core`; `01_core` não muda para `f32` por consumo externo.

A rotação **não** troca `width`/`height` dentro de `PageGeometry` — o tamanho visual após
rotação, quando necessário, é função separada (`display_size`). Detalhes em
`00_nucleo/prompts/page-geometry-model.md`.

## Escopo de content stream

O processamento de content stream deve reconhecer operadores relevantes para texto e posicionamento.

Operadores mínimos:

```text
BT
ET
Tf
Tm
Td
TD
T*
TL
Tc
Tw
Tz
Ts
Tj
TJ
Do
q
Q
cm
```

Observações:

- `BT` e `ET` delimitam objetos de texto.
- `Tf` seleciona fonte e tamanho.
- `Tm` define matriz de texto.
- `Td`, `TD` e `T*` movem a posição de linha.
- `TL` define leading.
- `Tc` define espaçamento entre caracteres.
- `Tw` define espaçamento de palavra.
- `Tz` define escala horizontal.
- `Ts` define elevação de texto.
- `Tj` mostra uma string.
- `TJ` mostra array de strings e ajustes numéricos.
- `Do` invoca XObjects.
- `q` e `Q` salvam e restauram estado gráfico.
- `cm` altera matriz de transformação corrente.

Se algum desses operadores não for necessário para a primeira versão, ele deve ser listado como fora de escopo ou como diagnóstico não processado.

## Extração de texto

Extração de texto não é fornecida pronta pelo `lopdf`.

O projeto deve implementar:

- interpretação de operadores de texto;
- cálculo de posição de texto;
- resolução de fonte;
- mapeamento de códigos de glifo para Unicode;
- tratamento de ajustes de `TJ`;
- diagnóstico de texto não mapeado.

A lógica de domínio para texto e geometria deve ficar em `01_core` quando for pura.

`03_infra` deve fornecer dados brutos já decodificados.

Exemplo de responsabilidade:

```text
03_infra:
  lê PDF com lopdf
  descomprime streams
  localiza fontes
  localiza ToUnicode
  devolve bytes e metadados

01_core:
  interpreta CMap
  interpreta operadores
  calcula posições
  produz GlyphInstance e PageGeometry
```

Se agrupamento textual for necessário, ele é derivado como `TextRun`.
`TextRun` não é usado como entrada bruta de `03_infra`.

## Parser de ToUnicode / CMap

O `lopdf` não exporta parser de CMap.

O parser de `ToUnicode` será código próprio do projeto.

Formato mínimo a suportar:

```text
begincodespacerange
endcodespacerange
beginbfchar
endbfchar
beginbfrange
endbfrange
```

Regra de camada:

```text
A lógica de parsing de CMap deve ser pura.
Ela recebe bytes ou texto e devolve estrutura de mapeamento.
Portanto, deve pertencer a 01_core.
```

`03_infra` apenas obtém o stream `ToUnicode` e passa os dados para `01_core`.

Exemplo:

```rust
pub struct CmapMapping {
    pub mappings: Vec<CmapEntry>,
}

pub enum CmapEntry {
    Char { src: u32, dst: char },
    Range { src_start: u32, src_end: u32, dst_start: u32 },
}
```

Se o parser depender de tipos do `lopdf`, ele deve ser movido para `03_infra` ou refatorado para remover essa dependência.

## Comportamento para ToUnicode ausente ou incompleto

O sistema não deve inventar texto.

Regras:

1. Se `ToUnicode` existir e mapear o código, usar o mapeamento.
2. Se `ToUnicode` não existir, marcar o trecho como não mapeado.
3. Se `ToUnicode` existir, mas não cobrir o código, marcar o código como não mapeado.
4. A ausência de mapeamento não deve falhar a leitura do documento inteiro.
5. O relatório deve poder mostrar diagnóstico de texto não mapeado.

Exemplo de tipo de domínio:

```rust
pub enum TextMappingStatus {
    Mapped,
    Unmapped,
    PartiallyMapped,
}
```

Se o requisito futuro exigir texto Unicode obrigatório para todo documento, essa política deve ser alterada por novo ADR.

## Comportamento para PDF criptografado

O projeto não implementa descriptografia.

Regras:

1. Antes de processar conteúdo, verificar criptografia.
2. Se o PDF estiver criptografado e não houver senha, retornar erro claro.
3. Não usar `load()` seguido de `decrypt()` como caminho principal.
4. Se senha for fornecida, usar `Document::load_with_password`.
5. Senha inválida deve retornar erro distinguível.

Erros sugeridos:

```rust
pub enum PdfError {
    Encrypted,
    InvalidPassword,
    Unsupported,
    Parse,
    Io,
}
```

Comportamento esperado:

```text
PDF criptografado sem senha -> PdfError::Encrypted
PDF criptografado com senha errada -> PdfError::InvalidPassword
```

Armadilha de API registrada:

```text
`Document::load()` seguido de `decrypt()` pode retornar Ok,
mas não produz o comportamento esperado.
O caminho correto é `Document::load_with_password`.
```

Gate de criptografia:

```text
`Document::is_encrypted()`
```

## Comportamento para páginas sem texto

Se uma página não contiver operadores de texto:

1. A leitura não deve falhar.
2. O resultado deve conter texto vazio.
3. O backend pode marcar diagnóstico de página baseada em imagem.
4. OCR está fora de escopo.

Exemplo:

```rust
pub struct PageText {
    pub spans: Vec<TextSpan>,
    pub diagnostics: Vec<PageDiagnostic>,
}

pub enum PageDiagnostic {
    ImageOnlyPage,
    NoTextOperators,
    UnmappedGlyphs,
}
```

## Comportamento para Form XObjects

Texto pode aparecer dentro de Form XObjects invocados por `Do`.

Regra para extração completa:

```text
Se um Form XObject invocado por Do contiver operadores de texto,
ele deve ser processado recursivamente.
```

Se a primeira implementação não processar Form XObjects, isso deve ser registrado como limitação explícita.

Exemplo de limitação aceita:

```text
Nesta fase, somente o content stream direto da página é processado.
Texto dentro de Form XObjects não será extraído.
```

Na v1, Form XObjects invocados por `Do` não são percorridos recursivamente. Se texto existir
apenas dentro de Form XObjects, ele não será extraído. O relatório deve registar essa limitação
quando detectar `Do` com XObject de conteúdo (diagnóstico, não erro). Esta decisão pressupõe
que o Typst não usa Form XObjects para texto de corpo — a validar com PDFs reais do Typst
(ver "Validações pendentes").

Sem esse registro, o comportamento fica ambíguo.

## Evidência

Experimento: `_lab/lopdf_probe/`.

A evidência deve registrar:

- versão do Rust;
- versão do `lopdf`;
- comando executado;
- hash dos arquivos PDF de teste;
- comando usado para gerar PDF com `xref` stream;
- saída resumida.

Resultados observados no experimento original:

| Item | Resultado |
|---|---|
| Abre PDF do Typst 0.15.1 | Passou |
| Lê `xref` tradicional | Passou |
| Lê `xref` stream | Passou, com teste via recompactação QPDF |
| Lê object streams | Passou |
| Decodifica `FlateDecode` | Passou |
| Acessa content stream | Passou |
| Decodifica operadores básicos | Passou |
| Acessa dicionário `/Font` | Passou |
| Acessa stream `ToUnicode` | Passou |
| Parser CMap completo | Não fornecido pelo lopdf |
| Detecta criptografia | Passou |
| PDF scan-like | Passou como leitura estrutural |

## Consequências

### Consequências técnicas

1. Será necessário implementar parser próprio de CMap/`ToUnicode`.
2. Será necessário implementar lógica de geometria de texto em `01_core`.
3. `03_infra` deve converter tipos do `lopdf` para tipos de domínio.
4. `01_core` não pode depender de `lopdf`.
5. PDFs criptografados devem ser rejeitados com erro claro.
6. PDFs somente com imagem não produzem texto.
7. OCR não faz parte do escopo.

### Consequências arquiteturais

1. A troca futura de backend é possível se `01_core` permanecer independente de `lopdf`.
2. A separação por `DocumentGeometry` só é válida se `DocumentGeometry` for tipo de `01_core`.
3. Se `01_core` importar `lopdf`, a troca de backend não será isolada.

### Consequências de produto

1. Texto ausente em scans deve ser mostrado como ausência de texto, não como erro inesperado.
2. Glifos não mapeados devem ser diagnosticados.
3. Usuário não deve receber texto inventado quando não houver mapeamento confiável.

## Riscos conhecidos

1. `lopdf` pode não cobrir PDFs malformados do Caso 2.
2. Scans podem ser somente imagem.
3. `ToUnicode` pode estar ausente ou incompleto.
4. Fontes CID podem exigir mais lógica que `bfchar`/`bfrange`.
5. Form XObjects podem conter texto não extraído se não forem tratados.
6. Dependências transitivas do `lopdf` precisam ser verificadas se a restrição for Rust puro sem nenhum código nativo.
7. A versão do Typst usada no experimento pode não representar todas as saídas futuras.

## Validações pendentes

Antes da implementação completa, validar:

1. PDFs reais do Caso 2.
2. PDFs com `xref` stream sem recompactação artificial.
3. PDFs com object streams sem recompactação artificial.
4. PDFs com `ToUnicode` ausente.
5. PDFs com `ToUnicode` parcial.
6. PDFs com Form XObjects contendo texto.
7. PDFs com `CropBox` diferente de `MediaBox`.
8. PDFs com `Rotate`.
9. PDFs com página somente imagem.
10. Dependências transitivas do `lopdf`.

## Prompts necessários

Para implementar esta decisão, criar ou atualizar prompts em `00_nucleo/prompts/`:

```text
lopdf-backend-adapter.md
pdf-error-policy.md
page-geometry-model.md
content-stream-text-model.md
cmap-tounicode-parser.md
pdf-scan-like-diagnostic.md
```

Cada código gerado deve conter cabeçalho de linhagem apontando para o prompt correspondente.

Exemplo:

```rust
/**
 * Crystalline Lineage
 * @prompt 00_nucleo/prompts/cmap-tounicode-parser.md
 * @layer L1
 * @updated 2026-08-12
 */
```
