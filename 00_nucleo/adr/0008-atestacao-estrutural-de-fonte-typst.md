# ADR 0008 — Verificação estrutural da fonte materializada pelo Typst

**Estado**: aceito
**Data**: 2026-09-19
**Complementa**: ADR 0005, ADR 0006 e ADR 0007

## Contexto

Bom ajuste geométrico não demonstra que o Typst usou a família solicitada. Uma compilação pode
recorrer a fallback por glifo, e a presença de algum `/BaseFont` não prova cobertura integral.

## Decisão

O comando `attest-scan-font` avalia uma hipótese explícita:

1. carrega a observação e vincula o raster;
2. planeja as linhas e emite Typst com `fallback: false`;
3. compila um PDF de uma página sob os limites do produto;
4. extrai sequência Unicode e recursos de fonte efetivamente usados;
5. compara família, peso e estilo por normalização fechada;
6. mede o candidato contra a observação;
7. publica o PDF somente quando o resultado estrutural é `preserved`.

A execução é direta. Não há segunda compilação destinada apenas a confirmar o processo.

## Alcance

`preserved` significa que o texto planejado foi extraído integralmente, todos os glifos possuem
recurso PDF conhecido e cada `/BaseFont` usado é compatível com a hipótese. Não significa
identidade visual, identidade binária do arquivo de fonte ou correção histórica da família.

O nome bruto de `/BaseFont` é preservado como evidência. Para comparação podem ser removidos
somente prefixo de subset `[A-Z]{6}+`, sufixos `-Identity-H`/`-Identity-V` e uma variante
fechada: `Regular`, `Bold`, `Italic`, `BoldItalic`, `Oblique` ou `BoldOblique`.

## Estados e publicação

- `preserved`: relatório de sucesso e publicação exclusiva do PDF;
- `violated`: testemunha estrutural incompatível; relatório sem publicação;
- `unknown`: evidência insuficiente; relatório sem publicação;
- falha de entrada, compilação, PDF, serialização ou publicação: erro operacional e stdout vazio.

O PDF permanece em memória até a publicação. Destino existente não é sobrescrito.

## Responsabilidades

- L1 decide cobertura textual e compatibilidade nominal sem I/O.
- L2 define CLI, limites e relatório.
- L3 emite Typst estrito, compila, analisa PDF e publica sem sobrescrita.
- L4 compõe uma única execução e impede publicação de resultado não preservado.
