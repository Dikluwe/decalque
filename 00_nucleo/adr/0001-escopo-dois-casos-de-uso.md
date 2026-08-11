# ADR 0001 — Escopo: dois casos de uso sobre o mesmo núcleo geométrico

**Estado**: aceito
**Data**: 2026-08-11

## Contexto

As specs iniciais de `00_nucleo/prompts/` foram escritas com foco num único cenário: paridade
entre dois PDFs digitais gerados por compiladores/versões diferentes do mesmo documento
(o caso motivador P948, typst vs. typst). O objetivo principal do projeto é, porém, mais
amplo: validar a conversão de PDF **digitalizado** (scan, página como imagem) para PDF
**digital puro** (texto nativo, por exemplo gerado via Typst), verificando se o gerado
mantém paridade com o original.

## Decisão

O Decalque cobre os dois casos de uso como **camadas distintas de funcionalidade sobre o
mesmo núcleo geométrico** (`01_core`):

1. **Paridade digital↔digital** — pipeline estrutural completo, ambos os lados com content
   stream de glifos e `ToUnicode`. Tolerâncias apertadas (divergência = regressão).
2. **Paridade scan→digital** — o lado do scan não tem glifos; a extração da geometria de
   texto do original é etapa externa (OCR/análise de imagem, fora do núcleo) que produz um
   `DocumentGeometry` que alimenta o mesmo pipeline. Tolerâncias e normalizações
   configuráveis por caso de uso (fontes substituídas, ligaduras expandidas, reflow são
   diferenças legitimamente esperadas, não erros).

## Consequências

- `MeasurementResolution` deixa de ser um valor global único: a política de tolerância é
  parâmetro do caso de uso, não constante do domínio.
- O emparelhamento (`engine/compare`) precisa de normalização de ligaduras — expansão do
  glifo via `ToUnicode` para a sequência de codepoints — para não reportar "fi" vs.
  "f"+"i" como divergência. `GlyphInstance.codepoint` passa a ser sequência, não `char`
  único (spec a atualizar).
- Pixels continuam fora do núcleo: comparação visual por imagem renderizada é sensível a
  DPI, antialiasing e rasterizador, e foi a abordagem que produziu conclusões
  contraditórias no caso motivador. Se um dia for desejada, é camada opcional fora de
  `01_core`, obrigatoriamente com o mesmo rasterizador e mesmo DPI nos dois lados.
- `XObject`s de imagem (o conteúdo do scan) participam da comparação por posição e
  dimensão, não por conteúdo rasterizado.
