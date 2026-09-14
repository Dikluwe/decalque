# Prompt: perfil tipográfico observado da tinta

**Camada**: adaptador externo do Caso 2
**Depende de**: `scan-word-geometry.md`, `scan-pixel-to-point.md`

## Obrigação

Derivar medidas tipográficas apenas do envelope de tinta de palavras com baseline confiável.
Palavras sem acentos formadas exclusivamente por letras de altura-x podem informar `x_height`;
palavras com maiúsculas ou letras ascendentes podem informar `ascender_height`; palavras com
descendentes podem informar `descender_depth`. Registrar as medidas em pixels e pontos.

## Política de desconhecido

Baseline abaixo da confiança mínima, geometria ausente/inválida ou baseline fora da caixa torna
o perfil `unknown`. Acentos impedem inferir altura-x pelo topo da caixa. A ausência de evidência
rasterizada da fonte candidata mantém `typography_status: unknown`: nome e tamanho declarados no
PDF não provam equivalência visual.

## Observáveis

- envelope, baseline e confiança usados permanecem disponíveis;
- cada medida não sustentada é `null`, não zero;
- o perfil informa as características textuais que autorizaram cada medida;
- a transformação não altera a geometria original.
