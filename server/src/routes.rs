use std::sync::OnceLock;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use water_routing::WaterGrid;

use crate::destinations;
use crate::error::{ApiError, ApiResult};
use crate::extract::QueryParams;
use crate::state::AppState;
use crate::util::{check_lat_lon, round_to};

static GRID: OnceLock<&'static WaterGrid> = OnceLock::new();

/// Malha de água: `WATER_GRID` (se definida e válida) ou a embutida no crate.
pub fn grid(path: Option<&std::path::Path>) -> &'static WaterGrid {
    GRID.get_or_init(|| {
        if let Some(p) = path {
            match WaterGrid::load(p) {
                Ok(g) => {
                    tracing::info!("malha de água carregada de {}", p.display());
                    return Box::leak(Box::new(g));
                }
                Err(e) => tracing::error!("WATER_GRID {} inválida ({e}); usando a embutida", p.display()),
            }
        }
        WaterGrid::embedded()
    })
}

#[derive(Deserialize)]
pub struct RouteQuery {
    lat: f64,
    lon: f64,
}

#[derive(Serialize)]
pub struct RouteOut {
    pub destination_id: i64,
    pub distance_m: f64,
    pub straight_m: f64,
    pub via_water: bool,
    pub path: Vec<[f64; 2]>,
}

/// `GET /api/routes?lat=..&lon=..` — rotas da origem até todos os destinos.
pub async fn get(
    State(st): State<AppState>,
    QueryParams(q): QueryParams<RouteQuery>,
) -> ApiResult<Json<Vec<RouteOut>>> {
    check_lat_lon(q.lat, q.lon)?;
    let dests = st.db.call(|c| Ok(destinations::all_points(c)?)).await?;
    let grid_path = st.cfg.water_grid.clone();
    let origin = (q.lat, q.lon);
    let out = tokio::task::spawn_blocking(move || {
        let g = grid(grid_path.as_deref());
        let targets: Vec<(f64, f64)> = dests.iter().map(|d| (d.1, d.2)).collect();
        let results = g.routes_from(origin, &targets);
        dests
            .iter()
            .zip(results)
            .map(|(d, r)| {
                let (path, distance) = if r.via_water && r.path.len() >= 2 {
                    (r.path, r.distance_m)
                } else {
                    (vec![origin, (d.1, d.2)], r.straight_m)
                };
                RouteOut {
                    destination_id: d.0,
                    distance_m: round_to(distance, 1),
                    straight_m: round_to(r.straight_m, 1),
                    via_water: r.via_water,
                    path: path.into_iter().map(|(la, lo)| [round_to(la, 6), round_to(lo, 6)]).collect(),
                }
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|_| ApiError::internal("Falha ao calcular as rotas"))?;
    Ok(Json(out))
}
