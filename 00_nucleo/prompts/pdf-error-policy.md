# Prompt: política de erros PDF — `PdfError`

**Camada**: L1 — `01_core`
**Arquivo gerado**: `01_core/src/entities/pdf_error.rs` (novo) + testes no mesmo ficheiro
**ADR**: `00_nucleo/adr/0002-lopdf-backend-parsing.md` (secção "Comportamento para PDF criptografado")

## Contexto

A taxonomia de erros PDF é **domínio**, não infra: `02_shell` precisa apresentá-los ao usuário
e `03_infra` apenas converte erros do lopdf para este tipo. Por isso `PdfError` vive em
`01_core` — defini-lo em `03_infra` criaria duas taxonomias quando o shell precisasse dele
(análise do dono sobre `lopdf-backend-adapter.md`, 2026-08-12).

Decisão do dono (2026-08-12): **a v1 não aceita senha**. PDF criptografado →
`PdfError::Encrypted`. `Document::load_with_password` fica registado no ADR 0002 como caminho
para versão futura.

## Restrições estruturais

- L1: zero I/O, não importa lopdf nem tipos de `03_infra`.
- Erros carregam mensagem legível para `02_shell` apresentar (o shell não deve ter de
  interpretar variantes para compor texto).

## Instrução

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum PdfError {
    Io { message: String },              // ficheiro não encontrado, permissão, leitura
    Parse { message: String },           // estrutura inválida, content stream indecodificável, MediaBox ausente
    Encrypted,                           // PDF criptografado (v1 não aceita senha)
    InvalidPassword,                     // reservado para versão futura com senha (ADR 0002)
    Unsupported { message: String },     // recurso PDF fora do escopo suportado
    PageNotFound { page_index: usize },  // page_index fora do intervalo do documento
}
```

Regras de mapeamento (obrigatórias em `03_infra`, documentadas aqui como contrato):

| Origem | Variante |
|---|---|
| Erros de ficheiro (não encontrado, permissão, I/O) | `Io` |
| Erros de parsing/estrutura (xref inválido, stream indecodificável, `MediaBox` ausente mesmo por herança) | `Parse` |
| Recurso PDF fora do escopo (filtro não suportado etc.) | `Unsupported` |
| `page_index` fora do intervalo | `PageNotFound` |
| PDF criptografado (gate `is_encrypted()`, sem senha na v1) | `Encrypted` |

`InvalidPassword` existe na taxonomia desde já (ADR 0002 prevê `load_with_password` no futuro)
mas **nenhum caminho da v1 o produz** — documentar no doc-comment da variante.

## Resultado esperado

- `01_core/src/entities/pdf_error.rs`: `PdfError` com as 6 variantes, `Display` legível em
  português técnico para cada variante, testes inline cobrindo os critérios.

## Critérios de verificação

Dado cada variante de `PdfError`
Quando `Display` é formatado
Então produz mensagem legível que distingue a variante sem necessidade de `match` no shell

Dado `PageNotFound { page_index: 7 }`
Quando inspeccionado
Então o índice pedido está acessível no payload (o shell pode dizer "página 7 não existe")

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| 2026-08-12 | Criação — análise do dono sobre o adapter: `PdfError` é domínio (01_core), não infra; `PageNotFound` adicionado; v1 sem senha (`Encrypted` apenas); tabela de mapeamento como contrato para `03_infra` | `pdf_error.rs` (novo) |
