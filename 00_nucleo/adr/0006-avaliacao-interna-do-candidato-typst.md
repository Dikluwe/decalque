# ADR 0006 — Compilação e avaliação interna do candidato Typst

**Estado**: aceito
**Data**: 2026-09-19
**Complementa**: ADR 0005 e ADR 0002

## Contexto

O Decalque já transforma uma `ScanObservation` vinculada ao raster em um plano geométrico e em
fonte Typst determinística. Até aqui, porém, fechar o ciclo exigia três comandos coordenados fora
do produto: emitir a fonte, chamar `typst compile` e entregar o PDF resultante ao comparador.
Essa lacuna impede que uma hipótese tipográfica seja tratada como uma iteração reproduzível do
processo de reconstrução.

Compilar Typst é uma fronteira de execução, não uma responsabilidade do domínio puro. O PDF
resultante também não pode receber tratamento privilegiado: ele só vira evidência sobre a
hipótese depois de atravessar o mesmo adaptador PDF, a mesma materialização de
`DocumentGeometry` e o mesmo comparador usados para qualquer candidato digital.

## Decisão

O Decalque passa a oferecer uma avaliação de candidato por linhas com o fluxo indivisível:

```text
argumentos completos
    -> ScanObservation estrita
    -> vinculação do raster declarado
    -> ReconstructionPlan
    -> fonte Typst determinística
    -> compilação limitada em subprocesso, sem shell
    -> PDF de uma página em memória
    -> PageSource -> DocumentGeometry
    -> ScanComparisonReport
    -> publicação atômica do PDF e relatório da iteração
```

O comando inicial é `evaluate-scan-lines`. Ele recebe uma hipótese tipográfica explícita, uma
política de comparação e um caminho de saída novo para o PDF. O resultado de uma execução
completa é um relatório JSON que identifica a hipótese, a versão do compilador, os hashes da
fonte e do PDF e incorpora o relatório estrutural da comparação.

### Autoridade e estados

1. `Materializable` autoriza criar um candidato; não declara que ele preserva a fonte.
2. `Preserved`, `Violated` e `Unknown` são resultados válidos da comparação. Todos significam
   que a iteração foi executada e podem publicar o PDF candidato.
3. observação inválida, raster divergente, planejamento inconclusivo, falha/timeout do compilador,
   saída acima do limite, PDF inválido ou quantidade de páginas diferente de uma são falhas de
   execução. Nenhuma delas é convertida em `Unknown` documental.
4. o candidato nunca participa da observação, da vinculação do raster, do mapeamento físico nem
   do planejamento que o gerou.
5. o PDF só é publicado depois de compilado, analisado, materializado, comparado e serializado.
   Um caminho já existente nunca é sobrescrito.

### Fronteira do compilador

L3 executa diretamente um binário configurado, sem interpretador de comandos. Os argumentos de
compilação são fixos e incluem saída PDF em stdout e `--creation-timestamp 0`; a fonte é enviada
por stdin. A versão é obtida por uma invocação limitada separada. Fonte, stdout PDF, stderr e
tempo têm tetos explícitos de produto. Todos os pipes são drenados concorrentemente para impedir
deadlock; excesso ou timeout termina o processo e recolhe seu estado.

Durante a implementação foi observado que o processo direto podia encerrar deixando descendentes
com stdout ou stderr herdados, prolongando um timeout de 30 s para aproximadamente 35,01 s ou
bloqueando indefinidamente. A fronteira atual encerra e recolhe a árvore inteira; um ensaio com
limite de 120 ms retornou em cerca de 130 ms sem deixar descendentes.

O compilador pode consultar as fontes tipográficas instaladas no ambiente, pois essa é a
capacidade sendo testada. A versão e os hashes do artefato tornam essa dependência visível. Esta
fase não baixa fontes, não resolve pacotes de rede e não aceita argumentos Typst arbitrários.

### Publicação do artefato

O PDF permanece em memória durante a avaliação. A publicação usa arquivo temporário exclusivo no
mesmo diretório e uma operação sem sobrescrita; falhas removem o temporário. stdout só recebe o
relatório após a publicação bem-sucedida. O caminho de saída não participa do conteúdo da fonte,
do PDF, da comparação nem do hash.

## Responsabilidades por camada

- **L1** conserva os contratos puros de planejamento e comparação existentes, sem subprocessos,
  bytes PDF ou caminhos.
- **L2** define CLI, limites fechados, política de comparação e serialização determinística do
  relatório da iteração.
- **L3** executa o compilador de forma limitada, calcula hashes, analisa o PDF a partir de bytes e
  publica o artefato sem sobrescrever.
- **L4** impõe a ordem do fluxo e mantém separadas as categorias de erro.

## Consequências

- Uma hipótese tipográfica agora pode ser testada em um único comando e produz um PDF real
  vinculado ao relatório estrutural correspondente.
- A divergência deixa de ser falha operacional e passa a ser o sinal mensurável para uma revisão
  posterior da hipótese.
- O primeiro laço ainda é manual. Propor automaticamente novos valores de tracking, corpo ou
  família exige contrato posterior, orçamento e critério de parada próprios.
- O fluxo continua limitado a uma página e ao fac-símile geométrico por linhas.

## Alternativas rejeitadas

### Considerar sucesso do Typst como preservação

Rejeitada: compilação prova apenas que existe um PDF sintaticamente válido.

### Comparar o raster do PDF gerado por pixels como veredito

Rejeitada como autoridade. Diagnósticos raster poderão complementar, mas não substituir, o
contrato estrutural e a política explícita.

### Gravar o PDF antes da validação estrutural

Rejeitada porque deixa artefatos parciais ou inválidos com aparência de iteração concluída.

### Incorporar o compilador em L1

Rejeitada porque mistura domínio puro com processo, filesystem, fontes do host e bytes PDF.
