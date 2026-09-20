# Prompt: verificação estrutural de fonte em candidato Typst

## Objetivo

Verificar se uma hipótese de família, corpo, peso, estilo e tracking foi realmente materializada no
PDF candidato, sem confundir compilação bem-sucedida ou bom ajuste geométrico com identidade de
fonte.

## Interface

```text
decalque attest-scan-font OBSERVATION.json
    --raster PAGE
    --output-pdf CANDIDATE.pdf
    --font-family FAMILY
    --font-size-pt NUMBER
    [--font-weight regular|bold]
    [--font-style normal|italic|oblique]
    [--tracking-pt NUMBER]
    --horizontal-tolerance-pt NUMBER
    --baseline-tolerance-pt NUMBER
    [--min-text-confidence NUMBER]
    [--min-geometry-confidence NUMBER]
    [--typst-bin PATH]
```

A granularidade é `line`. Argumentos inválidos falham antes de I/O de domínio.

## Fluxo

1. carregar a observação estrita e vincular o raster;
2. planejar linhas com a hipótese explícita;
3. emitir Typst com `fallback: false`;
4. identificar e executar o compilador sob os limites do produto;
5. exigir PDF válido de exatamente uma página;
6. materializar `DocumentGeometry` e catálogo bruto de fontes;
7. verificar sequência Unicode e recursos usados;
8. comparar a geometria com a observação;
9. serializar o relatório;
10. publicar o PDF somente quando a verificação for `preserved`.

O fluxo executa uma compilação. Nenhum dado do candidato altera observação, mapping, plano,
hipótese ou política.

## Verificação estrutural

Todos os glifos contam, inclusive espaços e ligaduras. Cada glifo precisa sustentar:

- mapeamento Unicode suficiente para reconstruir a sequência esperada;
- referência a um recurso PDF existente;
- `/BaseFont` não vazio e classificável;
- família e face compatíveis com a hipótese.

A normalização pode remover apenas prefixo de subset `[A-Z]{6}+`, `-Identity-H` ou
`-Identity-V` e uma face terminal conhecida. Família e stem usam ASCII alfanumérico, espaço,
hífen e sublinhado; separadores e caixa não participam da comparação.

## Estados

- `preserved`: texto e todos os recursos usados sustentam a hipótese; o PDF pode ser publicado;
- `violated`: existe incompatibilidade demonstrável; não publica;
- `unknown`: falta evidência necessária; não publica.

Falha operacional deixa stdout vazio e não cria destino novo. Um destino existente nunca é
alterado.

## Limites

São reutilizados os limites de avaliação de candidato: fonte 16 MiB, PDF 128 MiB, stderr 1 MiB,
versão 64 KiB e 30 segundos por invocação. O executável é chamado diretamente, sem shell.

## Testes comportamentais

Os testes devem cobrir:

- texto Unicode, espaços, ligaduras e recursos múltiplos;
- subset, família e seis faces reconhecidas;
- fallback ausente, recurso ausente e nomes opacos;
- `preserved`, `violated` e `unknown`;
- limites do compilador e PDF inválido;
- publicação somente em `preserved`, sem sobrescrita.

Eles devem verificar somente entradas, saídas e efeitos públicos.
