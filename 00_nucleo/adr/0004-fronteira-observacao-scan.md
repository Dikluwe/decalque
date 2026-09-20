# ADR 0004 — OCR externo e fronteira `ScanObservation v1`

**Estado**: aceito
**Data**: 2026-09-18
**Revisa**: ADR 0001 apenas quanto à fronteira OCR → núcleo e ao uso de
`DocumentGeometry` no Caso 2

## Contexto

O Caso 1 trabalha com glifos estruturais extraídos de PDFs digitais. O Caso 2 parte de uma
imagem: provedores OCR observam normalmente regiões, linhas ou palavras, com texto, geometria,
ordem, confiança e proveniência independentes. Essas observações não possuem necessariamente
`glyph_code`, avanço, fonte, tamanho ou posição por glifo.

Tratar a extração OCR como uma quinta camada Tekt levou `05_scan` a acumular responsabilidades
heterogêneas: provedores e modelos, pré-processamento raster, descoberta e reconstrução de
fontes, geração Typst, realimentação visual e composição editorial. Essas responsabilidades não
formam uma camada arquitetural do Decalque e não devem condicionar o núcleo Rust.

Forçar uma observação de região, linha ou palavra a entrar como `DocumentGeometry` também perde
informação material: apaga a proveniência de cada claim, esconde a granularidade realmente
observada e pressiona o produtor a fabricar glifos ou métricas inexistentes.

O ganho a preservar é a observação OCR inspecionável, não a implementação específica que a
produziu.

## Decisão

### Fronteira externa

1. A arquitetura Tekt do Decalque termina em `04_wiring`. Não existe camada arquitetural
   `05_scan`.
2. OCR, modelos, pré-processamento de imagem e associação de evidências são executados por um
   produtor externo ao Decalque.
3. O único contrato de entrada OCR é um artefato JSON versionado chamado
   `decalque.scan-observation`, versão 1, definido em
   `00_nucleo/prompts/scan-observation-model.md`.
4. Um artefato representa exatamente uma página e identifica por SHA-256 o raster exato que
   foi observado. Pixels não entram em `01_core`.
   Toda comparação recebe esse raster explicitamente e prova a identidade na mesma execução,
   antes de abrir o PDF candidato; uma validação anterior não substitui essa vinculação.
5. O produtor externo não recebe nem consulta o PDF candidato para construir a observação. Em
   particular, não pode escalar, recortar ou reclassificar a observação para fazê-la concordar
   com o candidato.
6. Formatos de fornecedor — Paddle, Ovis, LM Studio, hOCR, TSV ou equivalentes — não são
   contratos do Decalque. O produtor é responsável por convertê-los em `ScanObservation v1`.

### Contratos distintos

`ScanObservation` e `DocumentGeometry` são contratos distintos:

- `DocumentGeometry` representa glifos estruturais de uma página digital;
- `ScanObservation` representa claims OCR na granularidade realmente sustentada;
- não existe conversão implícita de região, linha ou palavra para `GlyphInstance`;
- o Caso 2 usa um comparador próprio:

```text
ScanObservation × DocumentGeometry × ScanComparisonPolicy
    -> ScanComparisonReport
```

O motor `engine/compare` continua responsável por
`DocumentGeometry × DocumentGeometry`. Normalização de ligaduras e demais propriedades gerais
desse motor permanecem válidas para PDFs digitais, mas não tornam uma palavra OCR num glifo.

### Granularidade

A versão 1 admite unidades `region`, `line`, `word` e `glyph`, organizadas por relações de pai
explícitas. Palavra/linha é a granularidade primária do comparador do Caso 2. Região fornece
contexto. Glifo só é aceito quando o produtor possui evidência própria dessa granularidade; a
presença do tipo não autoriza subdivisão proporcional nem herança da caixa do pai.

Tipografia, identidade de fonte, reconstrução de fonte, materialização Typst e comparação raster
não pertencem ao contrato v1.

### Política de `Unknown`

Todo observável incerto é uma união marcada `Known | Unknown`.

- `Known` exige valor válido, base de evidência, confiança explícita — conhecida ou
  desconhecida — e proveniência não vazia.
- `Unknown` exige motivo e não contém valor.
- `null`, zero, string vazia, caixa degenerada, ausência de campo e caixa copiada do pai não
  representam `Unknown` válido.
- artefato malformado ou internamente contraditório é erro de contrato, não observação
  `Unknown`.
- falha total do produtor não emite uma observação aparentemente válida.
- o consumidor pode rebaixar um `Known` a `Unknown` por política de confiança, mas nunca
  promover `Unknown` a sucesso.

No relatório, `violated` domina `unknown`, que domina `preserved`. `Preserved` exige cobertura
completa de um escopo declarado **não vazio** e todos os observáveis obrigatórios conhecidos e
dentro da tolerância. Cobertura `0/0` nos dois lados não é cobertura completa: é ausência de
evidência e produz `content_status: unknown`, `geometry_status: unknown` e
`overall_status: unknown`. Da mesma forma, `page_mapping: Unknown` impede
`geometry_status: preserved`, ainda que não exista unidade associada nem delta fora da
tolerância; uma violação conhecida em outro eixo continua a dominar pelo critério geral.

### Proveniência de anotação manual

Toda raiz do grafo de proveniência v1 referencia diretamente o raster declarado em
`source.raster.artifact_id`, inclusive quando `stage == manual-annotation`. A etapa manual
identifica quem ou como observou o raster; não cria uma origem de evidência sem artefato. Um
registro sem `input_artifact_ids` precisa ter ao menos um pai, e toda cadeia de claim conhecida
termina numa raiz ligada ao raster. Evidência humana sem raster identificável fica fora do
contrato v1, em vez de ser aceita como exceção implícita.

### Coordenadas

A geometria autoritativa permanece no frame do raster observado: pixels contínuos sobre bordas,
origem superior esquerda, X para a direita e Y para baixo. Uma transformação opcional para o
espaço físico da página-fonte em pontos é uma claim própria e proveniente.

É proibido derivar essa transformação das dimensões do PDF candidato. Dimensão física ausente,
crop desconhecido, rotação sem suporte, `UserUnit` sem tratamento ou transformação inválida
produzem geometria comparativa `Unknown`; nenhuma dessas situações autoriza ajuste implícito.

### Responsabilidades por camada

#### L1 — domínio puro

- tipos `ScanObservation`, `Claim`, granularidade, coordenadas e proveniência;
- validação sem I/O das invariantes do contrato;
- construção determinística da vista textual/geometria do candidato a partir de
  `DocumentGeometry`;
- comparação e agregação trivalente;
- nenhuma dependência de JSON, caminhos, OCR, rede ou subprocessos.

#### L2 — política de aplicação

- granularidade exigida;
- tolerâncias horizontal e de baseline;
- limiares de confiança;
- política explícita de normalização textual;
- apresentação do relatório sem separar métricas de cobertura.

#### L3 — fronteira externa

- leitura estrita do JSON v1;
- leitura limitada, decodificação completa e vinculação do raster explicitamente fornecido;
- rejeição de versão, campos, números ou referências inválidos;
- conversão de DTO externo para tipos L1;
- limites de tamanho/complexidade para entrada não confiável;
- nenhuma execução de OCR e nenhum acesso automático a caminhos ou URLs mencionados pelo
  artefato.

#### L4 — composição

- carregar a observação por L3;
- vincular a observação aos bytes do raster na mesma execução;
- carregar e materializar a página candidata pelo pipeline PDF existente;
- verificar que o índice da página candidata é o declarado pela observação;
- aplicar a política L2 e chamar o comparador L1;
- manter diagnósticos de parsing, PDF e comparação separados.

## Consequências

- O README deverá deixar de apresentar `05_scan` como camada quando houver uma revisão
  documental autorizada.
- Implementações OCR existentes podem ser movidas para ferramenta ou repositório próprio e
  continuar produzindo o contrato v1.
- `DocumentGeometry`, `GlyphInstance` e `engine/compare` deixam de prometer que qualquer OCR
  possa entrar no pipeline como glifos.
- O Caso 2 ganha uma fronteira auditável e independente do fornecedor, mas requer um novo
  comparador Rust em vez de reutilização forçada do motor por glifo.
- Observações sem transformação física continuam úteis para conteúdo e cobertura, mas seu eixo
  geométrico permanece `unknown`.
- A versão 1 é por página. Execução multipágina é composição sequencial e não altera o schema.

## Alternativas rejeitadas

### Manter `05_scan` como camada Tekt

Rejeitada porque mistura infraestrutura mutável e experimentos de reconstrução com o contrato
de domínio.

### Fazer OCR emitir `DocumentGeometry`

Rejeitada porque a granularidade usual não sustenta os campos obrigatórios de `GlyphInstance` e
porque a conversão apagaria `Unknown` e proveniência.

### Consumir diretamente o JSON de um fornecedor

Rejeitada porque tornaria o núcleo dependente de schema, versão, modelo e política do provedor.

### Remover OCR do objetivo do produto

Rejeitada porque descartaria texto, ordem e geometria já observáveis no scan. A decisão é
externalizar a produção, não eliminar o Caso 2.

## Verificação arquitetural

1. Nenhum módulo L1 ou L2 importa SDK/biblioteca de OCR, cliente de modelo ou runtime Python.
2. Nenhum caminho do Caso 2 cria `GlyphInstance` a partir de região, linha ou palavra.
3. O parser JSON está em L3; o domínio L1 não depende de serialização externa.
4. O produtor da observação pode ser substituído sem alteração do contrato ou do comparador.
5. Ausência de transformação para pontos não impede análise textual e não produz paridade
   geométrica.
6. Nenhum veredito `preserved` é possível com cobertura incompleta ou claim obrigatória
   `Unknown`.
7. O PDF candidato não participa da produção nem da calibração da observação.
8. Escopo vazio (`total_scan == 0` e `total_candidate == 0`) produz os três status
   `unknown`; completude não é satisfeita por igualdade vacuosa `0 == 0`.
9. `page_mapping: Unknown` impede `geometry_status: preserved`, inclusive em escopo vazio.
10. Toda raiz de proveniência, inclusive `manual-annotation`, referencia diretamente o raster
    observado; raiz sem raster é rejeitada.
11. A comparação falha antes de abrir o candidato quando o raster não confere ou não decodifica;
    o SHA publicado no relatório pertence à identidade vinculada nessa execução.

## Histórico de Revisões

| Data | Motivo |
|---|---|
| 2026-09-19 | Fechar a cadeia de evidência exigindo vinculação do raster dentro da comparação. |
| 2026-09-18 | Decisão de retirar OCR da arquitetura em camadas, preservar seu ganho por contrato externo versionado e separar `ScanObservation` de `DocumentGeometry`. |
| 2026-09-18 | Refinamento pós-verificação: escopo vazio e mapeamento desconhecido não produzem preservação; toda raiz de proveniência v1, inclusive manual, referencia o raster. |
