# Prompt: reconstrução Typst geométrica por linhas

## Intenção

Materializar o primeiro corte vertical do objetivo central do Decalque: transformar a observação
de uma página raster em uma fonte Typst tipográfica, determinística e inspecionável, que possa ser
compilada e comparada pelo próprio Decalque até convergir para a página-fonte.

Este corte não infere ainda a identidade da fonte nem recompõe parágrafos. Ele recebe uma
hipótese tipográfica explícita e projeta linhas OCR conhecidas para pontos físicos. A saída é um
candidato verificável; sua emissão não constitui veredito de preservação.

## Pré-requisitos

O fluxo reutiliza `ScanObservation v1`, o adaptador JSON estrito, a vinculação completa do
raster, o `PageMapping` homográfico e a comparação
`ScanObservation × DocumentGeometry`. Esses componentes são dependências funcionais, não
baselines congelados.

## Escopo observável

### Entrada

1. Uma `ScanObservation v1` válida e já vinculada aos bytes do raster declarado.
2. Uma hipótese tipográfica explícita contendo ao menos:
   - família não vazia;
   - tamanho finito e estritamente positivo em pontos;
   - peso Typst suportado;
   - estilo Typst suportado;
   - tracking finito em pontos.
3. Política de reconstrução por linhas, inicialmente estrita e limitada a unidades `line`.

A hipótese é configuração de reconstrução, não claim observada. Seu valor deve permanecer
visível no plano e na fonte emitida.

### Pré-condições físicas

`page_mapping` deve ser `Known`, válido e ligado por proveniência ao raster. Sua
`target_frame` usa pontos, origem superior esquerda, X à direita e Y para baixo. Nenhuma dimensão
é lida de um PDF candidato.

Cada linha selecionada exige:

- `text: Known`, não vazio;
- `geometry.bbox: Known`, não degenerada;
- `reading_order: Known`, inteiro não negativo e não ambíguo, assim como a ordem de cada
  ancestral `region` que determine sua posição;
- proveniência válida já garantida pelo contrato da observação.

Cada unidade observada permanece uma linha física indivisível. A composição usa sua largura
tipográfica natural; não quebra, comprime ou recorta o texto para caber na bbox. Qualquer excesso
permanece mensurável no PDF candidato.

`baseline` pode permanecer `Unknown` neste corte. Quando conhecida, sua projeção é preservada no
plano como alvo de comparação; quando desconhecida, não é substituída pela borda da bbox.

### Planejamento L1

O planejador puro:

1. seleciona somente unidades `line` no escopo;
2. constrói a chave de leitura pela sequência de `reading_order` dos ancestrais até a linha e
   ordena lexicograficamente por essa chave, sem usar a ordem incidental do JSON;
3. projeta os quatro cantos de cada bbox pela homografia;
4. constrói o envelope físico em pontos no frame da página-fonte;
5. projeta a baseline somente quando a claim correspondente é conhecida;
6. conserva id da unidade, texto, bbox-alvo, baseline opcional e hipótese tipográfica;
7. publica cobertura `linhas_planejadas / linhas_no_escopo` e diagnósticos determinísticos.

O plano completo possui dimensão física da página, linhas ordenadas e a hipótese tipográfica.
Ele não contém bytes raster, caminho do raster, PDF candidato nem `GlyphInstance` sintético.

### Política de `Unknown`

A materialização estrita não emite fonte Typst quando:

- `page_mapping` é `Unknown` ou não satisfaz a política;
- não existe linha no escopo;
- qualquer linha selecionada possui texto, bbox ou algum componente de sua chave de leitura
  `Unknown`;
- a hierarquia não determina uma ordem total inequívoca;
- qualquer ponto projetado é não finito ou a homografia não cobre a geometria;
- a hipótese tipográfica é inválida.

O resultado deve distinguir `Unknown` de erro de contrato. A observação malformada continua sendo
rejeitada antes do planejamento. `Unknown` não vira plano vazio, coordenada zero nem sucesso.

### Emissão Typst L3

Para um plano completo, o emissor produz UTF-8 determinístico:

1. `page(width:, height:, margin: 0pt)` usa exclusivamente a dimensão do target frame;
2. `text` declara família, tamanho, peso, estilo, tracking, desativa hifenização e usa bordas de
   glyph bounds para o estágio de fac-símile;
3. cada linha é emitida uma vez, em ordem de leitura, com `place(top + left, dx:, dy:)` a partir
   do topo esquerdo de sua bbox-alvo;
4. texto e família são escapados como strings Typst, sem permitir injeção de markup;
   o texto é preservado exatamente, sem `trim`, normalização Unicode ou colapso de espaços;
5. largura e altura observadas permanecem alvos no plano, mas o emissor não escala, estica,
   recorta nem quebra a linha silenciosamente para forçar coincidência;
6. não há imagem de fundo, conteúdo oculto, acesso de rede, caminho externo ou execução do
   compilador no emissor;
7. números usam representação decimal finita, locale-independente e sem `-0`;
8. a mesma entrada produz bytes idênticos independentemente de repetição ou ordem de campos JSON.

O emissor deste corte não declara `par.leading`, não usa caixa artificial de largura infinita e
não converte tamanho nominal da fonte em altura de tinta. A fonte não contém caminhos absolutos,
estado do host ou referência ao diretório de execução.

O posicionamento pela borda de ink é uma hipótese materializável. Baseline desconhecida continua
desconhecida no plano e no veredito posterior. O comparador mede o PDF compilado e fornece os
resíduos necessários para revisar tamanho, tracking e deslocamento.

## Distribuição por camada

### L1

- entidades de hipótese e plano;
- resultado tri-state de planejamento e diagnósticos;
- projeção homográfica e construção determinística do plano;
- zero I/O, JSON, Typst, OCR ou subprocessos.

### L2

- parsing da política e da hipótese na CLI;
- limites explícitos e apresentação de `Unknown`;
- nenhuma leitura de arquivo.

### L3

- serialização segura da fonte Typst a partir de um plano pronto;
- reutilização do adaptador JSON e da vinculação raster existentes;
- nenhuma execução de OCR ou compilador neste corte.

### L4

- ordem obrigatória: parse de argumentos → observação estrita → vinculação do raster →
  planejamento → emissão;
- stdout recebe fonte somente após sucesso completo;
- falhas e `Unknown` deixam stdout vazio e diagnóstico em stderr.

## Interface inicial

```text
decalque reconstruct-scan-lines OBSERVATION.json
    --raster PAGE.png
    --font-family FAMILY
    --font-size-pt NUMBER
    [--font-weight regular|bold]
    [--font-style normal|italic|oblique]
    [--tracking-pt NUMBER]
```

Defaults, quando existentes, são política L2 publicada e aparecem explicitamente na fonte.
Caminhos preservam bytes não UTF-8. Opções repetidas, desconhecidas ou sem valor falham antes de
qualquer I/O de domínio.

## Testes comportamentais

### Positivos

1. Escala afim simples projeta página e bbox para pontos exatos.
2. Homografia projetiva usa os quatro cantos da bbox.
3. Linhas fora de ordem no JSON são emitidas pela chave hierárquica de leitura.
4. Texto com aspas, barra invertida, colchetes, `#`, quebras e Unicode não injeta Typst.
5. Repetição da mesma entrada produz bytes idênticos e fonte compilável pelo Typst suportado.
6. Baseline conhecida é preservada no plano sem alterar a âncora de ink deste estágio.
7. A CLI vincula raster antes de planejar e nunca consulta PDF candidato.

### Negativos

1. Omissão ou repetição de opção obrigatória falha antes de I/O.
2. Hipótese vazia, não finita ou não positiva é rejeitada.
3. `page_mapping: Unknown` não produz fonte.
4. Texto, bbox ou ordem `Unknown` em linha do escopo não produz página parcial.
5. Ordem duplicada não é resolvida pela ordem incidental da entrada.
6. Homografia que produz ponto não finito não gera coordenada substituta.
7. O emissor não usa `scale`, `stretch`, `image`, `raw`, caminho ou conteúdo oculto.
8. O emissor não aplica `trim`, não colapsa espaços e não substitui peso ou estilo inválido por
   defaults silenciosos.

### Opacos

1. Observação válida sem linhas retorna `Unknown`, não fonte vazia.
2. Baseline `Unknown` permite candidato de ink-bounds, mas permanece explicitamente ausente do
   plano e não permite alegar preservação de baseline.

### Determinismo e arquitetura

1. Os testes passam em repetição e em ordem de execução invertida.
2. L1 continua sem dependências externas e sem símbolos de Typst/OCR/arquivo.
3. Nenhum caminho cria `GlyphInstance` a partir das linhas.
4. Nenhuma alteração dos contratos protegidos das fases 0004–0006 é necessária para obter êxito.

## Fora de escopo deste corte

- executar OCR dentro do binário Rust;
- descobrir ou baixar fontes;
- estimar família, peso, tamanho, baseline ou tracking;
- derivar corpo ou leading da altura da bbox;
- agrupar linhas em parágrafos ou inferir mestres de página;
- compilar Typst e executar o laço automático de otimização;
- comparar pixels;
- materializar imagens e ornamentos;
- declarar equivalência editorial ou geométrica geral.

Esses itens pertencem ao produto descrito pela ADR 0005 e serão materializados por contratos
posteriores sobre esta base.
