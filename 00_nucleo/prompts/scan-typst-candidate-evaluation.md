# Prompt: compilação e avaliação do candidato Typst por linhas

## Intenção

Fechar no próprio Decalque uma iteração verificável do fac-símile por linhas: carregar e vincular
a evidência do scan, materializar uma hipótese tipográfica explícita, compilar a fonte Typst,
extrair a geometria do PDF candidato e medir seus resíduos contra a mesma observação.

O resultado não escolhe uma hipótese seguinte e não promove semelhança visual a verdade. Ele
produz o PDF candidato e um relatório estrutural que permitirá ao estágio posterior revisar os
parâmetros.

## Pré-requisitos

O fluxo reutiliza `ScanObservation v1`, a vinculação integral do raster, o planejamento de
linhas, a emissão Typst com integridade de linha física e a comparação
`ScanObservation × DocumentGeometry`.

## Interface observável

```text
decalque evaluate-scan-lines OBSERVATION.json
    --raster PAGE
    --output-pdf CANDIDATE.pdf
    --font-family FAMILY
    --font-size-pt NUMBER
    [--font-weight regular|bold]
    [--font-style normal|italic|oblique]
    [--tracking-pt NUMBER]
    --granularity line|word
    --horizontal-tolerance-pt NUMBER
    --baseline-tolerance-pt NUMBER
    [--min-text-confidence NUMBER]
    [--min-geometry-confidence NUMBER]
    [--typst-bin PATH]
```

`--typst-bin` tem default literal `typst`. Todas as opções são analisadas antes de qualquer I/O de
domínio. Opções ausentes, repetidas, desconhecidas, sem valor ou numericamente inválidas falham
com código 2 e stdout vazio. Caminhos preservam a representação nativa do sistema; nenhum caminho
é inserido na fonte Typst ou no JSON do relatório.

## Ordem e isolamento obrigatórios

L4 executa exatamente esta dependência causal:

1. carregar a observação JSON com os limites publicados;
2. vincular todos os bytes e metadados do raster declarado;
3. planejar linhas apenas a partir da observação vinculada e da hipótese explícita;
4. emitir a fonte Typst determinística;
5. obter a versão e executar o compilador limitado;
6. exigir um PDF válido com exatamente uma página e materializá-la a partir dos bytes em memória;
7. comparar a observação original com o `DocumentGeometry` extraído;
8. serializar o relatório completo;
9. publicar o PDF em caminho novo, sem sobrescrever;
10. imprimir o relatório em stdout.

Nenhuma informação do PDF candidato pode alterar `ScanObservation`, `PageMapping`,
`ReconstructionPlan`, hipótese, fonte ou política. A página do candidato é sempre zero; o índice
da fonte continua identificado por `source.page_index` no relatório.

## Política de execução do Typst

Os limites v1 são fechados e não possuem flags de elevação:

- fonte: no máximo 16 MiB;
- PDF em stdout: no máximo 128 MiB;
- stderr de cada invocação: no máximo 1 MiB;
- versão em stdout: no máximo 64 KiB;
- tempo por invocação: 30 segundos.

O processo de compilação é criado diretamente, sem shell, e recebe somente:

```text
compile --format pdf --creation-timestamp 0 - -
```

A fonte vai para stdin; o PDF vem de stdout. stdout e stderr são drenados concorrentemente.
Timeout, excesso de qualquer limite, falha ao criar/escrever/esperar/terminar o processo, versão
inválida, status não zero, stdout vazio ou PDF inválido são erros de execução. O erro pode incluir
stderr limitado e sanitizado, mas nunca bytes arbitrários ilimitados.

## PDF e publicação

O adaptador `lopdf` aceita bytes em memória e reutiliza a mesma função interna de extração usada
por `load_page_source(path, page_index)`. O PDF deve ter exatamente uma página; zero ou múltiplas
páginas falham antes da comparação.

O caminho final não pode existir. A escrita usa temporário exclusivo no mesmo diretório, grava os
bytes completos, sincroniza-os e publica sem possibilidade de sobrescrever um alvo surgido em
corrida. Qualquer falha anterior deixa o destino ausente e remove o temporário. Se o destino já
existir, seus bytes permanecem inalterados.

## Relatório da iteração

Uma execução completa retorna JSON determinístico com schema
`decalque.scan-reconstruction-evaluation`, versão 1, contendo:

- índice original da página e SHA-256 do raster vinculado;
- hipótese completa: família, corpo, peso, estilo e tracking;
- versão textual do compilador;
- SHA-256 e tamanho em bytes da fonte Typst;
- SHA-256 e tamanho em bytes do PDF;
- o relatório `decalque.scan-comparison-report` integral, sem reinterpretar seus estados.

O relatório não contém timestamps, diretório corrente, caminho do executável ou caminho de saída.
Mesmos insumos, ambiente tipográfico e compilador produzem os mesmos bytes de fonte, PDF e JSON.

`Preserved`, `Violated` e `Unknown` da comparação são resultados válidos da iteração: código 0,
PDF publicado, stderr vazio e estado explícito no JSON. Código 0 nunca significa paridade por si
só. Planejamento `Unknown` e falhas de execução usam código 2, stdout vazio e nenhum PDF novo.

## Responsabilidades por camada

### L1

Nenhuma nova dependência nem I/O. Reutiliza `plan_scan_lines` e
`compare_scan_observation` sem criar conversão implícita entre observação e geometria digital.

### L2

- parser completo do novo subcomando;
- constantes dos limites e política explícita;
- renderizador determinístico do relatório externo;
- nenhuma abertura de arquivos ou processo.

### L3

- executor Typst limitado, sem shell;
- hashes dos artefatos;
- carregamento de `PageSource` a partir de bytes e exigência de página única;
- publicação atômica sem sobrescrita;
- nenhuma decisão de comparação ou reconstrução.

### L4

- composição na ordem funcional descrita acima;
- categorias distintas para observação, raster, planejamento, emissão, compilação, PDF,
  serialização e publicação;
- nenhuma saída parcial em stdout.

## Testes comportamentais

### Positivos

1. A fixture de referência, a 10 pt, gera PDF `%PDF`, exatamente uma página, texto completo, hash
   publicado e relatório estrutural válido.
2. A mesma execução em dois destinos novos produz fonte, PDF e JSON idênticos, exceto por nenhum
   campo — caminhos não aparecem no relatório.
3. O contraexemplo de 30 pt permanece uma única linha, cobertura `1/1`, conteúdo `preserved`,
   geometria e overall `violated`; ainda assim o comando retorna 0 e publica o PDF.
4. `Unknown` de baseline permanece `unknown` no relatório e não impede a publicação.
5. Um caminho de compilador com espaços/metacaracteres é tratado como um único caminho, nunca
   como comando de shell.

### Negativos

1. Raster ausente ou divergente falha antes de qualquer invocação do compilador.
2. Planejamento `Unknown` falha antes do compilador e não cria PDF.
3. Binário ausente, versão com status não zero, compilação com status não zero, timeout, stdout ou
   stderr acima do limite e stdout vazio falham sem publicar artefato.
4. Bytes que não são PDF e PDFs com zero ou duas páginas falham antes da comparação/publicação.
5. Destino existente nunca é alterado, inclusive sob corrida de publicação.
6. Falha de serialização ou publicação deixa stdout vazio.
7. O candidato não é aberto por caminho para planejar e não altera a observação ou o mapping.
8. Nenhum caminho usa shell, argumentos fornecidos pelo usuário, URL, package import, imagem de
   fundo, glifo sintético, escala, stretch, recorte ou conteúdo oculto.

### Opacos e arquitetura

1. Resultado comparativo `Unknown` é sucesso de execução e continua `Unknown`; não é convertido
   em erro, zero ou `Preserved`.
2. Diagnósticos de página, geometria, fonte e interpretação do PDF são preservados no relatório
   comparativo incorporado.
3. L1 continua sem símbolos de processo, filesystem, PDF bytes ou Typst.
4. O comando anterior `reconstruct-scan-lines` mantém interface e bytes de saída.
5. Os comandos anteriores de observação e reconstrução continuam funcionando.

## Fora de escopo

- escolher automaticamente a próxima hipótese;
- busca, download ou incorporação de fontes;
- múltiplas páginas ou manifesto de livro;
- recomposição editorial de parágrafos e leading;
- imagens, ornamentos ou comparação raster autoritativa;
- cache de compilação e execução remota.
