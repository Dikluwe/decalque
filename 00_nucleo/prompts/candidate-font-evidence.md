# Prompt: evidência tipográfica do PDF candidato

**Camada**: adaptador externo do Caso 2
**Depende de**: `scan-observation-model.md`, `pdf-font-model.md`

## Obrigação

Enriquecer tokens observados no scan com a fonte declarada no PDF candidato somente quando
o texto do token estiver integralmente alinhado a glifos mapeados, todos os glifos usarem a
mesma fonte e a fonte possuir `/BaseFont`. A evidência é estrutural: não afirma semelhança
visual entre a fonte do scan e a fonte do candidato.

O catálogo do candidato deve preservar, por glifo, texto Unicode, recurso de fonte, nome
`/BaseFont`, tamanho em pontos e posição. Prefixos de subset (`ABCDEF+`) podem ser removidos
da família apresentada, mas o nome declarado original permanece na evidência.

## Política de desconhecido

Token parcialmente alinhado, glifo sem Unicode, mistura de recursos/tamanhos, fonte sem
`/BaseFont` ou correspondência ambígua permanece `unknown`. Espaçamento é normalizado apenas
para localizar texto e nunca constitui evidência tipográfica.

## Observáveis

- enriquecimento não altera texto, spans ou geometria do scan;
- evidência identifica método, PDF, recurso e `/BaseFont` original;
- tamanho do candidato usa `size_pt`; não é convertido para pixels sem DPI conhecido;
- falhas de alinhamento não escolhem a fonte majoritária.
