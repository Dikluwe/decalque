# Decalque <sub>_comparação geométrica de PDFs, sem OCR, sem pixel_</sub>

Ferramenta genérica para comparar a posição real de cada traço/glifo desenhado entre dois PDFs
quaisquer — não compara pixels renderizados (sensível a DPI, antialiasing, tipo de rasterizador) e
não compara texto extraído (cego a duplicação/posição, como já provou ser insuficiente no projeto
que motivou esta ferramenta). Lê os operadores de desenho do content stream diretamente e reporta
a diferença de posição real, em pontos.

Construída seguindo a Arquitetura Cristalina (Tekt) — `00_nucleo` é o ponto de partida para
qualquer trabalho neste projeto.

*Decalque*: papel de decalque reproduz a posição exata de cada traço de um desenho original,
por transferência directa — não por olhar e redesenhar. É essa a diferença entre esta ferramenta
e comparar pixels renderizados ou texto extraído: em vez de reinterpretar o que se vê, ela lê a
posição real de cada traço directamente da fonte (o content stream), como o papel encostado ao
desenho original.

---

## O pipeline central (o problema em três perguntas)

Dado um PDF, a ferramenta responde, em ordem, três perguntas — cada uma é uma peça de domínio
própria, e a resposta de uma condiciona a seguinte:

1. **Qual é o tamanho da folha?** (`PageGeometry`) — a `MediaBox` (ou equivalente) de cada página.
   Dois PDFs do "mesmo" documento podem legitimamente ter folhas de tamanhos diferentes (por
   exemplo, `width: auto` resolvendo para valores distintos em compiladores distintos) — isto não
   é por si só uma divergência a reportar, é o contexto que torna o resto da medição possível.
2. **Onde fica o ponto (0,0)?** (`normalize_to_top_left`) — o espaço de usuário do PDF é
   YUp por especificação (origem no canto inferior esquerdo); a inversão YDown só existe via
   matriz `cm`, que o intérprete já processa na CTM. Resta só a convenção de saída do
   relatório: YDown, origem no canto superior esquerdo — uma conversão matemática pura, não
   uma heurística de detecção.
3. **Com que resolução/tolerância a posição é medida?** (`MeasurementResolution`) — dois traços
   "na mesma posição" na prática nunca têm exatamente o mesmo float; é preciso um limiar. E esse
   limiar provavelmente não deveria ser um valor absoluto fixo (0.5pt faz sentido para texto de
   corpo, mas é enorme relativo a um índice de 6pt, e minúsculo relativo a um título de 40pt) —
   candidato natural: resolução relativa ao tamanho de fonte/em do que está a ser medido, não um
   número absoluto único para o documento inteiro. **Decisão fechada (ADR 0001)**: ambas as formas
   suportadas; a tolerância é parâmetro do caso de uso, não constante do domínio — ver
   `00_nucleo/prompts/entities/measurement-resolution.md`.

Só depois destas três respostas é que faz sentido comparar dois documentos: extrair a lista de
glifos desenhados de cada um (`GlyphInstance`), emparelhar os correspondentes entre os dois
(por conteúdo — codepoint via `ToUnicode` — e ordem de leitura, não por posição absoluta, que é
precisamente o que está a ser medido), e calcular o delta de posição de cada par, relativo à
origem apropriada (não necessariamente a origem da página — pode ser a origem do cluster/
construção a que o glifo pertence, para não confundir "página maior" com "glifo deslocado").

## Escopo: dois casos de uso, camadas distintas de funcionalidade

O Decalque não é uma ferramenta de um único cenário — é o mesmo núcleo geométrico servindo a
dois casos de uso com exigências diferentes. Deixar isto explícito evita decisões de desenho
que resolvam um caso e quebrem o outro.

### Caso 1 — Paridade entre versões (o motivador original)

Dois PDFs **digitais** gerados por compiladores/versões diferentes do mesmo documento-fonte
(por exemplo, duas versões do Typst compilando o mesmo `.typ`). Ambos os lados têm content
stream com glifos posicionados e `ToUnicode` — o pipeline estrutural completo se aplica sem
ressalvas. Tolerâncias podem ser apertadas: divergências reais aqui são bugs de regressão.

### Caso 2 — Scan → digital puro (o objetivo principal)

Um PDF **digitalizado** (página como imagem rasterizada, sem texto real) foi convertido para
um documento digital nativo (por exemplo, via Typst), e o Decalque valida se o PDF gerado
mantém paridade com o original. Este caso muda a assimetria da comparação:

- O lado do scan **não tem glifos no content stream** — tem `XObject`s de imagem. Extrair a
  geometria de texto do original exige uma etapa anterior de extração (OCR/análise da imagem),
  fora do núcleo do Decalque, que produz um `DocumentGeometry` equivalente para alimentar o
  mesmo pipeline.
- Diferenças legitimamente esperadas são maiores: fontes substituídas, ligaduras expandidas
  ("fi" como 1 glifo vs. "f"+"i" separados), reflow de quebra de linha. A política de
  tolerância e a normalização de emparelhamento (expansão via `ToUnicode`) precisam ser
  configuráveis por caso de uso, não globais.
- A comparação continua estrutural — pixels continuam fora do núcleo (sensíveis a DPI,
  antialiasing, rasterizador; foi essa abordagem que falhou no caso motivador). Comparação
  visual, se um dia for desejada, é camada opcional fora de `01_core`, com mesmo rasterizador
  e mesmo DPI nos dois lados.

## Por que isto existe

Um projeto irmão (typst-crystalline, arquitetura Tekt) construiu uma primeira versão disto como
script interno (`tools/geometry/compare.py`, Python, `pikepdf`) para resolver um problema concreto:
comparações visuais e por texto extraído produziram conclusões contraditórias entre passos
consecutivos sobre se um delimitador matemático estava "duplicado" ou não — só resolvido descendo
ao content stream bruto com posições. Esta ferramenta generaliza essa solução para qualquer par de
PDFs, e reimplementa em Rust para poder ser embutida/reusada (não só um script standalone).

## Estrutura

```
decalque/
├── 00_nucleo/     # Prompts e ADRs (a Semente)
├── 01_core/       # Modelo de domínio puro: PageGeometry, normalize_to_top_left,
│                  # MeasurementResolution, GlyphInstance, DocumentGeometry,
│                  # algoritmo de emparelhamento/diff. Zero I/O.
├── 02_shell/      # CLI
├── 03_infra/      # Leitura/parsing real de PDF (content stream, ToUnicode)
├── 04_wiring/     # main.rs, composição
└── _lab/          # Experimentos isolados
```

## Estado actual

`PageGeometry`, o modelo de fontes, o parser `ToUnicode`/CMap, `PdfError`,
`normalize_to_top_left`, o intérprete de content stream e o adaptador
`lopdf` já refletem as especificações activas.
`CartesianOrigin` foi removido: a transformação para YDown agora é uma função pura que
consome a origem da caixa efectiva. Os diagnósticos de página, `DocumentGeometry` e a
revisão do motor de comparação também já estão materializados. `02_shell` já aplica o
perfil padrão do Caso 1 e mantém métricas agregadas junto da cobertura; o parsing da CLI
ainda aguarda especificação própria. O próximo passo implementável é `04_wiring`.
