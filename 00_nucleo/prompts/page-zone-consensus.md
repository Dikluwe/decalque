# Prompt: zonas hierárquicas e consenso OCR da página

**Camada**: adaptador externo do Caso 2
**Depende de**: `scan-margin-word-anchors.md`, `paddle-line-geometry.md`

## Obrigação

Dividir uma página em quadrantes espaciais e regiões de conteúdo sem cortar regiões que cruzem
fronteiras. Detectar linhas pela tinta, associá-las em ordem às linhas do testemunho textual e
agrupar linhas por distância vertical. Cada região registra tipo provável, quadrantes tocados,
caixa, linhas, âncoras laterais e confirmação independente disponível.

OvisOCR2 atua como testemunho de estrutura e texto; PaddleOCR/PP-OCR fornece texto e, quando
disponível, geometria; GOT-OCR2.0 é um testemunho textual opcional. A tinta observada é sempre a
autoridade geométrica. Ausência de um provedor não conta como concordância.

## Observáveis

- quatro quadrantes cobrem a página inteira sem sobreposição interna;
- uma região que atravessa uma fronteira lista todos os quadrantes intersectados;
- linhas preservam a ordem, o texto e a caixa mínima de tinta;
- símbolos sem linha textual correspondente são registrados como regiões visuais, não descartados;
- cada linha expõe âncoras esquerda/direita quando observáveis;
- a imagem de inspeção usa cores estáveis por tipo e identifica regiões e linhas;
- o JSON registra os testemunhos efetivamente presentes e o motivo do estado de consenso.

## Estados

- `confirmed`: geometria observada e pelo menos dois testemunhos textuais concordantes;
- `geometry-confirmed`: geometria observada, mas sem duas leituras concordantes;
- `text-confirmed`: duas leituras concordam, mas a geometria é desconhecida;
- `disputed`: testemunhos presentes divergem materialmente;
- `unknown`: evidência insuficiente para qualquer conclusão anterior.

## Política de desconhecido

Contagem incompatível de linhas, associação ambígua, imagem ilegível ou provedor ausente não é
convertido em sucesso. Zonas ambíguas permanecem explícitas e não autorizam materialização Typst.
