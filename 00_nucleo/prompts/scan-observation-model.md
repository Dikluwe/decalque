# Prompt: modelo intermédio de observações de scan

**Camada**: fronteira inspeccionável do Caso 2
**Depende de**: `case2-scan-to-digital.md`

## Obrigação

Representar separadamente aquilo que foi observado na imagem e aquilo que foi apenas
derivado da transcrição. A hierarquia é página → região → linha → token. Geometria,
reconhecimento e tipografia têm confiança e proveniência independentes.

Uma caixa de região nunca deve ser copiada ou subdividida para fabricar caixas de linha,
palavra ou glifo. Quebras presentes literalmente na transcrição podem criar linhas sem
geometria. Tokens podem ser derivados reversivelmente do texto da linha e carregam offsets,
mas continuam sem geometria até existir detector que a forneça.

## Tipografia

Cada token contém `font`, com `status` igual a `observed`, `inferred` ou `unknown`.
Família, estilo, peso e tamanho permanecem nulos em `unknown`. Uma hipótese observada ou
inferida precisa indicar confiança e evidências identificando método e fonte. Ausência de
informação nunca escolhe a fonte mais provável por padrão.

## Observáveis

- o texto da região é reconstruível exactamente a partir de linhas, `break_after` e tokens;
- offsets de token são intervalos Unicode semiabertos dentro da linha;
- confiança do detector de layout não é confiança do reconhecimento nem da fonte;
- geometria indisponível é `null`, não caixa vazia, zero ou cópia da região;
- itens sem ordem declarada mantêm ordem estável depois dos itens ordenados.

## Política de desconhecido

Qualquer identidade tipográfica, geometria ou confiança sem evidência pública suficiente é
serializada como `null`/`unknown`. O consumidor não pode converter desconhecido em sucesso.
