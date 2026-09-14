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

O candidato deve ser rasterizado diretamente na resolução do scan. Perfis de palavras com
correspondência textual exata e única são comparáveis medida a medida. Ao menos uma medida
homóloga é obrigatória: deltas dentro da tolerância são `preserved`, fora dela são `violated`;
sem medida homóloga o resultado tipográfico é `unknown`. Esse veredito cobre o perfil medido,
não identidade absoluta da família da fonte.

Além das métricas verticais, cada envelope pode registrar descritor normalizado da forma da
tinta: densidade, centroide e projeções em oito faixas horizontais e verticais. Descritores de
mesma versão e dimensão produzem uma distância visual separada. Distância acima do limiar viola
o perfil mesmo quando altura-x, ascendentes e descendentes coincidem.

Os defaults são 1 pt para medidas verticais e 0,12 para a distância normalizada. A tolerância
vertical também deve cobrir dois pixels na escala observada, representando a quantização das duas
bordas comparadas. Os limiares e a contribuição de quantização permanecem explícitos no relatório.

## Observáveis

- envelope, baseline e confiança usados permanecem disponíveis;
- cada medida não sustentada é `null`, não zero;
- o perfil informa as características textuais que autorizaram cada medida;
- a transformação não altera a geometria original.
