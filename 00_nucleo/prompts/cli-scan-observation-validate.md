# Prompt: validação vinculada de `ScanObservation v1`

**Camadas**: L2 (`02_shell`), L3 (`03_infra`) e L4 (`04_wiring`)
**Arquivos gerados/revistos futuramente**: argumentos e relatório em L2, inspeção de raster em
L3 e composição no binário `decalque` em L4
**ADR**: `00_nucleo/adr/0004-fronteira-observacao-scan.md`
**Depende de**: `scan-observation-model.md`, `scan-observation-json-adapter.md`

## Obrigação

Validar um `ScanObservation v1` isoladamente e vinculá-lo aos bytes do raster declarado. O
comando prova conformidade estrutural, limites de entrada e identidade observação-raster; não
prova que o OCR leu corretamente a página, que a provenance descreve honestamente o processo ou
que o candidato preserva a referência.

## Interface

```text
decalque validate-scan-observation <observacao.json> --raster <raster>
```

- os dois caminhos são obrigatórios exatamente uma vez e preservam nomes não UTF-8;
- `--raster` é obrigatório; não existe modo JSON-only rotulado como validação completa;
- `--help` descreve explicitamente o escopo da prova;
- argumento ausente, repetido ou desconhecido é erro antes de I/O;
- flags de candidato, OCR, modelo, rede, dispositivo, DPI, fonte e Typst são proibidas.

## Responsabilidades

L2 parseia argumentos, declara os mesmos `ScanObservationLimits` usados pela comparação e
renderiza o resumo determinístico. L3:

1. lê e valida o JSON v1 com os limites de produto;
2. lê os bytes do raster sob um limite explícito;
3. calcula SHA-256 sobre esses bytes;
4. identifica o formato pelo conteúdo e decodifica integralmente o payload de pixels sob um
   limite explícito de memória, rejeitando entrada truncada mesmo quando cabeçalho, comprimentos e
   checksums do contêiner foram reconstruídos;
5. suporta na v1 somente `image/png`, `image/jpeg` e `image/x-portable-graymap`;
6. compara digest, media type, largura e altura com `source.raster`.

L4 executa essa sequência e só então pede a L2 o relatório. Não abre PDF, não executa OCR, não
consulta rede, não inicia subprocesso e nunca interpreta `artifact_id` ou texto como caminho/URL.

PNG exige que o fluxo comprimido de todos os IDAT seja decodificado até produzir os pixels
declarados; JPEG exige decodificação completa do scan entrópico; PGM `P2` e `P5` exige todas as
amostras declaradas. Validar somente assinatura, IHDR, SOF, marcadores, comprimentos, CRC ou
cabeçalho não satisfaz esta obrigação. Extensão de arquivo não decide media type. Formato
suportado com payload incompleto, bytes, digest, tipo ou dimensões divergentes é erro, nunca
aviso.

## Saída de sucesso

`stdout` contém somente um JSON em uma linha:

```json
{"schema":"decalque.scan-observation-validation","schema_version":1,"status":"valid","scope":"contract-and-raster-identity","source":{"page_index":0,"raster_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","media_type":"image/png","width_px":1000,"height_px":2000},"counts":{"units":12,"provenance_records":3,"diagnostics":1}}
```

Regras:

- campos e ordem são fixos; números usam representação decimal sem locale;
- `status: valid` significa somente `scope: contract-and-raster-identity`;
- o resumo não ecoa texto OCR, caminhos locais, pixels, parâmetros secretos ou provenance;
- logs e erro pertencem a stderr;
- a execução válida é byte a byte determinística.

## Códigos de saída

- `0`: contrato estrito, orçamentos e identidade do raster conferem; stdout contém o resumo;
- `2`: uso, I/O, orçamento, schema, contrato, digest, formato, media type, dimensão ou
  serialização falhou; stdout permanece vazio.

Não há sucesso parcial. Um JSON estruturalmente válido ligado ao raster errado é erro.

## Limites e não alegações

- os limites JSON são exatamente os de `scan-observation`;
- L3 declara um máximo finito para bytes do raster e outro para memória de decodificação; ambos
  valem antes da publicação de qualquer resultado;
- o comando não reexecuta nem avalia o modelo OCR;
- não confirma a autoria semântica de claims nem recalcula `parameters_sha256`;
- não transforma `Unknown`, não cria `DocumentGeometry` e não produz `GlyphInstance`;
- não compara com PDF candidato.

## Critérios de verificação

1. O par fixture JSON/raster real é aceito e produz o resumo exato.
2. Byte alterado, raster substituto de mesmo tamanho e digest declarado falso são rejeitados.
3. Dimensão ou media type declarado divergente é rejeitado independentemente do digest.
4. PNG com IDAT truncado, JPEG com scan entrópico truncado, PGM sem todas as amostras, qualquer
   raster malformado e formato não suportado são rejeitados; recalcular comprimento/CRC do
   contêiner não transforma pixels incompletos em entrada válida.
5. JSON inválido falha antes de publicar qualquer resumo.
6. `artifact_id` e texto contendo URL/caminho não provocam acesso externo.
7. Flags de candidato, OCR, modelo, servidor, DPI, fonte ou Typst são rejeitadas.
8. Caminhos não UTF-8 são preservados pelo parser de argumentos.
9. Arquivo acima dos limites falha sem relatório parcial.
10. Duas execuções sobre os mesmos bytes geram stdout idêntico.
11. `--help` informa que validade não é exatidão do OCR nem paridade com candidato.

## Histórico de Revisões

| Data | Motivo |
|---|---|
| 2026-09-18 | Exigir decodificação integral após ataque com IDAT truncado e CRC reconstruído. |
| 2026-09-18 | Contrato inicial após auditoria adversarial do produtor externo. |
