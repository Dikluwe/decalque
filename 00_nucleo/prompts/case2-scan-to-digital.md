# Caso 2 — scan → digital puro

**Estado**: ativo — contrato arquitetural do caso de uso
**ADR**: `00_nucleo/adr/0001-escopo-dois-casos-de-uso.md`,
`00_nucleo/adr/0004-fronteira-observacao-scan.md`
**Depende de**: `scan-observation-model.md`, `scan-observation-json-adapter.md`,
`scan-observation-compare.md`, `cli-scan-observation-compare.md`

## Objetivo

Validar se um PDF digital nativo preserva conteúdo e geometria observáveis de uma página
escaneada, sem fazer OCR dentro do Decalque e sem fingir que região, linha ou palavra é um glifo.

O ganho OCR entra por um artefato externo, inspecionável e versionado. O PDF candidato continua
a ser lido estruturalmente pelo pipeline Rust existente.

## Assimetria fundamental

| | Referência scan | Candidato digital |
|---|---|---|
| Fonte | raster observado externamente | content stream PDF |
| Contrato | `ScanObservation` | `DocumentGeometry` |
| Granularidade | região, linha, palavra ou glifo realmente observado | glifo estrutural |
| Proveniência | obrigatória por claim | diagnósticos do pipeline PDF |
| Incerteza | `Known | Unknown` | mapeado/não mapeado + diagnósticos |

O fluxo normativo é:

```text
produtor OCR externo
    -> ScanObservation v1 por página
    -> adaptador JSON L3
    -> ScanObservation validada em L1

raster observado
    -> decodificação e identidade vinculada em L3

PDF candidato
    -> PageSource L3
    -> DocumentGeometry L1

ScanObservation + DocumentGeometry + política L2
    -> comparação L1
    -> ScanComparisonReport
```

## Fronteira do produtor externo

O produtor externo pode escolher qualquer ferramenta, linguagem ou modelo. Ele é responsável
por:

- identificar o raster exato observado por SHA-256;
- preservar a granularidade realmente sustentada;
- separar texto, geometria, ordem, associação e confiança em claims independentes;
- registrar ferramenta, versão, modelo, método, parâmetros e predecessores;
- emitir `Unknown` quando não existir evidência suficiente;
- não consultar o PDF candidato para construir, escalar ou corrigir a observação.

O produtor não faz parte de L1/L2/L3/L4. Formatos de fornecedor não atravessam a fronteira.
Não existe uma camada `05_scan` no desenho Tekt.

## Responsabilidades Tekt

### L1 — domínio puro

- representar e validar `ScanObservation` sem I/O;
- preservar raster, granularidade, claims e proveniência;
- construir uma vista comparável do `DocumentGeometry` candidato;
- emparelhar texto exato e único na granularidade escolhida;
- medir geometria apenas quando ambos os lados estão no mesmo frame físico conhecido;
- produzir cobertura, testemunhas e vereditos trivalentes.

L1 não conhece JSON, OCR, arquivos, URLs, subprocessos ou provedores.

### L2 — política

- escolher `line` ou `word` como granularidade obrigatória;
- definir tolerância horizontal e de baseline em pontos;
- definir se confiança conhecida é obrigatória e seus limiares;
- definir a normalização textual permitida;
- nunca apresentar métrica sem cobertura nos dois sentidos.

Na v1, a normalização padrão é exata: nenhuma normalização Unicode, de caixa ou espaços é
implícita.

### L3 — infraestrutura

- ler e validar estritamente `ScanObservation v1` em JSON;
- decodificar integralmente o raster explícito e vinculá-lo à identidade declarada;
- rejeitar schema desconhecido, referência quebrada, número não finito e claim inválida;
- converter DTO externo nos tipos L1;
- carregar o PDF candidato pelo adaptador existente;
- não executar OCR, rede ou caminhos declarados no JSON.

### L4 — composição

- carregar a observação e a página candidata do mesmo índice zero-based;
- vincular o raster antes de abrir o PDF candidato;
- manter separados erros da observação, erros PDF e diagnósticos de comparação;
- aplicar a política L2 e chamar o comparador L1;
- produzir um único relatório completo; falha de uma etapa não vira sucesso parcial.

## Política de `Unknown`

`Unknown` é esperado quando texto, geometria, ordem, confiança, associação ou transformação não
têm evidência suficiente.

- `Unknown` não é erro de execução.
- JSON malformado ou claim contraditória é erro de contrato, não `Unknown`.
- claim abaixo do limiar de confiança da política é rebaixada a `Unknown` com causa explícita.
- cobertura incompleta produz eixo `unknown`, salvo quando já existe violação demonstrável.
- cobertura `0/0` nos dois lados é escopo vazio, não prova de completude, e produz os três
  status `unknown`.
- `page_mapping: Unknown` sempre impede `geometry_status: preserved`, inclusive quando o
  escopo está vazio ou nenhum delta chegou a ser medido.
- somente cobertura completa de um escopo não vazio e todos os observáveis obrigatórios
  aprovados produzem `preserved`.

Precedência agregada:

```text
violated > unknown > preserved
```

## Granularidade e associação

- Região fornece contexto e nunca é subdividida proporcionalmente.
- Linha é comparável quando possui texto conhecido, associação única e, para geometria,
  transformação física conhecida.
- Palavra é comparável somente quando `text` e `span_in_parent` são `Known` dentro de linha conhecida.
- Glifo OCR é aceito apenas como observação explícita; não é necessário para a v1 e não é
  convertido automaticamente em `GlyphInstance`.
- Texto repetido, associação múltipla ou span incompatível permanece `unknown`.
- Uma linha observada cujas palavras conhecidas correspondem inequivocamente a mais de uma linha
  candidata produz testemunha de reflow `violated`.

## Coordenadas

Geometria OCR permanece em pixels YDown do raster observado. Uma claim `page_mapping` pode
projetá-la para pontos YDown da página-fonte exibida.

- coordenadas originais nunca são alteradas;
- a transformação não usa dimensões do candidato;
- bbox transformada usa os quatro cantos;
- dimensão física, crop, rotação ou `UserUnit` sem tratamento tornam a geometria comparativa
  `unknown`;
- texto e cobertura continuam comparáveis quando a transformação física é desconhecida.

## Proveniência manual

Toda raiz de proveniência referencia diretamente o raster observado, inclusive uma raiz com
`stage: manual-annotation`. Anotação humana é um método de observação do raster, não uma fonte
sem artefato. Registro sem artefato de entrada só é válido quando possui pai, e a cadeia termina
numa raiz ligada a `source.raster.artifact_id`.

## Diferenças esperadas

- Ligaduras no candidato são expandidas pela sequência Unicode já preservada em
  `GlyphInstance.codepoints`.
- Fonte substituída pode alterar avanços; a v1 mede início, fim, largura e baseline de
  linha/palavra, não identidade de fonte.
- Reflow é tratado explicitamente, não escondido por emparelhamento posicional.
- Erro OCR pode causar conteúdo não associado; na ausência de prova adicional isso reduz
  cobertura e produz `unknown`, não falsa violação geométrica.
- Imagens e ornamentos sem observação textual permanecem fora do comparador v1.

## Fora de escopo da v1

- executar ou configurar OCR;
- baixar ou descobrir fontes;
- inferir identidade tipográfica;
- gerar fontes, Typst ou PDF;
- comparar pixels ou realimentar parâmetros por raster;
- composição editorial;
- equivalência de elementos não textuais;
- container multipágina no schema.

## Critérios de verificação

1. Um produtor diferente pode emitir o mesmo schema sem alteração em L1/L2/L3/L4.
2. Uma região com texto e sem linhas geométricas continua válida; as linhas têm geometria
   `Unknown`.
3. Nenhum caminho cria glifos OCR por divisão de bbox ou texto.
4. Observação com `page_mapping: Unknown` permite relatório de conteúdo e força geometria
   `unknown`, inclusive quando não há unidades na granularidade selecionada.
5. Texto exato e único com geometria dentro da tolerância pode produzir `preserved` somente
   com cobertura completa e não vazia nos dois lados.
6. Texto repetido sem desambiguação produz associação `unknown`.
7. Reflow demonstrado produz `violated` e identifica linhas envolvidas.
8. Métrica zero sobre cobertura parcial não produz `preserved`.
9. O índice usado para o candidato é o `source.page_index` do artefato.
10. Nenhuma opção de CLI seleciona modelo, servidor ou ferramenta OCR.
11. Escopo `0/0` nos dois lados produz conteúdo, geometria e resultado geral `unknown`.
12. Uma raiz `manual-annotation` sem referência ao raster é rejeitada; com referência direta
    ao raster, segue as mesmas regras das demais raízes.
13. Raster divergente ou truncado falha antes da abertura do candidato e não publica relatório.

## Histórico de Revisões

| Data | Motivo |
|---|---|
| 2026-09-19 | Tornar a vinculação do raster pré-condição atômica da comparação. |
| 2026-08-11 | Planejamento inicial baseado em conversão OCR → `DocumentGeometry`. |
| 2026-09-18 | ADR 0004 substitui a conversão por fronteira externa `ScanObservation v1` e comparador Rust dedicado. |
| 2026-09-18 | Correção normativa focal: elegibilidade de palavra exige conjuntamente `text` e `span_in_parent` conhecidos, alinhada a `scan-observation-compare.md`. |
| 2026-09-18 | Refinamento pós-verificação: proibida preservação vacuosa em `0/0`, explicitado o veto geométrico de `page_mapping: Unknown` e vinculada toda raiz manual ao raster. |
