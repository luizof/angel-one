//! `water_routing` — distância pela água, desviando de terra.
//!
//! A malha `assets/water_grid.bin` (gerada por `app/tools/build_water_grid.py`)
//! é um bitset de células de ~40 m sobre o bbox lat -27.66..-27.18,
//! lon -48.72..-48.26 (norte da Ilha de Santa Catarina, Baía Norte e
//! continente). 1 = água navegável (já com 1 célula de margem da costa).
//!
//! [`WaterGrid::routes_from`] roda **um** Dijkstra 8-conexo a partir da origem
//! e para quando todos os alvos foram resolvidos. Pontos em terra/na costa são
//! levados ("snap") para a água mais próxima num raio de 1,5 km; o caminho é
//! suavizado por linha de visada e a distância é a soma dos segmentos
//! (haversine), incluindo as pernas do snap.
//!
//! Formato do arquivo (little-endian): `b"AOWG"`, `u32` versão = 1,
//! `f64` lat_min, lat_max, lon_min, lon_max, `u32` rows, cols, depois o
//! bitset row-major (linha 0 = sul, coluna 0 = oeste), bit LSB-first.

use std::io;
use std::path::Path;
use std::sync::OnceLock;

/// Raio médio da Terra usado em [`haversine_m`] e nos custos da malha.
pub const EARTH_RADIUS_M: f64 = 6_371_008.8;
/// Raio máximo para levar origem/destino até a água.
pub const SNAP_RADIUS_M: f64 = 1_500.0;
/// Número máximo de pontos em [`RouteResult::path`].
pub const MAX_PATH_POINTS: usize = 80;

const M_PER_DEG: f64 = EARTH_RADIUS_M * std::f64::consts::PI / 180.0;
const MAGIC: &[u8; 4] = b"AOWG";
const VERSION: u32 = 1;
const HEADER_LEN: usize = 48;
/// Limite de sanidade para não alocar absurdos com um arquivo corrompido.
const MAX_CELLS: usize = 64 * 1024 * 1024;

static EMBEDDED_BYTES: &[u8] = include_bytes!("../assets/water_grid.bin");
static EMBEDDED: OnceLock<WaterGrid> = OnceLock::new();

/// Malha de água em memória (imutável; `Send + Sync`).
pub struct WaterGrid {
    lat_min: f64,
    lat_max: f64,
    lon_min: f64,
    lon_max: f64,
    rows: usize,
    cols: usize,
    dlat: f64,
    dlon: f64,
    /// Largura da linha acolchoada (`cols + 2`).
    stride: usize,
    /// `(rows + 2) * (cols + 2)` bytes, 1 = água; a moldura é terra, então
    /// vizinhos de uma célula de água nunca saem do vetor.
    mask: Vec<u8>,
    /// Custo (dm) de um passo norte/sul.
    cost_v: u32,
    /// Custo (dm) de um passo leste/oeste, por linha acolchoada.
    cost_h: Vec<u32>,
    /// Custo (dm) de um passo diagonal entre a linha acolchoada `i` e `i + 1`.
    cost_d: Vec<u32>,
}

/// Rota de uma origem até um destino.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteResult {
    /// Distância pela água (ou = `straight_m` no fallback), em metros.
    pub distance_m: f64,
    /// Distância em linha reta (haversine) origem → destino, em metros.
    pub straight_m: f64,
    /// `false` = fallback em linha reta (fora da malha, sem água em 1,5 km
    /// ou sem caminho).
    pub via_water: bool,
    /// `(lat, lon)`; começa na origem e termina no destino reais. No máximo
    /// [`MAX_PATH_POINTS`] pontos.
    pub path: Vec<(f64, f64)>,
}

impl RouteResult {
    fn fallback(origin: (f64, f64), target: (f64, f64)) -> RouteResult {
        let straight_m = haversine_m(origin, target);
        RouteResult {
            distance_m: straight_m,
            straight_m,
            via_water: false,
            path: vec![origin, target],
        }
    }
}

/// Distância de grande círculo entre dois `(lat, lon)` em graus, em metros.
pub fn haversine_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (lat1, lat2) = (a.0.to_radians(), b.0.to_radians());
    let dlat = lat2 - lat1;
    let dlon = (b.1 - a.1).to_radians();
    let s1 = (dlat * 0.5).sin();
    let s2 = (dlon * 0.5).sin();
    let h = s1 * s1 + lat1.cos() * lat2.cos() * s2 * s2;
    2.0 * EARTH_RADIUS_M * h.sqrt().min(1.0).asin()
}

/// Ponto já levado à água.
#[derive(Clone, Copy)]
struct Snap {
    /// Índice acolchoado da célula de água.
    idx: usize,
    /// `(lat, lon)` na água: o próprio ponto, ou o centro da célula escolhida.
    water_pt: (f64, f64),
    /// O mesmo ponto em coordenadas da malha `(x = coluna, y = linha)`.
    grid: (f64, f64),
    /// `true` se o ponto real estava em terra e foi movido.
    moved: bool,
}

// Direções: N, S, L, O, NE, NO, SE, SO (N = linha + 1).
const DIR_NONE: u8 = 8;
const SETTLED: u8 = 0x80;

impl WaterGrid {
    /// Malha embutida no binário (`assets/water_grid.bin`), carregada uma vez.
    pub fn embedded() -> &'static WaterGrid {
        EMBEDDED.get_or_init(|| {
            WaterGrid::from_bytes(EMBEDDED_BYTES).expect("water_grid.bin embutido inválido")
        })
    }

    /// Lê uma malha no formato `AOWG` v1.
    pub fn from_bytes(bytes: &[u8]) -> io::Result<WaterGrid> {
        let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, format!("water_grid: {m}"));
        if bytes.len() < HEADER_LEN {
            return Err(bad("arquivo menor que o cabeçalho"));
        }
        if &bytes[0..4] != MAGIC {
            return Err(bad("assinatura AOWG ausente"));
        }
        let u32_at = |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        let f64_at = |o: usize| {
            let mut b = [0u8; 8];
            b.copy_from_slice(&bytes[o..o + 8]);
            f64::from_le_bytes(b)
        };
        let version = u32_at(4);
        if version != VERSION {
            return Err(bad(&format!("versão {version} não suportada")));
        }
        let (lat_min, lat_max, lon_min, lon_max) = (f64_at(8), f64_at(16), f64_at(24), f64_at(32));
        let (rows, cols) = (u32_at(40) as usize, u32_at(44) as usize);
        let finite = [lat_min, lat_max, lon_min, lon_max].iter().all(|v| v.is_finite());
        if !finite || lat_min >= lat_max || lon_min >= lon_max {
            return Err(bad("bbox inválido"));
        }
        if !(-90.0..=90.0).contains(&lat_min) || !(-90.0..=90.0).contains(&lat_max) {
            return Err(bad("latitude fora de -90..90"));
        }
        if rows == 0 || cols == 0 || rows.saturating_mul(cols) > MAX_CELLS {
            return Err(bad("dimensões inválidas"));
        }
        let ncells = rows * cols;
        let bits = &bytes[HEADER_LEN..];
        if bits.len() < ncells.div_ceil(8) {
            return Err(bad("bitset truncado"));
        }

        let stride = cols + 2;
        let mut mask = vec![0u8; (rows + 2) * stride];
        for r in 0..rows {
            let base = (r + 1) * stride + 1;
            let bit0 = r * cols;
            for c in 0..cols {
                let i = bit0 + c;
                mask[base + c] = (bits[i >> 3] >> (i & 7)) & 1;
            }
        }

        let dlat = (lat_max - lat_min) / rows as f64;
        let dlon = (lon_max - lon_min) / cols as f64;
        let cell_h = dlat * M_PER_DEG;
        // linha acolchoada pr ↔ linha real pr - 1
        let width_at = |pr: usize| -> f64 {
            let lat = lat_min + (pr as f64 - 0.5) * dlat;
            dlon * M_PER_DEG * lat.to_radians().cos()
        };
        // custos inteiros em decímetros (erro de arredondamento < 0,2 %; só
        // afeta a escolha do caminho, a distância final é haversine)
        let dm = |m: f64| (m * 10.0).round().max(1.0) as u32;
        let cost_h: Vec<u32> = (0..rows + 2).map(|pr| dm(width_at(pr))).collect();
        let cost_d: Vec<u32> = (0..rows + 2)
            .map(|pr| {
                let w = 0.5 * (width_at(pr) + width_at(pr + 1));
                dm((cell_h * cell_h + w * w).sqrt())
            })
            .collect();
        let max_cost = cost_d.iter().chain(cost_h.iter()).copied().max().unwrap_or(1).max(dm(cell_h));
        if max_cost as usize >= BUCKETS {
            return Err(bad("células grandes demais para a fila de prioridade"));
        }

        Ok(WaterGrid {
            lat_min,
            lat_max,
            lon_min,
            lon_max,
            rows,
            cols,
            dlat,
            dlon,
            stride,
            mask,
            cost_v: dm(cell_h),
            cost_h,
            cost_d,
        })
    }

    /// Lê a malha de um arquivo.
    pub fn load(path: &Path) -> io::Result<WaterGrid> {
        WaterGrid::from_bytes(&std::fs::read(path)?)
    }

    /// `true` se o ponto está dentro do bbox da malha.
    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        lat >= self.lat_min && lat <= self.lat_max && lon >= self.lon_min && lon <= self.lon_max
    }

    /// `true` se o ponto cai numa célula de água navegável.
    pub fn is_water(&self, lat: f64, lon: f64) -> bool {
        match self.cell_of(lat, lon) {
            Some((r, c)) => self.mask[self.pidx(r, c)] == 1,
            None => false,
        }
    }

    /// Rotas da origem até cada alvo, na mesma ordem de `targets`.
    pub fn routes_from(&self, origin: (f64, f64), targets: &[(f64, f64)]) -> Vec<RouteResult> {
        let mut out: Vec<RouteResult> =
            targets.iter().map(|&t| RouteResult::fallback(origin, t)).collect();
        if targets.is_empty() {
            return out;
        }
        let Some(os) = self.snap(origin) else {
            return out;
        };
        let snaps: Vec<Option<Snap>> = targets.iter().map(|&t| self.snap(t)).collect();

        // Alvos à vista não precisam do Dijkstra.
        let mut pending: Vec<usize> = Vec::new();
        for (i, ts) in snaps.iter().enumerate() {
            let Some(ts) = ts else { continue };
            if self.line_of_sight(os.grid, ts.grid) {
                out[i] = self.finish(origin, &os, vec![os.water_pt, ts.water_pt], ts, targets[i]);
            } else {
                pending.push(i);
            }
        }
        if pending.is_empty() {
            return out;
        }

        let mut cells: Vec<usize> = pending.iter().map(|&i| snaps[i].unwrap().idx).collect();
        cells.sort_unstable();
        cells.dedup();
        let search = self.dijkstra(os.idx, &cells);

        for &i in &pending {
            let ts = snaps[i].unwrap();
            if search.dist[ts.idx] == u32::MAX {
                continue; // sem caminho → fallback
            }
            let cells = search.path_to(ts.idx, self.stride);
            let mut pts: Vec<(f64, f64)> = Vec::with_capacity(cells.len() + 1);
            pts.push(os.grid);
            if cells.len() > 2 {
                for &idx in &cells[1..cells.len() - 1] {
                    pts.push(self.center_grid(idx));
                }
            }
            pts.push(ts.grid);
            let water: Vec<(f64, f64)> =
                self.string_pull(&pts).into_iter().map(|p| self.grid_to_latlon(p)).collect();
            out[i] = self.finish(origin, &os, water, &ts, targets[i]);
        }
        out
    }

    // ---- interno -------------------------------------------------------

    #[inline]
    fn pidx(&self, r: usize, c: usize) -> usize {
        (r + 1) * self.stride + c + 1
    }

    /// `(x = coluna, y = linha)` contínuos.
    #[inline]
    fn to_grid(&self, lat: f64, lon: f64) -> (f64, f64) {
        ((lon - self.lon_min) / self.dlon, (lat - self.lat_min) / self.dlat)
    }

    #[inline]
    fn grid_to_latlon(&self, g: (f64, f64)) -> (f64, f64) {
        (self.lat_min + g.1 * self.dlat, self.lon_min + g.0 * self.dlon)
    }

    fn cell_of(&self, lat: f64, lon: f64) -> Option<(usize, usize)> {
        if !self.contains(lat, lon) {
            return None;
        }
        let (x, y) = self.to_grid(lat, lon);
        let r = (y.floor().max(0.0) as usize).min(self.rows - 1);
        let c = (x.floor().max(0.0) as usize).min(self.cols - 1);
        Some((r, c))
    }

    /// Centro (coordenadas da malha) de uma célula pelo índice acolchoado.
    #[inline]
    fn center_grid(&self, idx: usize) -> (f64, f64) {
        let pr = idx / self.stride;
        let pc = idx % self.stride;
        (pc as f64 - 0.5, pr as f64 - 0.5)
    }

    /// Leva o ponto à água: ele mesmo se já estiver na água; senão o centro
    /// da célula de água mais próxima dentro de [`SNAP_RADIUS_M`].
    fn snap(&self, p: (f64, f64)) -> Option<Snap> {
        let (r, c) = self.cell_of(p.0, p.1)?;
        let idx = self.pidx(r, c);
        let g = self.to_grid(p.0, p.1);
        if self.mask[idx] == 1 {
            return Some(Snap { idx, water_pt: p, grid: g, moved: false });
        }
        let mh = self.dlat * M_PER_DEG;
        let mw = self.dlon * M_PER_DEG * p.0.to_radians().cos();
        let rr = (SNAP_RADIUS_M / mh).ceil() as isize + 1;
        let rc = (SNAP_RADIUS_M / mw).ceil() as isize + 1;
        let lim2 = (SNAP_RADIUS_M + mh.max(mw)).powi(2);
        let mut best: Option<(f64, usize, usize)> = None;
        for dr in -rr..=rr {
            let r2 = r as isize + dr;
            if r2 < 0 || r2 >= self.rows as isize {
                continue;
            }
            let r2 = r2 as usize;
            let dy = (r2 as f64 + 0.5 - g.1) * mh;
            if dy * dy > lim2 {
                continue;
            }
            let c_lo = (c as isize - rc).max(0) as usize;
            let c_hi = ((c as isize + rc) as usize).min(self.cols - 1);
            let base = self.pidx(r2, 0);
            for c2 in c_lo..=c_hi {
                if self.mask[base + c2] == 0 {
                    continue;
                }
                let dx = (c2 as f64 + 0.5 - g.0) * mw;
                let d2 = dx * dx + dy * dy;
                if best.is_none_or(|b| d2 < b.0) {
                    best = Some((d2, r2, c2));
                }
            }
        }
        let (_, r2, c2) = best?;
        let grid = (c2 as f64 + 0.5, r2 as f64 + 0.5);
        let water_pt = self.grid_to_latlon(grid);
        if haversine_m(p, water_pt) > SNAP_RADIUS_M {
            return None;
        }
        Some(Snap { idx: self.pidx(r2, c2), water_pt, grid, moved: true })
    }

    /// Célula `(x, y)` (não acolchoada; -1 e rows/cols caem na moldura) é água?
    #[inline]
    fn water_xy(&self, cx: isize, cy: isize) -> bool {
        if cx < -1 || cy < -1 || cx > self.cols as isize || cy > self.rows as isize {
            return false;
        }
        self.mask[(cy + 1) as usize * self.stride + (cx + 1) as usize] == 1
    }

    /// O segmento (coordenadas da malha) passa só por células de água?
    /// Percorre todas as células tocadas (Amanatides–Woo); ao cruzar
    /// exatamente uma quina, exige as duas células laterais.
    fn line_of_sight(&self, a: (f64, f64), b: (f64, f64)) -> bool {
        let (x0, y0) = a;
        let (x1, y1) = b;
        let mut cx = x0.floor() as isize;
        let mut cy = y0.floor() as isize;
        if !self.water_xy(cx, cy) {
            return false;
        }
        let ex = x1.floor() as isize;
        let ey = y1.floor() as isize;
        let (dx, dy) = (x1 - x0, y1 - y0);
        let axis = |d: f64, p: f64, cell: isize| -> (isize, f64, f64) {
            if d > 0.0 {
                (1, 1.0 / d, ((cell + 1) as f64 - p) / d)
            } else if d < 0.0 {
                (-1, -1.0 / d, (p - cell as f64) / -d)
            } else {
                (0, f64::INFINITY, f64::INFINITY)
            }
        };
        let (sx, tdx, mut tmx) = axis(dx, x0, cx);
        let (sy, tdy, mut tmy) = axis(dy, y0, cy);
        let mut guard = (ex - cx).unsigned_abs() + (ey - cy).unsigned_abs() + 4;
        while (cx != ex || cy != ey) && guard > 0 {
            guard -= 1;
            if tmx.min(tmy) > 1.0 + 1e-9 {
                break;
            }
            if (tmx - tmy).abs() <= 1e-9 {
                if !self.water_xy(cx + sx, cy) || !self.water_xy(cx, cy + sy) {
                    return false;
                }
                cx += sx;
                cy += sy;
                tmx += tdx;
                tmy += tdy;
            } else if tmx < tmy {
                cx += sx;
                tmx += tdx;
            } else {
                cy += sy;
                tmy += tdy;
            }
            if !self.water_xy(cx, cy) {
                return false;
            }
        }
        self.water_xy(ex, ey)
    }

    /// Remove o zigue-zague: a partir de cada âncora, avança (passos
    /// exponenciais) até o ponto mais distante ainda à vista.
    fn string_pull(&self, pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
        let last = pts.len() - 1;
        let mut out = vec![pts[0]];
        let mut a = 0;
        while a < last {
            let mut lo = a + 1; // vizinho de malha: sempre visível
            let mut step = 1;
            loop {
                let hi = (lo + step).min(last);
                if hi == lo {
                    break;
                }
                if self.line_of_sight(pts[a], pts[hi]) {
                    lo = hi;
                    step *= 2;
                } else if step == 1 {
                    break;
                } else {
                    step /= 2;
                }
            }
            out.push(pts[lo]);
            a = lo;
        }
        out
    }

    /// Monta o resultado: pernas do snap + trecho na água; soma os segmentos.
    fn finish(
        &self,
        origin: (f64, f64),
        os: &Snap,
        water: Vec<(f64, f64)>,
        ts: &Snap,
        target: (f64, f64),
    ) -> RouteResult {
        let mut path = Vec::with_capacity(water.len() + 2);
        if os.moved {
            path.push(origin);
        }
        path.extend(water);
        if ts.moved {
            path.push(target);
        }
        path.dedup();
        if path.len() < 2 {
            path = vec![origin, target];
        }
        // garante extremos exatos
        path[0] = origin;
        let n = path.len();
        path[n - 1] = target;
        let distance_m: f64 = path.windows(2).map(|w| haversine_m(w[0], w[1])).sum();
        RouteResult {
            distance_m,
            straight_m: haversine_m(origin, target),
            via_water: true,
            path: decimate(path, MAX_PATH_POINTS),
        }
    }

    /// Dijkstra 8-conexo (custos inteiros em dm, fila de baldes circular)
    /// a partir de `src` até resolver todas as células de `targets`.
    fn dijkstra(&self, src: usize, targets: &[usize]) -> Search {
        let n = self.mask.len();
        let s = self.stride;
        let mut dist = vec![u32::MAX; n];
        let mut from = vec![DIR_NONE; n];
        let mut is_target = vec![0u64; n / 64 + 1];
        for &t in targets {
            is_target[t >> 6] |= 1 << (t & 63);
        }
        let mut remaining = targets.len();
        let mask = &self.mask;
        let cv = self.cost_v;
        let mut heap = BucketQueue::new();
        dist[src] = 0;
        heap.push(0, src as u32);

        while let Some((d, u)) = heap.pop() {
            let u = u as usize;
            if d != dist[u] || from[u] & SETTLED != 0 {
                continue;
            }
            from[u] |= SETTLED;
            if is_target[u >> 6] & (1 << (u & 63)) != 0 {
                remaining -= 1;
                if remaining == 0 {
                    break;
                }
            }
            let pr = u / s;
            let ch = self.cost_h[pr];
            let dn = self.cost_d[pr];
            let ds = self.cost_d[pr - 1];
            let wn = mask[u + s] == 1;
            let ws = mask[u - s] == 1;
            let we = mask[u + 1] == 1;
            let ww = mask[u - 1] == 1;
            let mut relax = |v: usize, cost: u32, dir: u8| {
                let nd = d + cost;
                if nd < dist[v] {
                    dist[v] = nd;
                    from[v] = dir;
                    heap.push(nd, v as u32);
                }
            };
            if wn {
                relax(u + s, cv, 0);
            }
            if ws {
                relax(u - s, cv, 1);
            }
            if we {
                relax(u + 1, ch, 2);
            }
            if ww {
                relax(u - 1, ch, 3);
            }
            // diagonais só sem cortar quina de terra
            if wn && we && mask[u + s + 1] == 1 {
                relax(u + s + 1, dn, 4);
            }
            if wn && ww && mask[u + s - 1] == 1 {
                relax(u + s - 1, dn, 5);
            }
            if ws && we && mask[u - s + 1] == 1 {
                relax(u - s + 1, ds, 6);
            }
            if ws && ww && mask[u - s - 1] == 1 {
                relax(u - s - 1, ds, 7);
            }
        }
        Search { dist, from }
    }
}

impl std::fmt::Debug for WaterGrid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaterGrid")
            .field("lat", &(self.lat_min, self.lat_max))
            .field("lon", &(self.lon_min, self.lon_max))
            .field("rows", &self.rows)
            .field("cols", &self.cols)
            .finish_non_exhaustive()
    }
}

struct Search {
    dist: Vec<u32>,
    from: Vec<u8>,
}

impl Search {
    /// Células (índices acolchoados) da origem até `t`.
    fn path_to(&self, t: usize, stride: usize) -> Vec<usize> {
        let s = stride as isize;
        let back: [isize; 8] = [-s, s, -1, 1, -s - 1, -s + 1, s - 1, s + 1];
        let mut cells = vec![t];
        let mut v = t;
        loop {
            let dir = self.from[v] & 0x0F;
            if dir == DIR_NONE {
                break;
            }
            v = (v as isize + back[dir as usize]) as usize;
            cells.push(v);
        }
        cells.reverse();
        cells
    }
}

/// Reduz para no máximo `max` pontos mantendo os extremos (só para exibição;
/// a distância já foi somada com o caminho completo).
fn decimate(path: Vec<(f64, f64)>, max: usize) -> Vec<(f64, f64)> {
    let n = path.len();
    if n <= max || max < 2 {
        return path;
    }
    (0..max)
        .map(|k| path[(k * (n - 1) + (max - 1) / 2) / (max - 1)])
        .collect()
}

/// Baldes circulares; maior que o maior custo de aresta (dm).
const BUCKETS: usize = 1024;

/// Fila de prioridade monotônica de Dial: como toda aresta custa menos que
/// [`BUCKETS`], as chaves pendentes cabem em `[cur, cur + BUCKETS)` e cada
/// chave tem um balde só.
struct BucketQueue {
    cur: u32,
    len: usize,
    buckets: Vec<Vec<u32>>,
}

impl BucketQueue {
    fn new() -> BucketQueue {
        BucketQueue { cur: 0, len: 0, buckets: (0..BUCKETS).map(|_| Vec::new()).collect() }
    }

    #[inline]
    fn push(&mut self, key: u32, val: u32) {
        debug_assert!(key >= self.cur && ((key - self.cur) as usize) < BUCKETS);
        self.buckets[key as usize & (BUCKETS - 1)].push(val);
        self.len += 1;
    }

    #[inline]
    fn pop(&mut self) -> Option<(u32, u32)> {
        if self.len == 0 {
            return None;
        }
        loop {
            if let Some(v) = self.buckets[self.cur as usize & (BUCKETS - 1)].pop() {
                self.len -= 1;
                return Some((self.cur, v));
            }
            self.cur += 1;
        }
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn bucket_queue_orders() {
        let mut h = BucketQueue::new();
        let keys = [5u32, 3, 9, 3, 100, 7, 64, 65, 0, 1000];
        for (i, &k) in keys.iter().enumerate() {
            h.push(k, i as u32);
        }
        let mut got = Vec::new();
        while let Some((k, _)) = h.pop() {
            got.push(k);
            if k == 3 {
                h.push(4, 99); // inserção monotônica no meio
            }
        }
        let mut want = keys.to_vec();
        want.push(4);
        want.push(4);
        want.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn decimate_keeps_ends() {
        let p: Vec<(f64, f64)> = (0..200).map(|i| (i as f64, 0.0)).collect();
        let d = decimate(p, MAX_PATH_POINTS);
        assert_eq!(d.len(), MAX_PATH_POINTS);
        assert_eq!(d[0], (0.0, 0.0));
        assert_eq!(d[MAX_PATH_POINTS - 1], (199.0, 0.0));
    }

    #[test]
    fn rejects_bad_header() {
        assert!(WaterGrid::from_bytes(b"XXXX").is_err());
        let mut b = EMBEDDED_BYTES.to_vec();
        b[0] = b'X';
        assert!(WaterGrid::from_bytes(&b).is_err());
        let b = &EMBEDDED_BYTES[..EMBEDDED_BYTES.len() - 10];
        assert!(WaterGrid::from_bytes(b).is_err());
    }
}
