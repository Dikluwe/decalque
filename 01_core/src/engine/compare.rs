//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/engine/compare.md
//! @layer L1
//! @updated 2026-08-12
//!
//! Motor de emparelhamento e comparação de dois `DocumentGeometry`.
//!
//! ESTADO: cumpre a revisão de 2026-08-11 da spec. A revisão de 2026-08-12
//! (secção "Como o motor usa os campos do novo `GlyphInstance`") ainda não
//! está aplicada: o emparelhamento posicional decide-se aqui por
//! `codepoints.is_none()`, e não por `mapping_status == Unmapped`, porque o
//! campo ainda não existe em `GlyphInstance` (ver `glyph_instance.rs`).

use crate::entities::{DocumentGeometry, GlyphInstance, MeasurementResolution};

/// Um par de glifos emparelhados, com o delta de posição entre eles.
#[derive(Debug)]
pub struct GlyphPair<'a> {
    /// Glifo do lado A (primeiro glifo do lado A no par — ver `compare`).
    pub a: &'a GlyphInstance,
    /// Glifo do lado B (primeiro glifo do lado B no par).
    pub b: &'a GlyphInstance,
    /// Delta de posição **relativo à origem do cluster**, com convenção de
    /// sinal `b − a`: `(dx, dy)` positivo significa que B está mais à direita /
    /// mais abaixo que A, em coordenadas relativas ao cluster de cada lado.
    pub delta: (f64, f64),
    /// `true` se `|dx|` e `|dy|` estão dentro da `MeasurementResolution` para
    /// o tamanho de fonte do glifo de A.
    pub within_resolution: bool,
}

/// Relatório agregado de uma comparação.
///
/// Glifos sem par **não são erro**: `unmatched_*` regista quantos ficaram de
/// cada lado (pode ser sintoma real — conteúdo a mais/a menos — ou limitação
/// do emparelhamento; não decidir qual sem inspecção humana).
///
/// A mediana é o sinal primário de triagem (lição 3 do P948: `max|Δ|` tem
/// artefactos que a mediana não tem); o máximo é exposto para inspecção.
#[derive(Debug)]
pub struct ComparisonReport<'a> {
    pub pairs: Vec<GlyphPair<'a>>,
    pub unmatched_a: Vec<&'a GlyphInstance>,
    pub unmatched_b: Vec<&'a GlyphInstance>,
    pub median_abs_dx: f64,
    pub median_abs_dy: f64,
    pub max_abs_dx: f64,
    pub max_abs_dy: f64,
}

/// Compara dois `DocumentGeometry` (já normalizados) e produz o relatório.
///
/// Pipeline (lições do protótipo P948):
/// 1. **Ordenação por ordem de leitura** (y, depois x) — a ordem de emissão no
///    content stream pode divergir entre dois PDFs visualmente equivalentes.
/// 2. **Clusterização por proximidade vertical** ("linhas"): um glifo inicia
///    novo cluster quando `|y - y_referência_do_cluster| > 0.5 * font_size_pt`
///    do glifo. Clusters de A e B são emparelhados pela sua ordem vertical.
/// 3. **Emparelhamento por âncora textual** dentro de cada par de clusters:
///    a sequência de codepoints de cada lado é expandida a nível de codepoint
///    (**normalização de ligaduras**, ADR 0001 — um glifo "fi" contribui dois
///    codepoints, "f"+"i" contribuem um cada) e alinhada por LCS
///    (implementação própria, sem dependência externa). Cada sequência
///    contígua emparelhada gera um `GlyphPair` que pode ser **1-para-N
///    glifos**; o delta usa a posição do **primeiro glifo de cada lado** —
///    a diferença de largura entre ligadura e expandido não é divergência de
///    posição a reportar. Glifos sem `codepoints` não têm âncora textual e são
///    emparelhados entre si pela ordem horizontal dentro do cluster.
/// 4. **Delta relativo à origem do cluster** (posição do primeiro glifo do
///    cluster, em ordem de leitura, de cada lado), não da página inteira —
///    páginas de tamanhos diferentes são legítimas (ver `PageGeometry`).
///
/// O motor não sabe nem precisa de saber qual caso de uso está a servir
/// (ADR 0001): o que muda entre Caso 1 e Caso 2 é **parâmetro** (a
/// `MeasurementResolution` passada pelo chamador), não algoritmo.
pub fn compare<'a>(
    a: &'a DocumentGeometry,
    b: &'a DocumentGeometry,
    resolution: &MeasurementResolution,
) -> ComparisonReport<'a> {
    let clusters_a = clusterizar(&a.glyphs);
    let clusters_b = clusterizar(&b.glyphs);

    let mut pairs = Vec::new();
    let mut unmatched_a = Vec::new();
    let mut unmatched_b = Vec::new();

    // Emparelha clusters pela ordem vertical; clusters a mais de um lado
    // contribuem todos os seus glifos para unmatched.
    let n = clusters_a.len().min(clusters_b.len());
    for i in 0..n {
        emparelhar_clusters(
            &a.glyphs,
            &clusters_a[i],
            &b.glyphs,
            &clusters_b[i],
            resolution,
            &mut pairs,
            &mut unmatched_a,
            &mut unmatched_b,
        );
    }
    for c in &clusters_a[n..] {
        unmatched_a.extend(c.iter().map(|&i| &a.glyphs[i]));
    }
    for c in &clusters_b[n..] {
        unmatched_b.extend(c.iter().map(|&i| &b.glyphs[i]));
    }

    let mut abs_dx: Vec<f64> = pairs.iter().map(|p| p.delta.0.abs()).collect();
    let mut abs_dy: Vec<f64> = pairs.iter().map(|p| p.delta.1.abs()).collect();
    let max_abs_dx = abs_dx.iter().copied().fold(0.0, f64::max);
    let max_abs_dy = abs_dy.iter().copied().fold(0.0, f64::max);
    let median_abs_dx = mediana(&mut abs_dx);
    let median_abs_dy = mediana(&mut abs_dy);

    ComparisonReport {
        pairs,
        unmatched_a,
        unmatched_b,
        median_abs_dx,
        median_abs_dy,
        max_abs_dx,
        max_abs_dy,
    }
}

/// Ordena os índices dos glifos por ordem de leitura (y, depois x) e agrupa
/// em clusters por proximidade vertical. Cada cluster é um `Vec` de índices
/// em ordem de leitura; os clusters vêm ordenados pelo y de referência.
fn clusterizar(glyphs: &[GlyphInstance]) -> Vec<Vec<usize>> {
    let mut ordem: Vec<usize> = (0..glyphs.len()).collect();
    ordem.sort_by(|&i, &j| {
        glyphs[i]
            .position
            .1
            .partial_cmp(&glyphs[j].position.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                glyphs[i]
                    .position
                    .0
                    .partial_cmp(&glyphs[j].position.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let mut clusters: Vec<Vec<usize>> = Vec::new();
    let mut y_referencia = 0.0;
    for i in ordem {
        let g = &glyphs[i];
        let novo_cluster = clusters.last().is_none()
            || (g.position.1 - y_referencia).abs() > 0.5 * g.font_size_pt;
        if novo_cluster {
            y_referencia = g.position.1;
            clusters.push(vec![i]);
        } else {
            clusters.last_mut().expect("cluster corrente existe").push(i);
        }
    }
    clusters
}

/// Emparelha os glifos de um par de clusters: primeiro por âncora textual
/// (LCS sobre a sequência expandida de codepoints), depois os glifos sem
/// codepoints restantes pela ordem horizontal.
#[allow(clippy::too_many_arguments)]
fn emparelhar_clusters<'a>(
    glyphs_a: &'a [GlyphInstance],
    cluster_a: &[usize],
    glyphs_b: &'a [GlyphInstance],
    cluster_b: &[usize],
    resolution: &MeasurementResolution,
    pairs: &mut Vec<GlyphPair<'a>>,
    unmatched_a: &mut Vec<&'a GlyphInstance>,
    unmatched_b: &mut Vec<&'a GlyphInstance>,
) {
    // Origem do cluster: posição do primeiro glifo em ordem de leitura.
    let origem_a = glyphs_a[cluster_a[0]].position;
    let origem_b = glyphs_b[cluster_b[0]].position;

    // Expansão a nível de codepoint (normalização de ligaduras, ADR 0001):
    // cada entrada é (codepoint, índice do glifo que o contribuiu).
    let texto_a = expandir(glyphs_a, cluster_a);
    let texto_b = expandir(glyphs_b, cluster_b);

    let alinhamento = lcs(&texto_a, &texto_b);

    // Agrupa o alinhamento em sequências contíguas nos dois lados; cada
    // sequência gera um par 1-para-N usando o primeiro glifo de cada lado.
    let mut emparelhados_a = vec![false; glyphs_a.len()];
    let mut emparelhados_b = vec![false; glyphs_b.len()];
    let mut pares_textuais: Vec<(usize, usize)> = Vec::new();

    let mut inicio = 0;
    while inicio < alinhamento.len() {
        let mut fim = inicio + 1;
        while fim < alinhamento.len()
            && alinhamento[fim].0 == alinhamento[fim - 1].0 + 1
            && alinhamento[fim].1 == alinhamento[fim - 1].1 + 1
        {
            fim += 1;
        }
        let sequencia = &alinhamento[inicio..fim];
        for &(ia, ib) in sequencia {
            emparelhados_a[texto_a[ia].1] = true;
            emparelhados_b[texto_b[ib].1] = true;
        }
        // Subdivisão da sequência em pares: um novo par inicia-se sempre que
        // **os dois lados** mudam de glifo ao mesmo tempo. Assim, glifos 1-1
        // geram um par cada (o deslocamento de um glifo individual é
        // observável no seu próprio par), enquanto uma ligadura 1-para-N
        // gera um único par (o lado da ligadura não muda de glifo).
        let mut segmento_inicio = 0;
        for k in 1..=sequencia.len() {
            let fronteira = k == sequencia.len()
                || (texto_a[sequencia[k].0].1 != texto_a[sequencia[k - 1].0].1
                    && texto_b[sequencia[k].1].1 != texto_b[sequencia[k - 1].1].1);
            if fronteira {
                let ga = texto_a[sequencia[segmento_inicio].0].1;
                let gb = texto_b[sequencia[segmento_inicio].1].1;
                // Dois segmentos podem partilhar o mesmo primeiro glifo
                // (trechos da mesma ligadura separados por falha de
                // alinhamento); nesse caso não se duplica o par.
                if !pares_textuais.contains(&(ga, gb)) {
                    pares_textuais.push((ga, gb));
                    pairs.push(emparelhar(
                        &glyphs_a[ga],
                        origem_a,
                        &glyphs_b[gb],
                        origem_b,
                        resolution,
                    ));
                }
                segmento_inicio = k;
            }
        }
        inicio = fim;
    }

    // Glifos sem âncora textual (codepoints: None) e não emparelhados:
    // emparelhamento posicional pela ordem horizontal dentro do cluster.
    let mut sem_ancora_a: Vec<usize> = cluster_a
        .iter()
        .copied()
        .filter(|&i| !emparelhados_a[i] && glyphs_a[i].codepoints.is_none())
        .collect();
    let mut sem_ancora_b: Vec<usize> = cluster_b
        .iter()
        .copied()
        .filter(|&i| !emparelhados_b[i] && glyphs_b[i].codepoints.is_none())
        .collect();
    sem_ancora_a.sort_by(|&i, &j| {
        glyphs_a[i].position.0.partial_cmp(&glyphs_a[j].position.0).unwrap_or(std::cmp::Ordering::Equal)
    });
    sem_ancora_b.sort_by(|&i, &j| {
        glyphs_b[i].position.0.partial_cmp(&glyphs_b[j].position.0).unwrap_or(std::cmp::Ordering::Equal)
    });
    let m = sem_ancora_a.len().min(sem_ancora_b.len());
    for k in 0..m {
        emparelhados_a[sem_ancora_a[k]] = true;
        emparelhados_b[sem_ancora_b[k]] = true;
        pairs.push(emparelhar(
            &glyphs_a[sem_ancora_a[k]],
            origem_a,
            &glyphs_b[sem_ancora_b[k]],
            origem_b,
            resolution,
        ));
    }

    // O que sobrou de cada lado fica sem par — registado, não julgado.
    for &i in cluster_a {
        if !emparelhados_a[i] {
            unmatched_a.push(&glyphs_a[i]);
        }
    }
    for &i in cluster_b {
        if !emparelhados_b[i] {
            unmatched_b.push(&glyphs_b[i]);
        }
    }
}

/// Expande um cluster a nível de codepoint: devolve pares
/// `(codepoint, índice_do_glifo)` em ordem de leitura.
fn expandir(glyphs: &[GlyphInstance], cluster: &[usize]) -> Vec<(char, usize)> {
    let mut texto = Vec::new();
    for &i in cluster {
        if let Some(codepoints) = &glyphs[i].codepoints {
            for &c in codepoints {
                texto.push((c, i));
            }
        }
    }
    texto
}

/// Alinhamento LCS próprio (programação dinâmica) sobre as sequências de
/// codepoints. Devolve pares `(índice_em_a, índice_em_b)` em ordem crescente.
/// Implementação simples — os clusters são linhas, logo as sequências são
/// curtas; evita a dependência externa `similar`.
fn lcs(a: &[(char, usize)], b: &[(char, usize)]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    // dp[i][j] = comprimento do LCS de a[i..] e b[j..]
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i].0 == b[j].0 {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let mut pares = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i].0 == b[j].0 {
            pares.push((i, j));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pares
}

/// Constrói um `GlyphPair`: delta `b − a` em coordenadas relativas à origem
/// de cada cluster, e veredicto da resolução usando o tamanho de fonte de A.
fn emparelhar<'a>(
    a: &'a GlyphInstance,
    origem_a: (f64, f64),
    b: &'a GlyphInstance,
    origem_b: (f64, f64),
    resolution: &MeasurementResolution,
) -> GlyphPair<'a> {
    let rel_a = (a.position.0 - origem_a.0, a.position.1 - origem_a.1);
    let rel_b = (b.position.0 - origem_b.0, b.position.1 - origem_b.1);
    let delta = (rel_b.0 - rel_a.0, rel_b.1 - rel_a.1);
    let within_resolution =
        resolution.is_within(delta.0, a.font_size_pt) && resolution.is_within(delta.1, a.font_size_pt);
    GlyphPair { a, b, delta, within_resolution }
}

/// Mediana de uma fatia (modificada in-place pela ordenação). Vazio → 0.0.
fn mediana(valores: &mut [f64]) -> f64 {
    if valores.is_empty() {
        return 0.0;
    }
    valores.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    let meio = valores.len() / 2;
    if valores.len() % 2 == 1 {
        valores[meio]
    } else {
        (valores[meio - 1] + valores[meio]) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::PageGeometry;

    fn glifo(x: f64, y: f64, codepoints: Option<Vec<char>>) -> GlyphInstance {
        GlyphInstance {
            position: (x, y),
            codepoints,
            font_size_pt: 12.0,
            font_ref: "F1".to_string(),
        }
    }

    fn doc(glyphs: Vec<GlyphInstance>) -> DocumentGeometry {
        DocumentGeometry {
            page: PageGeometry { width: 595.0, height: 842.0 },
            glyphs,
        }
    }

    fn tolerancia() -> MeasurementResolution {
        MeasurementResolution::Absolute { tolerance_pt: 0.5 }
    }

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn documentos_identicos_emparelham_tudo_com_delta_zero() {
        let glyphs = vec![
            glifo(100.0, 700.0, Some(vec!['o'])),
            glifo(110.0, 700.0, Some(vec!['l'])),
            glifo(118.0, 700.0, Some(vec!['a'])),
        ];
        let a = doc(glyphs.clone());
        let b = doc(glyphs);
        let r = compare(&a, &b, &tolerancia());
        assert!(r.unmatched_a.is_empty());
        assert!(r.unmatched_b.is_empty());
        assert_eq!(r.pairs.len(), 3); // um par por glifo 1-1
        assert!(r.pairs.iter().all(|p| p.delta == (0.0, 0.0)));
        assert_eq!(r.median_abs_dx, 0.0);
        assert_eq!(r.median_abs_dy, 0.0);
        assert_eq!(r.max_abs_dx, 0.0);
        assert_eq!(r.max_abs_dy, 0.0);
    }

    #[test]
    fn documentos_vazios_produzem_relatorio_vazio() {
        let a = doc(vec![]);
        let b = doc(vec![]);
        let r = compare(&a, &b, &tolerancia());
        assert!(r.pairs.is_empty());
        assert_eq!(r.median_abs_dx, 0.0);
        assert_eq!(r.max_abs_dx, 0.0);
    }

    #[test]
    fn glifo_deslocado_5pt_nao_contamina_origem_do_cluster() {
        // Critério de verificação da spec: um glifo deslocado 5pt em x dentro
        // de um cluster com outros glifos inalterados — só esse par tem
        // delta.0 ≈ 5.0; os restantes pares do mesmo cluster ficam ~0.
        let a = doc(vec![
            glifo(100.0, 700.0, Some(vec!['a'])),
            glifo(120.0, 700.0, Some(vec!['b'])),
            glifo(140.0, 700.0, Some(vec!['c'])),
        ]);
        let b = doc(vec![
            glifo(100.0, 700.0, Some(vec!['a'])),
            glifo(125.0, 700.0, Some(vec!['b'])), // +5pt em x
            glifo(140.0, 700.0, Some(vec!['c'])),
        ]);
        let r = compare(&a, &b, &tolerancia());
        assert!(r.unmatched_a.is_empty() && r.unmatched_b.is_empty());
        assert_eq!(r.pairs.len(), 3);
        let deslocados: Vec<_> = r.pairs.iter().filter(|p| approx(p.delta.0, 5.0)).collect();
        assert_eq!(deslocados.len(), 1);
        assert!(!deslocados[0].within_resolution); // 5pt > 0.5pt
        let quietos = r.pairs.iter().filter(|p| approx(p.delta.0, 0.0)).count();
        assert_eq!(quietos, 2);
        assert!(r.pairs.iter().all(|p| approx(p.delta.1, 0.0)));
        assert!(approx(r.max_abs_dx, 5.0));
        assert!(approx(r.median_abs_dx, 0.0)); // mediana robusta ao outlier (lição 3)
    }

    #[test]
    fn glifo_deslocado_5pt_em_linha_propria_tem_delta_5_e_vizinhos_zero() {
        // Três linhas (clusters), duas letras cada; a segunda letra da linha
        // do meio desloca 5pt em x. Como a origem de cada cluster é o
        // primeiro glifo da linha (inalterado), o deslocamento é observável
        // e as linhas vizinhas mantêm delta 0 — a origem de cada cluster não
        // é contaminada pelo deslocamento alheio.
        let a = doc(vec![
            glifo(100.0, 700.0, Some(vec!['a'])),
            glifo(110.0, 700.0, Some(vec!['b'])),
            glifo(100.0, 720.0, Some(vec!['c'])),
            glifo(110.0, 720.0, Some(vec!['d'])),
            glifo(100.0, 740.0, Some(vec!['e'])),
            glifo(110.0, 740.0, Some(vec!['f'])),
        ]);
        let b = doc(vec![
            glifo(100.0, 700.0, Some(vec!['a'])),
            glifo(110.0, 700.0, Some(vec!['b'])),
            glifo(100.0, 720.0, Some(vec!['c'])),
            glifo(115.0, 720.0, Some(vec!['d'])), // +5pt em x
            glifo(100.0, 740.0, Some(vec!['e'])),
            glifo(110.0, 740.0, Some(vec!['f'])),
        ]);
        let r = compare(&a, &b, &tolerancia());
        assert_eq!(r.pairs.len(), 6);
        let deslocado = r.pairs.iter().filter(|p| approx(p.delta.0, 5.0)).count();
        let quietos = r.pairs.iter().filter(|p| approx(p.delta.0, 0.0)).count();
        assert_eq!(deslocado, 1);
        assert_eq!(quietos, 5);
        assert!(approx(r.max_abs_dx, 5.0));
        assert!(approx(r.median_abs_dx, 0.0)); // mediana robusta ao outlier (lição 3)
        assert!(!r.pairs.iter().find(|p| approx(p.delta.0, 5.0)).unwrap().within_resolution);
    }

    #[test]
    fn ligadura_fi_um_glifo_emparelha_com_f_mais_i_dois_glifos() {
        // A: ligadura "fi" (1 glifo, 2 codepoints) em x=100.
        // B: "f" + "i" (2 glifos) a começar na mesma posição.
        let a = doc(vec![glifo(100.0, 700.0, Some(vec!['f', 'i']))]);
        let b = doc(vec![
            glifo(100.0, 700.0, Some(vec!['f'])),
            glifo(106.0, 700.0, Some(vec!['i'])),
        ]);
        let r = compare(&a, &b, &tolerancia());
        assert!(r.unmatched_a.is_empty(), "nenhum glifo de A sem par");
        assert!(r.unmatched_b.is_empty(), "nenhum glifo de B sem par");
        assert_eq!(r.pairs.len(), 1); // par 1-para-N, primeiro glifo de cada lado
        assert!(approx(r.pairs[0].delta.0, 0.0));
        assert!(r.pairs[0].within_resolution);
    }

    #[test]
    fn conteudo_divergente_fica_unmatched_dos_dois_lados() {
        let a = doc(vec![glifo(100.0, 700.0, Some(vec!['a']))]);
        let b = doc(vec![
            glifo(100.0, 700.0, Some(vec!['a'])),
            glifo(120.0, 700.0, Some(vec!['z'])), // conteúdo a mais em B
        ]);
        let r = compare(&a, &b, &tolerancia());
        assert!(r.unmatched_a.is_empty());
        assert_eq!(r.unmatched_b.len(), 1);
    }

    #[test]
    fn glifos_sem_codepoints_emparelham_posicionalmente() {
        let a = doc(vec![
            glifo(100.0, 700.0, Some(vec!['a'])),
            glifo(200.0, 700.0, None),
        ]);
        let b = doc(vec![
            glifo(100.0, 700.0, Some(vec!['a'])),
            glifo(201.0, 700.0, None),
        ]);
        let r = compare(&a, &b, &tolerancia());
        assert!(r.unmatched_a.is_empty() && r.unmatched_b.is_empty());
        assert_eq!(r.pairs.len(), 2);
        let posicional = r.pairs.iter().find(|p| p.a.codepoints.is_none()).unwrap();
        assert!(approx(posicional.delta.0, 1.0));
    }

    #[test]
    fn tolerancia_relativa_ao_em_usa_font_size_do_glifo() {
        // Linha com dois glifos; o segundo desloca 0.15pt em x. (Com um glifo
        // único por cluster a origem seria auto-referente e o delta seria 0.)
        let mut base = |x2: f64| {
            let mut g1 = glifo(100.0, 700.0, Some(vec!['a']));
            let mut g2 = glifo(x2, 700.0, Some(vec!['b']));
            g1.font_size_pt = 10.0;
            g2.font_size_pt = 10.0;
            doc(vec![g1, g2])
        };
        let a = base(120.0);
        let b = base(120.15);
        let par_b = |r: &ComparisonReport| {
            r.pairs
                .iter()
                .find(|p| p.a.codepoints.as_deref() == Some(&['b'][..]))
                .expect("par do glifo 'b' existe")
                .within_resolution
        };
        let rel = MeasurementResolution::RelativeToEm { fraction: 0.02 }; // tol = 0.2pt
        assert!(par_b(&compare(&a, &b, &rel))); // 0.15 <= 0.2
        let apertada = MeasurementResolution::RelativeToEm { fraction: 0.01 }; // tol = 0.1pt
        assert!(!par_b(&compare(&a, &b, &apertada))); // 0.15 > 0.1
    }

    #[test]
    fn ordem_de_emissao_diferente_emparelha_por_ordem_de_leitura() {
        // Mesmo conteúdo visual, ordem de emissão invertida no content stream.
        let a = doc(vec![
            glifo(100.0, 700.0, Some(vec!['a'])),
            glifo(120.0, 700.0, Some(vec!['b'])),
        ]);
        let b = doc(vec![
            glifo(120.0, 700.0, Some(vec!['b'])),
            glifo(100.0, 700.0, Some(vec!['a'])),
        ]);
        let r = compare(&a, &b, &tolerancia());
        assert!(r.unmatched_a.is_empty() && r.unmatched_b.is_empty());
        assert!(r.pairs.iter().all(|p| p.delta == (0.0, 0.0)));
    }
}
