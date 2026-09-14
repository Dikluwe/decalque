# Prompt: CLI de comparação digital↔digital

**Camadas**: L2 (`02_shell`) e L4 (`04_wiring`)
**Arquivos gerados**: `02_shell/src/cli.rs`; composição em `04_wiring/src/main.rs`
**Depende de**: `engine/compare.md`, `entities/measurement-resolution.md`,
`lopdf-backend-adapter.md`

## Objetivo

Expor o Caso 1 por uma interface mínima e determinística:

```text
decalque <referencia.pdf> <candidato.pdf> [--page <indice>]
```

O índice é zero-based e vale `0` quando omitido. `--help` imprime uso. Argumentos ausentes,
desconhecidos, repetidos ou índice inválido são erro de uso.

## Comportamento

1. Carregar a mesma página dos dois PDFs via `03_infra`.
2. Materializar ambos os lados via `04_wiring`.
3. Comparar com o perfil padrão digital↔digital de `02_shell`.
4. Imprimir métricas em pontos com três casas, usando `n/a` para `None`, sempre acompanhadas
   pela cobertura de ambos os lados e pelas contagens de pares e não emparelhados.
5. Sucesso de processamento retorna código 0; a v1 é ferramenta de medição e não transforma
   divergência em código de falha. Erro de uso ou leitura retorna código 2 e mensagem em stderr.

Não implementar Caso 2, múltiplas páginas numa execução, JSON, configuração de tolerância ou
um veredito global nesta versão.

## Verificação

- caminhos são preservados como `PathBuf`, inclusive quando não são UTF-8;
- default e `--page` explícito são parseados;
- argumentos inválidos são rejeitados;
- relatório sem pares imprime `n/a` e cobertura, nunca zero como falsa paridade.
