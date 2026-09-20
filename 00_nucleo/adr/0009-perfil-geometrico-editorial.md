# ADR 0009 — Perfil geométrico observado para Typst

**Estado**: aceito
**Data**: 2026-09-19
**Complementa**: ADR 0004, ADR 0005 e ADR 0007

## Contexto

O Decalque precisa transformar medições sustentadas por um scan em parâmetros consumíveis por
Typst sem converter ausência de evidência em valor presumido. A entrada continua sendo
`ScanObservation`; OCR e visão fornecem evidência, não verdade editorial.

## Decisão

O núcleo deriva um `ScanLayoutProfile` somente a partir da observação validada.

- O tamanho físico da página vem de `page_mapping.target_frame.extent`.
- Bboxes são projetadas pelos quatro cantos da homografia.
- Envelope textual e margens são calculados somente quando existe geometria comparável.
- Regiões e linhas preservam identificadores, ordem observada e estados explícitos.
- Recuo, leading, tracking, identidade de fonte e semântica editorial não são inferidos pela
  ausência de dados.
- Cada medição informa `derived` ou `unavailable`, acompanhada de razão estável.

A estrutura interna do perfil permanece privada. A API pública oferece apenas visões de leitura,
evitando que chamadores fabriquem um perfil sem passar pela derivação.

## Saídas

A camada de política produz duas representações determinísticas:

1. um relatório JSON para inspeção e automação;
2. um módulo Typst somente de dados, com dimensões e medições em pontos.

O SHA-256 e o tamanho pertencem aos bytes efetivamente emitidos; são identidade de artefato, não
cadeia administrativa. Entrada semanticamente equivalente deve produzir os mesmos bytes.

A camada de composição pode publicar o módulo Typst em caminho novo. Destino existente nunca é
sobrescrito. Se a página física estiver indisponível, o comando retorna relatório válido sem
fabricar artefato Typst.

## Consequências

- Medidas observadas podem alimentar templates Typst sem acoplar o núcleo ao renderer.
- Lacunas do OCR permanecem visíveis.
- A derivação não resolve sozinha fonte, corpo, tracking, leading ou estrutura de parágrafo.
- Refinamentos posteriores devem ser avaliados compilando o candidato e medindo novamente o PDF.
