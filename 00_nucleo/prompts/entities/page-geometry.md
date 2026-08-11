# Prompt: `PageGeometry`

**Camada**: L1 — `01_core`
**Arquivo gerado**: `01_core/src/entities/page_geometry.rs` + testes no mesmo ficheiro

## Contexto

Primeira das três perguntas do pipeline (ver `README.md`). Antes de comparar qualquer posição
entre dois PDFs, é preciso saber o tamanho de cada folha — não para exigir que sejam iguais (não
são, necessariamente, e essa diferença não é por si só um defeito), mas para servir de referência
a qualquer normalização de coordenadas feita depois.

## Restrições estruturais

- L1: zero I/O. Esta struct **não lê ficheiro nenhum** — recebe os valores já extraídos (isso é
  trabalho de `03_infra`, que lê a `MediaBox`/equivalente do PDF real e constrói esta struct).
- Deve suportar múltiplas páginas por documento (um PDF pode ter mais que uma página; a versão
  inicial pode assumir 1 página — o caso de uso motivador, typst com `.typ` de página única — mas
  a struct não deve ser desenhada de forma que impeça extensão a múltiplas páginas depois).

## Instrução

Criar `PageGeometry { width: f64, height: f64 }` (unidades: pontos PDF, 1/72 polegada — a unidade
nativa do formato, não converter para outra coisa).

Método `fn size_delta(&self, other: &PageGeometry) -> (f64, f64)` — devolve `(Δwidth, Δheight)`,
sem julgar se é "grande" ou "pequeno" (isso é responsabilidade de quem consome, não desta struct).

## Critérios de verificação

Dado duas `PageGeometry` iguais
Quando `size_delta` é chamado
Então devolve `(0.0, 0.0)`

Dado duas `PageGeometry` com larguras diferentes e alturas iguais
Quando `size_delta` é chamado
Então `Δwidth` reflete a diferença real (com sinal, `other - self`, documentar a convenção
escolhida explicitamente no doc-comment) e `Δheight` é `0.0`

## Histórico de Revisões

| Data | Motivo | Ficheiros afectados |
|------|--------|----------------------|
| 2026-08-11 | Criação inicial + primeira geração de `01_core` | `page_geometry.rs` |
