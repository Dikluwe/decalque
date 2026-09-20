# ADR 0005 — Reconstrução tipográfica como capacidade do Decalque

**Estado**: aceito
**Data**: 2026-09-19
**Revisa**: ADR 0004 quanto ao limite do produto; preserva sua fronteira de domínio,
seus contratos e suas regras de evidência

## Contexto

O objetivo do Decalque não termina em comparar uma observação de scan com um PDF já existente.
O produto deve transformar imagens de páginas de livros em documentos tipográficos digitais e
usar a comparação para fazê-los convergir geometricamente e textualmente para a página-fonte.

A retirada do antigo diretório `05_scan` da arquitetura Tekt corrigiu uma confusão real: SDKs de
OCR, modelos de visão, busca de fontes, scripts Python, compiladores externos e experimentos não
formam uma quinta camada. A formulação anterior, porém, confundiu esse limite arquitetural com o
limite do produto e passou a apresentar o Decalque como apenas um comparador.

O aprendizado preservado em `_lab/scan_observer` mostra que reconstrução exige capacidades
distintas: observação, calibração física, inferência editorial, formulação de hipóteses
tipográficas, geração Typst, compilação, comparação e refinamento. Elas pertencem ao produto,
mas não ao mesmo contrato nem à mesma camada.

## Decisão

### Limite do produto e limite do núcleo

1. O Decalque é o sistema completo de reconstrução tipográfica de páginas observadas em imagem.
2. A arquitetura Tekt continua terminando em `04_wiring`; não existe camada `05_scan`.
3. Provedores OCR, modelos de visão, mecanismos de busca de fontes e o executável Typst podem
   viver e ser distribuídos dentro do repositório e do produto Decalque como ferramentas,
   adaptadores ou processos compostos. Eles permanecem fora de L1 e não definem contratos de
   domínio por seus formatos particulares.
4. `ScanObservation` continua sendo a fronteira versionada entre observação e domínio. Um
   fornecedor Paddle, Ovis ou equivalente não entra no núcleo e uma unidade OCR não é convertida
   implicitamente em `GlyphInstance`.
5. `DocumentGeometry` continua representando a estrutura extraída de um PDF digital. O PDF
   gerado por Typst torna-se candidato somente depois da compilação e da extração estrutural.

“Externo” passa a ter sentidos separados: um mecanismo pode ser externo ao núcleo puro ou ao
processo — como um executável Typst ou servidor OCR — sem ser externo ao produto. O Decalque é
responsável por seus contratos, adaptadores, políticas, limites e workflows, mesmo quando a
execução concreta é fornecida por terceiro.

### Ciclo de reconstrução

O fluxo de produto é:

```text
raster imutável
    -> observação e calibração independentes do candidato
    -> ScanObservation

ScanObservation + hipóteses tipográficas + política editorial
    -> ReconstructionPlan
    -> fonte Typst determinística
    -> PDF candidato
    -> DocumentGeometry

ScanObservation × DocumentGeometry × ScanComparisonPolicy
    -> resíduos e veredito
    -> revisão somente das hipóteses e políticas
```

A observação e sua calibração não podem ser alteradas para concordar com o candidato. O ciclo de
feedback modifica somente hipóteses de reconstrução — família, tamanho, tracking, margens,
leading, estrutura de parágrafo e parâmetros equivalentes — e conserva a proveniência de cada
decisão.

Cada iteração identifica seus insumos, hipótese, versão das ferramentas e fontes, fonte Typst,
PDF candidato e relatórios. Livros multipágina serão agregados por um índice próprio sem mudar o
schema por página de `ScanObservation v1`; o registro de iterações é append-only.

### Contrato de plano

`ReconstructionPlan` é distinto de `ScanObservation` e de `DocumentGeometry`:

- a observação registra o que foi sustentado pelo raster;
- a hipótese tipográfica registra uma escolha testável, não um fato observado;
- o plano projeta medidas conhecidas para o espaço físico e associa hipóteses aos elementos;
- a fonte Typst materializa o plano sem promover hipóteses a evidência;
- somente a comparação do PDF resultante pode avaliar preservação.

Um `preserved` emitido pelo comparador v1 atesta somente seu fragmento observável. Aceitação do
documento reconstruído exige uma política de produto que enumere avaliadores obrigatórios;
tipografia, imagens ou composição ainda não observadas não são aprovadas por transitividade.

Um plano pode ser materializável mesmo contendo e publicando incertezas que não impedem a
geração de um candidato. `materializable` não significa `preserved`. Quando falta informação
estrutural indispensável — por exemplo `page_mapping` físico, texto ou caixa de uma linha
selecionada — o plano permanece `Unknown` e a materialização estrita não emite uma página que
pareça completa.

### Dois estágios de reconstrução

1. **Fac-símile geométrico por linhas**: posiciona texto reconhecido em coordenadas físicas,
   usando hipóteses tipográficas explícitas. Não força largura por deformação silenciosa nem
   inventa parágrafos. É o primeiro corte vertical.
2. **Recomposição editorial**: infere mestres de página, estilos recorrentes, parágrafos,
   colunas, cabeçalhos, rodapés, hifenização e fluxo. Ela reutiliza o mesmo ciclo de comparação,
   mas possui contratos próprios posteriores.

O primeiro estágio é uma base verificável para o segundo, não a definição final do produto.

### Responsabilidades por camada

#### L1 — domínio puro

- hipóteses tipográficas explícitas e plano de reconstrução;
- projeção determinística de geometria observada para pontos;
- preservação de `Known | Unknown`, proveniência e diagnósticos;
- planejamento sem JSON, arquivos, subprocessos, Typst ou SDK de OCR.

#### L2 — política de aplicação

- escolha do estágio de reconstrução e granularidade;
- defaults tipográficos declarados e limites numéricos;
- política de completude e apresentação de diagnósticos;
- argumentos de linha de comando.

#### L3 — fronteiras

- leitura de contratos externos e vinculação do raster;
- serialização segura e determinística da fonte Typst;
- catálogo de fontes e, posteriormente, execução limitada do compilador Typst;
- adaptadores de provedores sem vazar seus DTOs para L1.

#### L4 — composição

- observar ou carregar a observação;
- vincular raster, construir plano, emitir Typst, compilar, materializar o PDF e comparar;
- manter separados erros de observação, hipótese, materialização, compilação e comparação.

Ferramentas Python ou binários auxiliares que vivem no repositório não ganham número de camada.
Quando amadurecem como capacidade do produto, são chamados por L3/L4 através de uma fronteira
explícita; quando ainda são exploratórios, permanecem em `_lab`.

## Invariantes

1. O candidato nunca participa da observação nem da calibração física da página-fonte.
2. `Unknown` nunca é preenchido com zero, default visual, dimensão do candidato ou caixa do pai.
3. Hipótese tipográfica é identificada como hipótese e nunca publicada como observação.
4. O gerador Typst é determinístico para o mesmo plano e não incorpora o raster como fundo para
   produzir igualdade aparente.
5. Ajuste de largura por escala, stretch, tracking ou espaçamento é sempre explícito no plano.
6. A cobertura da reconstrução é publicada; uma página parcial não é apresentada como completa.
7. O PDF gerado percorre o mesmo extrator estrutural que qualquer candidato digital.
8. O comparador fornece o sinal de convergência, mas não reescreve a evidência-fonte.
9. Dependências de OCR, visão e Typst não entram em L1.
10. Não existe conversão implícita `ScanObservation -> DocumentGeometry`.
11. Falha de OCR, provedor, fonte ou compilador é falha de execução; não é `Unknown` documental
    nem divergência da página.
12. Orçamento e condição de parada do laço são política explícita; esgotamento termina como
    inconclusivo, nunca como preservado.

## Consequências

- O README passa a apresentar comparação como mecanismo de verificação e convergência, não como
  totalidade do produto.
- A frase “produtor externo ao Decalque” da ADR 0004 deve ser lida e, em revisão documental,
  substituída por “produtor externo ao núcleo e ao contrato L1”. O produtor pode integrar a
  distribuição do Decalque.
- O laboratório deixa de ser o destino conceitual permanente da reconstrução. Capacidades
  comprovadas migram seletivamente para contratos e adaptadores do produto.
- A reconstrução não infla `ScanObservation v1` com campos de fonte ou de Typst; novos contratos
  preservam a separação entre evidência e hipótese.

## Alternativas rejeitadas

### Manter o Decalque apenas como comparador

Rejeitada porque não cumpre o objetivo de transformar páginas de imagem em documentos digitais.

### Restaurar `05_scan` como camada

Rejeitada porque mistura mecanismos de fornecedor, experimentos e responsabilidades de todas as
camadas numa direção de dependência impossível de sustentar.

### Acrescentar tipografia ao `ScanObservation v1`

Rejeitada porque identidade de fonte e parâmetros editoriais são frequentemente hipóteses. Sua
inclusão apagaria a diferença entre observação e reconstrução.

### Otimizar somente por pixels

Rejeitada como autoridade normativa porque permite compensações visuais opacas. Métricas raster
podem servir como diagnóstico auxiliar, mas a convergência autoritativa permanece estrutural e
geométrica, com parâmetros explícitos.

## Histórico de Revisões

| Data | Motivo |
|---|---|
| 2026-09-19 | Tornar explícito que OCR, inferência, Typst e convergência pertencem ao produto, preservando as fronteiras de camada. |
