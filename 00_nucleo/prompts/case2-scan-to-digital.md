# Caso 2 — scan → digital puro (planeamento)

**Estado**: planeamento — **não gera código agora**. O objectivo corrente é construir o Caso 1
(digital↔digital); este documento existe para fixar onde o Caso 2 se liga ao Caso 1, de forma a
que as decisões de desenho de hoje não fechem portas que o Caso 2 precisa abertas (ADR 0001).

## O caso de uso

Um PDF digitalizado (página como imagem rasterizada, sem texto real) foi convertido para um
documento digital nativo (por exemplo, via Typst, após OCR/transcrição). O Decalque valida se o
PDF gerado mantém paridade com o original.

## Assimetria fundamental

| | Caso 1 (digital↔digital) | Caso 2 (scan→digital) |
|---|---|---|
| Lado A | content stream de glifos (`03_infra`) | imagem raster — **sem glifos no content stream** |
| Lado B | content stream de glifos (`03_infra`) | content stream de glifos (`03_infra`) |
| Diferenças esperadas | quase nenhuma (divergência = regressão) | fontes substituídas, ligaduras expandidas, reflow |
| Tolerância | apertada | mais larga |
| Origem do `DocumentGeometry` A | leitura do PDF | **extracção externa (OCR/análise de imagem)** |

## Onde se liga ao Caso 1 (os pontos de contacto)

O núcleo geométrico (`01_core`) **não muda** entre os casos. O Caso 2 entra exactamente nos
mesmos pontos de fronteira que o Caso 1 já define:

1. **`DocumentGeometry` é o contrato de entrada único** — a extracção externa do scan (OCR +
   posições dos glifos reconhecidos) produz um `DocumentGeometry` na mesma forma que `03_infra`
   produz para um PDF digital. O motor de comparação não distingue a proveniência
   (`engine/compare.md`, secção "Ligação ao Caso 2").
2. **A extracção OCR é fora do núcleo e fora de `03_infra` tal como está specificado** — é uma
   fonte alternativa de `DocumentGeometry`. Camada a decidir quando o Caso 2 for construído
   (módulo novo, por exemplo `05_scan`, ou ferramenta externa que emite um formato
   intermédio). Decisão adiada deliberadamente.
3. **Tolerância é parâmetro do chamador** — `02_shell` escolhe o perfil de
   `MeasurementResolution` por caso de uso (ver `entities/measurement-resolution.md`).
4. **Normalização de ligaduras já está no motor** (ADR 0001) — motivada pelo Caso 2,
   implementada no Caso 1 porque é correcta em geral.

## Diferenças esperadas no Caso 2 (para não tratar como erro)

- Ligadura expandida: "fi" (1 glifo) vs. "f"+"i" (2 glifos) — resolvido pela normalização.
- Fonte substituída: métricas diferentes → posições de glifos subsequentes na mesma linha
  deslocam-se sistematicamente. O delta relativo à origem do cluster (lição 2 de P948) mitiga;
  a tolerância mais larga absorve o resto. Se não bastar, pode ser preciso um modo de
  comparação por palavra/linha em vez de por glifo — decisão a tomar com dados reais, não agora.
- Reflow de quebra de linha: OCR/transcrição pode quebrar linhas em pontos diferentes. A
  clusterização por proximidade vertical assume linhas equivalentes nos dois lados — este é o
  **maior risco conhecido** do Caso 2 sobre o desenho actual, a validar com um documento real
  quando o caso for construído.
- Imagens: o scan inteiro é um `XObject`; o gerado pode reutilizar imagens recortadas ou não
  ter imagem nenhuma. Comparação de `XObject`s por posição/dimensão (sem pixel) fica para a
  spec de elementos não-textuais, quando necessária.

## O que falta decidir quando o Caso 2 for construído

1. Formato intermédio da extracção OCR (directo para `DocumentGeometry` vs. ficheiro
   intermédio inspeccionável — preferência: inspeccionável, lição P948 de que a inspecção
   humana resolve o que a heurística não decide).
2. Se o OCR entrega confiança por glifo/palavra, e se essa confiança entra no relatório.
3. Modo de comparação por palavra/linha (ver reflow acima).
4. Spec de elementos não-textuais (`XObject`).
5. Ferramenta de OCR concreta (Tesseract hOCR/TSV dá posições por palavra; glifo a glifo é
   mais raro — isto pode forçar a decisão 3).
