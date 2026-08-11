# Prompt: adaptador lopdf — `03_infra`

**Camada**: L3 — `03_infra`
**Arquivo gerado**: `03_infra/src/lopdf_adapter.rs` (novo) + testes de integração em `03_infra/tests/` (usam fixtures de `_lab/lopdf_probe/fixtures/` ou fixtures próprios)
**ADR**: `00_nucleo/adr/0002-lopdf-backend-parsing.md` (regra de camada, escopo de leitura, criptografia)

## Contexto

`03_infra` é a única camada que toca ficheiros e a única que importa lopdf. O seu papel é
estreito e deliberado: ler o PDF, descomprimir o que há para descomprimir, e entregar a
`01_core` **dados brutos já decodificados** em tipos de domínio — nunca tipos do lopdf, nunca
interpretação. A interpretação (CMap, operadores, posições) vive em `01_core`
(`cmap-tounicode-parser.md`, `content-stream-text-model.md`).

## Restrições estruturais

- Direcção permitida: `03_infra → lopdf`, `03_infra → 01_core`. Proibida: `01_core → lopdf`.
- **Não vazar tipos do lopdf** na interface pública: nada de `lopdf::Document`,
  `lopdf::Object`, `lopdf::Stream` em assinaturas.
- Converter todos os números para `f64` antes de entregar (ver `00_nucleo/adr/0003-convencoes-transversais.md`).

## Instrução

O adaptador **deve**:

- abrir o PDF (`Document::load`; gate de criptografia com `is_encrypted()` — ver política de
  erros abaixo);
- ler trailer, root e catálogo; localizar páginas na árvore;
- para cada página, extrair `MediaBox`, `CropBox`, `Rotate`, `UserUnit` e montar
  `PageBoxModel` (valores brutos — a resolução é de `01_core`);
- resolver recursos herdados de página e extrair o dicionário `/Font`;
- obter `/Contents` como stream ou array de streams, descomprimir (FlateDecode via lopdf) e
  entregar `Vec<Vec<u8>>` (um bloco por stream, na ordem);
- para cada fonte, localizar o stream `ToUnicode` e entregar os bytes descomprimidos;
- converter números para `f64`;
- emitir diagnóstico para página sem texto ou baseada em imagem (pré-marcação para o
  diagnóstico de `01_core`: `ImageOnlyPage`/`NoTextOperators` — ver
  `pdf-scan-like-diagnostic.md`, a escrever);
- detectar XObjects de formulário invocados por `Do`? — **não**: a detecção de `Do` é do
  intérprete de `01_core`; o adaptador apenas entrega os streams.

O adaptador **não deve**:

- calcular geometria final de texto;
- interpretar CMap (passa os bytes de `ToUnicode` a `01_core`);
- interpretar operadores de texto;
- decidir política de relatório;
- implementar OCR;
- reparar, escrever, mesclar, dividir ou reorganizar o PDF.

### Interface (esqueleto)

```rust
pub struct FontResource {
    pub resource_name: String,        // nome no dicionário /Font da página (ex.: "f0")
    pub base_font: Option<String>,    // /BaseFont, se presente
    pub tounicode: Option<Vec<u8>>,   // bytes do stream ToUnicode, descomprimidos
}

pub struct PageSource {
    pub box_model: PageBoxModel,             // tipo de 01_core, valores brutos
    pub content_streams: Vec<Vec<u8>>,       // descomprimidos, na ordem de /Contents
    pub fonts: Vec<FontResource>,
    pub has_text_operators_hint: bool,       // heurística barata para diagnóstico; 01_core decide
}

pub fn load_page_source(path: &Path, page_index: usize) -> Result<PageSource, PdfError>;
```

### Política de erros (pré-figuração de `pdf-error-policy.md`, a escrever a seguir)

```rust
pub enum PdfError {
    Encrypted,
    InvalidPassword,
    Unsupported,
    Parse,
    Io,
}
```

Regras (ADR 0002): PDF criptografado sem senha → `PdfError::Encrypted`. Senha fornecida →
`Document::load_with_password`; senha errada → `PdfError::InvalidPassword` (variante de erro
distinguível — comparar por `match`, `lopdf::Error` não implementa `PartialEq`).
**Armadilha registada**: `load()` seguido de `decrypt()` retorna Ok mas não funciona — caminho
proibido; o correcto é `load_with_password`.

## Critérios de verificação

Dado o fixture `typst.pdf` (Typst 0.15.1 real, em `_lab/lopdf_probe/fixtures/`)
Quando `load_page_source(path, 0)` é chamado
Então devolve `PageSource` com `box_model.media_box` preenchido, `content_streams` não vazio e
legível (bytes começam com operadores PDF), e `fonts` contendo pelo menos uma fonte com
`tounicode: Some(_)`

Dado o fixture com xref stream + object streams (recompactado via qpdf)
Quando `load_page_source` é chamado
Então comporta-se como o anterior (transparência de formato de xref)

Dado o fixture criptografado (AES-256, qpdf) sem senha
Quando `load_page_source` é chamado
Então devolve `Err(PdfError::Encrypted)`

Dado o fixture scan-like (página só com XObject de imagem)
Quando `load_page_source` é chamado
Então `content_streams` decodifica sem erro e `has_text_operators_hint == false`

Dado um PDF com página sem `CropBox` e com `Rotate: 90`
Quando `load_page_source` é chamado
Então `box_model.crop_box == None` e `box_model.rotate == Some(90)` (bruto, sem normalizar —
normalização é de `01_core`)

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| 2026-08-12 | Criação — restrições do dono: sem vazamento de tipos lopdf, conversão para `f64`, gate de criptografia, entrega de dados brutos (sem interpretação) | `lopdf_adapter.rs` (novo) |
