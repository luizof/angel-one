//! Uso: cargo run --release --example route -- <lat> <lon> <lat> <lon> [<lat> <lon> ...]
//! O primeiro par é a origem; os demais são destinos. Imprime uma linha por
//! destino: `via_water distance_m straight_m ms | lat,lon;lat,lon;...`.
use std::time::Instant;
use water_routing::WaterGrid;

fn main() {
    let nums: Vec<f64> = std::env::args()
        .skip(1)
        .map(|a| a.parse().expect("número inválido"))
        .collect();
    if nums.len() < 4 || !nums.len().is_multiple_of(2) {
        eprintln!("uso: route <lat> <lon> <lat> <lon> [...]");
        std::process::exit(2);
    }
    let pts: Vec<(f64, f64)> = nums.chunks(2).map(|c| (c[0], c[1])).collect();
    let grid = WaterGrid::embedded();
    let t0 = Instant::now();
    let rs = grid.routes_from(pts[0], &pts[1..]);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    for r in rs {
        let path: Vec<String> = r.path.iter().map(|p| format!("{:.6},{:.6}", p.0, p.1)).collect();
        println!(
            "{} {:.1} {:.1} {:.2} | {}",
            r.via_water,
            r.distance_m,
            r.straight_m,
            ms,
            path.join(";")
        );
    }
}
