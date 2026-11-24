use std::time::Instant;
use water_routing::{haversine_m, RouteResult, WaterGrid, MAX_PATH_POINTS, SNAP_RADIUS_M};

const JURERE: (f64, f64) = (-27.4365, -48.4990);
const ARVOREDO_SUL: (f64, f64) = (-27.2950, -48.3610);
const SANTO_ANTONIO: (f64, f64) = (-27.5070, -48.5200);
const INGLESES: (f64, f64) = (-27.4330, -48.3930);

fn assert_send_sync<T: Send + Sync>() {}

/// Todos os segmentos do caminho passam só por água (amostra a cada ~10 m),
/// exceto as pernas do snap (origem/destino reais em terra até a água).
fn assert_path_in_water(g: &WaterGrid, r: &RouteResult, origin: (f64, f64), target: (f64, f64)) {
    let p = &r.path;
    assert!(p.len() >= 2);
    assert_eq!(p[0], origin);
    assert_eq!(*p.last().unwrap(), target);
    let n = p.len();
    let skip_first = !g.is_water(origin.0, origin.1);
    let skip_last = !g.is_water(target.0, target.1);
    if skip_first {
        let leg = haversine_m(p[0], p[1]);
        println!("  perna do snap na origem: {leg:.0} m");
        assert!(leg <= SNAP_RADIUS_M, "perna do snap da origem longa demais");
    }
    if skip_last {
        let leg = haversine_m(p[n - 2], p[n - 1]);
        println!("  perna do snap no destino: {leg:.0} m");
        assert!(leg <= SNAP_RADIUS_M, "perna do snap do destino longa demais");
    }
    let mut samples = 0;
    for i in 0..n - 1 {
        if (i == 0 && skip_first) || (i == n - 2 && skip_last) {
            continue;
        }
        let (a, b) = (p[i], p[i + 1]);
        let len = haversine_m(a, b);
        let steps = (len / 10.0).ceil().max(1.0) as usize;
        for k in 0..=steps {
            let t = k as f64 / steps as f64;
            let q = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            assert!(g.is_water(q.0, q.1), "segmento {i} passa por terra em {q:?} ({a:?} -> {b:?})");
            samples += 1;
        }
    }
    assert!(samples > 0);
}

fn show(name: &str, r: &RouteResult) {
    println!(
        "{name}: via_water={} dist={:.0} m reta={:.0} m razão={:.3} pontos={}",
        r.via_water,
        r.distance_m,
        r.straight_m,
        r.distance_m / r.straight_m,
        r.path.len()
    );
}

#[test]
fn t1_loads_embedded_grid() {
    assert_send_sync::<WaterGrid>();
    let g = WaterGrid::embedded();
    assert!(std::ptr::eq(g, WaterGrid::embedded()), "carregada uma vez só");
    assert!(g.contains(-27.40, -48.50));
    assert!(!g.contains(-27.00, -48.50));
    assert!(g.is_water(-27.38, -48.45), "mar aberto ao norte de Canasvieiras");
    assert!(g.is_water(-27.54, -48.57), "Baía Norte");
    assert!(!g.is_water(-27.50, -48.45), "interior da Ilha");
    assert!(!g.is_water(-27.45, -48.66), "continente");
    // load() lê o mesmo arquivo
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/water_grid.bin");
    let g2 = WaterGrid::load(&path).expect("load");
    assert!(g2.is_water(-27.38, -48.45));
    println!("{g:?}");
}

#[test]
fn t2_jurere_to_arvoredo() {
    let g = WaterGrid::embedded();
    let r = &g.routes_from(JURERE, &[ARVOREDO_SUL])[0];
    show("Jurerê → Arvoredo (sul)", r);
    assert!(r.via_water);
    assert!(r.distance_m >= r.straight_m);
    assert!(r.distance_m < 1.6 * r.straight_m);
    assert_path_in_water(g, r, JURERE, ARVOREDO_SUL);
}

#[test]
fn t3_jurere_to_santo_antonio_around_ponta_do_forte() {
    let g = WaterGrid::embedded();
    let r = &g.routes_from(JURERE, &[SANTO_ANTONIO])[0];
    show("Jurerê → Santo Antônio de Lisboa", r);
    assert!(r.via_water);
    assert!(r.distance_m > r.straight_m * 1.05, "tem que contornar a ponta");
    assert_path_in_water(g, r, JURERE, SANTO_ANTONIO);
    // contorna pelo oeste: algum ponto a oeste do Pontal da Daniela
    assert!(r.path.iter().any(|p| p.1 < -48.535), "deveria entrar na Baía Norte pelo oeste");
}

#[test]
fn t4_jurere_to_ingleses_around_north_tip() {
    let g = WaterGrid::embedded();
    let r = &g.routes_from(JURERE, &[INGLESES])[0];
    show("Jurerê → Ingleses", r);
    assert!(r.via_water);
    assert!(r.distance_m > r.straight_m * 1.1, "tem que contornar a ponta norte");
    assert_path_in_water(g, r, JURERE, INGLESES);
    // passa ao norte da Ponta das Canas/Lagoinha
    assert!(r.path.iter().any(|p| p.0 > -27.39), "deveria subir até a ponta norte da Ilha");
}

#[test]
fn t5_outside_bbox_falls_back() {
    let g = WaterGrid::embedded();
    let fora = (-27.00, -48.00);
    // origem fora
    let rs = g.routes_from(fora, &[JURERE, INGLESES]);
    for (r, t) in rs.iter().zip([JURERE, INGLESES]) {
        assert!(!r.via_water);
        assert_eq!(r.distance_m, r.straight_m);
        assert_eq!(r.path, vec![fora, t]);
    }
    // destino fora (os outros seguem normais)
    let rs = g.routes_from(JURERE, &[fora, INGLESES]);
    assert!(!rs[0].via_water);
    assert_eq!(rs[0].distance_m, rs[0].straight_m);
    assert_eq!(rs[0].path, vec![JURERE, fora]);
    assert!(rs[1].via_water);
    // terra longe da água (> 1,5 km): centro da Ilha
    let r = &g.routes_from(JURERE, &[(-27.5000, -48.4500)])[0];
    assert!(!r.via_water);
    // NaN não quebra
    let r = &g.routes_from((f64::NAN, -48.5), &[JURERE])[0];
    assert!(!r.via_water);
    // lista vazia
    assert!(g.routes_from(JURERE, &[]).is_empty());
}

#[test]
fn t6_same_order_and_direct_line() {
    let g = WaterGrid::embedded();
    let a = (-27.40, -48.47); // água aberta
    let b = (-27.39, -48.46); // à vista
    let rs = g.routes_from(a, &[INGLESES, b, SANTO_ANTONIO, a]);
    assert_eq!(rs.len(), 4);
    assert!((rs[1].distance_m - haversine_m(a, b)).abs() < 1e-6, "linha reta na água");
    assert_eq!(rs[1].path, vec![a, b]);
    assert_eq!(rs[3].distance_m, 0.0);
    let solo = g.routes_from(a, &[SANTO_ANTONIO]);
    assert!((solo[0].distance_m - rs[2].distance_m).abs() < 1e-6);
    for r in &rs {
        assert!(r.path.len() <= MAX_PATH_POINTS);
    }
}

#[test]
fn t7_timing_20_targets() {
    let g = WaterGrid::embedded();
    let targets: Vec<(f64, f64)> = vec![
        ARVOREDO_SUL,
        SANTO_ANTONIO,
        INGLESES,
        (-27.4520, -48.5350), // Daniela
        (-27.4320, -48.5170), // Forte
        (-27.4260, -48.4620), // Canasvieiras
        (-27.3920, -48.4330), // Ponta das Canas
        (-27.3880, -48.4220), // Lagoinha
        (-27.3980, -48.4140), // Praia Brava
        (-27.4630, -48.3800), // Santinho
        (-27.4260, -48.5630), // Anhatomirim
        (-27.4740, -48.5620), // Ratones Grande
        (-27.4900, -48.5280), // Sambaqui
        (-27.5300, -48.5250), // Cacupé
        (-27.5800, -48.5450), // Beira-Mar Norte
        (-27.3200, -48.5500), // Ganchos
        (-27.3500, -48.5250), // Armação da Piedade
        (-27.2050, -48.5250), // Zimbros
        (-27.2700, -48.3310), // Ilha Deserta
        (-27.6300, -48.5750), // Baía Sul (depois das pontes)
    ];
    assert_eq!(targets.len(), 20);
    for (label, origin) in [("Jurerê", JURERE), ("pontes (pior caso)", (-27.5950, -48.5640))] {
        let _ = g.routes_from(origin, &targets); // aquece
        let runs = 10;
        let mut best = f64::MAX;
        let mut total = 0.0;
        let mut rs = Vec::new();
        for _ in 0..runs {
            let t0 = Instant::now();
            rs = g.routes_from(origin, &targets);
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            best = best.min(ms);
            total += ms;
        }
        println!(
            "origem {label}: 20 alvos em {:.1} ms (melhor) / {:.1} ms (média de {runs})",
            best,
            total / runs as f64
        );
        let ok = rs.iter().filter(|r| r.via_water).count();
        for (t, r) in targets.iter().zip(&rs) {
            println!(
                "  {:?}: via_water={} {:.0} m (reta {:.0} m, {} pts)",
                t,
                r.via_water,
                r.distance_m,
                r.straight_m,
                r.path.len()
            );
        }
        assert!(ok >= 18, "quase todos os destinos deveriam ter rota pela água ({ok}/20)");
        if !cfg!(debug_assertions) {
            assert!(best < 60.0, "lento demais: {best:.1} ms");
        }
    }
}
