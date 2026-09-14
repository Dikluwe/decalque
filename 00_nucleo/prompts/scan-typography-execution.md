# Prompt: testes de execução da tipografia scan→digital

**Camada**: fronteira externa do Caso 2
**Depende de**: `scan-typographic-profile.md`

## Obrigação

Exercitar os programas como processos reais, usando arquivos JSON e PDF em disco. O corpus deve
cobrir rasterização bem-sucedida de uma fixture real, falha explícita do renderizador e emissão de
testemunha para mutação da forma da tinta. Não substituir `pdftocairo`, parsing de argumentos,
serialização ou códigos de saída por chamadas internas.

## Vereditos

- execução válida termina com código 0 e JSON parseável em stdout;
- erro de entrada/renderização termina com código 2 e diagnóstico em stderr;
- mutação tipográfica produz `typography_status: violated` com distância e limiar;
- ausência de evidência nunca é aceita como preservação.
