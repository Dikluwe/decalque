# Prompt: derivação do perfil geométrico observado

## Objetivo

Converter uma `ScanObservation` validada em medições físicas que um documento Typst possa
consumir, preservando a diferença entre evidência conhecida e informação indisponível.

## Entrada

O comando público é:

```text
decalque derive-scan-layout OBSERVATION.json
    --raster PAGE
    [--output-typst DESTINATION.typ]
```

A observação é carregada pelo adaptador JSON estrito e vinculada aos bytes reais do raster. O
raster prova identidade; as coordenadas físicas vêm exclusivamente de `page_mapping`.

## Derivação no núcleo

`derive_scan_layout`:

1. exige uma observação de domínio válida;
2. projeta todos os quatro cantos de cada bbox pela homografia;
3. deriva a dimensão física da página quando o frame de destino fornece extensão em pontos;
4. calcula envelope textual e margens somente com linhas projetáveis;
5. agrupa regiões e linhas sem usar a ordem incidental do JSON;
6. registra cada resultado como `derived` ou `unavailable`;
7. não inventa baseline, recuo, leading, tracking, fonte ou papel editorial.

A estrutura `ScanLayoutProfile` é opaca. Chamadores recebem visões imutáveis da fonte,
cobertura, página, envelope, margens e regiões.

## Representações

`render_scan_layout_profile` retorna um bundle com:

- relatório JSON determinístico;
- módulo Typst somente de dados quando a página é derivável;
- SHA-256 e tamanho calculados dos bytes exatos do módulo;
- variante de relatório para publicação concluída.

O módulo Typst não executa lógica, não importa pacotes e não contém caminhos locais. Strings são
escapadas e números finitos são emitidos de forma canônica.

## Publicação

Sem `--output-typst`, o comando apenas escreve o relatório não publicado em stdout. Com destino:

- artefato disponível: publica exatamente os bytes renderizados, sem sobrescrever, e só então
  libera o relatório com `published: true`;
- artefato indisponível: não cria arquivo e retorna o relatório explicando a razão;
- erro de publicação: stdout permanece vazio e o destino anterior permanece intacto.

## Invariantes de comportamento

- permutar campos JSON ou unidades semanticamente equivalentes não altera a saída;
- página desconhecida nunca vira página de tamanho zero;
- projeção de bbox usa quatro cantos, inclusive em homografia projetiva;
- entrada inválida, hash de raster divergente e publicação recusada falham com marcador estável;
- testes verificam valores públicos, bytes emitidos e efeitos no destino.
