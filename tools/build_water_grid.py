#!/usr/bin/env python3
"""Gera a malha de água usada pelo crate `water_routing`.

Uso (a partir de qualquer pasta):
    python3 -I app/tools/build_water_grid.py [--refresh]

1. Baixa do Overpass (uma consulta) as ways `natural=coastline` do bbox e
   guarda o JSON bruto em tools/cache/coastline.json (reutilizado se existir;
   --refresh força novo download).
2. Rasteriza a costa num grid regular em graus com célula de ~40 m. Cada
   segmento marca todas as células que atravessa (travessia tipo
   Amanatides-Woo, 4-conexa), então a costa é uma parede contínua.
3. Flood fill 4-conexo a partir de um ponto de mar aberto: alcançado = água,
   o resto = terra (inclui ilhas e lagoas internas).
4. Erosão de 1 célula da água junto à terra (margem de segurança). Bolsões
   que a erosão separar do mar principal (passagens estreitas) são religados
   pelo caminho mais curto na água original, com 1 célula de largura.
5. Escreve routing/assets/water_grid.bin e tools/cache/water_grid_debug.png.

Formato do .bin (little-endian):
    b"AOWG", u32 versão=1, f64 lat_min, f64 lat_max, f64 lon_min, f64 lon_max,
    u32 rows, u32 cols, bitset row-major (linha 0 = lat_min/sul,
    coluna 0 = lon_min/oeste), bit LSB-first, 1 = água.
"""

import argparse
import json
import math
import struct
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
APP = HERE.parent
CACHE = HERE / "cache"
COAST_JSON = CACHE / "coastline.json"
DEBUG_PNG = CACHE / "water_grid_debug.png"
OUT_BIN = APP / "routing" / "assets" / "water_grid.bin"

LAT_MIN, LAT_MAX = -27.66, -27.18
LON_MIN, LON_MAX = -48.72, -48.26
CELL_M = 40.0
EARTH_R = 6371008.8  # raio médio; o mesmo do haversine no crate
M_PER_DEG = EARTH_R * math.pi / 180.0
SEED = (-27.38, -48.45)  # mar aberto entre o norte da Ilha e Gov. Celso Ramos

OVERPASS_URL = "https://overpass-api.de/api/interpreter"
USER_AGENT = "angel-one-routing/1.0"

# Pontos de conferência (lat, lon, esperado_agua, nome)
CHECKS = [
    (-27.38, -48.45, True, "semente (mar aberto)"),
    (-27.4300, -48.5100, True, "em frente a Jurerê"),
    (-27.5400, -48.5700, True, "Baía Norte"),
    (-27.6300, -48.5750, True, "Baía Sul (após as pontes)"),
    (-27.3050, -48.3610, True, "sul da Ilha do Arvoredo"),
    (-27.5954, -48.5480, False, "Florianópolis centro"),
    (-27.5000, -48.4500, False, "interior da Ilha"),
    (-27.4500, -48.6600, False, "continente (Biguaçu)"),
    (-27.2780, -48.3650, False, "Ilha do Arvoredo"),
    (-27.4300, -48.5650, False, "Ilha de Anhatomirim"),
]


def fetch_coastline(refresh: bool) -> dict:
    if COAST_JSON.exists() and not refresh:
        print(f"usando cache {COAST_JSON}")
        with open(COAST_JSON, "rb") as f:
            return json.load(f)
    query = (
        "[out:json][timeout:180];"
        f'way["natural"="coastline"]({LAT_MIN},{LON_MIN},{LAT_MAX},{LON_MAX});'
        "out geom;"
    )
    data = urllib.parse.urlencode({"data": query}).encode()
    req = urllib.request.Request(
        OVERPASS_URL, data=data, headers={"User-Agent": USER_AGENT}
    )
    print("baixando coastline do Overpass...")
    t0 = time.time()
    raw = None
    for attempt in range(1, 5):
        try:
            with urllib.request.urlopen(req, timeout=240) as resp:
                raw = resp.read()
            break
        except urllib.error.HTTPError as e:
            # 429/502/503/504 são comuns no Overpass público: espera e tenta de novo
            if e.code not in (429, 502, 503, 504) or attempt == 4:
                raise
            wait = 20 * attempt
            print(f"  HTTP {e.code}; nova tentativa em {wait}s")
            time.sleep(wait)
    doc = json.loads(raw)
    CACHE.mkdir(parents=True, exist_ok=True)
    with open(COAST_JSON, "wb") as f:
        f.write(raw)
    print(f"  {len(raw)} bytes em {time.time() - t0:.1f}s -> {COAST_JSON}")
    return doc


class Grid:
    def __init__(self):
        lat_mid = (LAT_MIN + LAT_MAX) / 2.0
        self.rows = round((LAT_MAX - LAT_MIN) * M_PER_DEG / CELL_M)
        self.cols = round(
            (LON_MAX - LON_MIN) * M_PER_DEG * math.cos(math.radians(lat_mid)) / CELL_M
        )
        self.dlat = (LAT_MAX - LAT_MIN) / self.rows
        self.dlon = (LON_MAX - LON_MIN) / self.cols
        self.cell_h_m = self.dlat * M_PER_DEG
        self.cell_w_m = self.dlon * M_PER_DEG * math.cos(math.radians(lat_mid))

    def to_xy(self, lat, lon):
        return (lon - LON_MIN) / self.dlon, (lat - LAT_MIN) / self.dlat

    def cell(self, lat, lon):
        x, y = self.to_xy(lat, lon)
        c, r = int(math.floor(x)), int(math.floor(y))
        if 0 <= r < self.rows and 0 <= c < self.cols:
            return r, c
        return None


def raster_segment(blocked, rows, cols, x0, y0, x1, y1):
    """Marca todas as células atravessadas pelo segmento (grid coords)."""
    if max(x0, x1) < -1 or min(x0, x1) > cols + 1:
        return
    if max(y0, y1) < -1 or min(y0, y1) > rows + 1:
        return

    def mark(cx, cy):
        if 0 <= cx < cols and 0 <= cy < rows:
            blocked[cy * cols + cx] = 1

    cx, cy = int(math.floor(x0)), int(math.floor(y0))
    dx, dy = x1 - x0, y1 - y0
    inf = float("inf")
    if dx > 0:
        sx, tdx, tmx = 1, 1.0 / dx, (cx + 1 - x0) / dx
    elif dx < 0:
        sx, tdx, tmx = -1, -1.0 / dx, (x0 - cx) / -dx
    else:
        sx, tdx, tmx = 0, inf, inf
    if dy > 0:
        sy, tdy, tmy = 1, 1.0 / dy, (cy + 1 - y0) / dy
    elif dy < 0:
        sy, tdy, tmy = -1, -1.0 / dy, (y0 - cy) / -dy
    else:
        sy, tdy, tmy = 0, inf, inf

    mark(cx, cy)
    guard = int(abs(dx) + abs(dy)) + 8
    while guard > 0:
        guard -= 1
        t = tmx if tmx < tmy else tmy
        if t > 1.0:
            break
        if abs(tmx - tmy) <= 1e-12:
            # passa exatamente pela quina: marca os dois vizinhos para a
            # parede continuar 4-conexa
            mark(cx + sx, cy)
            mark(cx, cy + sy)
            cx += sx
            cy += sy
            tmx += tdx
            tmy += tdy
        elif tmx < tmy:
            cx += sx
            tmx += tdx
        else:
            cy += sy
            tmy += tdy
        mark(cx, cy)


def flood_fill(passable, rows, cols, seed_idx):
    """4-conexo. passable: bytearray (1 = pode entrar). Retorna bytearray 1=alcançado."""
    n = rows * cols
    seen = bytearray(n)
    if not passable[seed_idx]:
        return seen
    seen[seed_idx] = 1
    stack = [seed_idx]
    pop, push = stack.pop, stack.append
    while stack:
        i = pop()
        c = i % cols
        if c > 0:
            j = i - 1
            if passable[j] and not seen[j]:
                seen[j] = 1
                push(j)
        if c < cols - 1:
            j = i + 1
            if passable[j] and not seen[j]:
                seen[j] = 1
                push(j)
        if i >= cols:
            j = i - cols
            if passable[j] and not seen[j]:
                seen[j] = 1
                push(j)
        j = i + cols
        if j < n and passable[j] and not seen[j]:
            seen[j] = 1
            push(j)
    return seen


# anel 8-vizinho (dr, dc)
RING = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)]


def erode_reconnect(water, rows, cols, seed_idx):
    """Erosão de 1 célula junto à terra sem desconectar a água.

    1. Erosão completa: sai toda célula de água com algum 8-vizinho (dentro
       do grid) que é terra. A borda do grid conta como mar.
    2. Componentes 4-conexos do que sobrou que ficaram separados do
       componente da semente (baías atrás de canais estreitos, ou o outro
       lado de uma passagem que fechou) são religados: BFS a partir do
       componente principal pela água ORIGINAL; ao alcançar um componente
       ainda solto, o caminho mais curto de volta (1 célula de largura) é
       restaurado. Canais de 1-2 células que não levam a nada somem (o snap
       de até 1,5 km cobre destinos ali).
    Retorna (água final, removidas, restauradas, componentes religados).
    """
    n = rows * cols
    eroded = bytearray(water)
    removed = 0
    for r in range(rows):
        base = r * cols
        for c in range(cols):
            i = base + c
            if not water[i]:
                continue
            for dr, dc in RING:
                rr, cc = r + dr, c + dc
                if 0 <= rr < rows and 0 <= cc < cols and not water[rr * cols + cc]:
                    eroded[i] = 0
                    removed += 1
                    break
    if not eroded[seed_idx]:
        sys.exit("semente perto demais da costa para a erosão")

    # rótulos dos componentes do grid erodido (0 = sem componente)
    label = [0] * n
    nlab = 0
    for start in range(n):
        if eroded[start] and not label[start]:
            nlab += 1
            label[start] = nlab
            stack = [start]
            while stack:
                i = stack.pop()
                c = i % cols
                for j in (
                    i - 1 if c > 0 else -1,
                    i + 1 if c < cols - 1 else -1,
                    i - cols,
                    i + cols,
                ):
                    if 0 <= j < n and eroded[j] and not label[j]:
                        label[j] = nlab
                        stack.append(j)
    main = label[seed_idx]
    connected = {main}

    # BFS pela água original a partir do componente principal
    parent = [-2] * n  # -2 = não visitado, -1 = raiz
    queue = [i for i in range(n) if label[i] == main]
    for i in queue:
        parent[i] = -1
    restored = 0
    head = 0
    while head < len(queue):
        i = queue[head]
        head += 1
        lb = label[i]
        if lb and lb not in connected:
            connected.add(lb)
            k = parent[i]
            while k >= 0 and not eroded[k]:
                eroded[k] = 1
                restored += 1
                k = parent[k]
            # o trecho já restaurado/erodido até a raiz está ligado
        c = i % cols
        for j in (
            i - 1 if c > 0 else -1,
            i + 1 if c < cols - 1 else -1,
            i - cols,
            i + cols,
        ):
            if 0 <= j < n and water[j] and parent[j] == -2:
                parent[j] = i
                queue.append(j)
    return eroded, removed, restored, len(connected) - 1


def write_bin(path, grid, water):
    rows, cols = grid.rows, grid.cols
    n = rows * cols
    bits = bytearray((n + 7) // 8)
    for i in range(n):
        if water[i]:
            bits[i >> 3] |= 1 << (i & 7)
    header = b"AOWG" + struct.pack(
        "<I4d2I", 1, LAT_MIN, LAT_MAX, LON_MIN, LON_MAX, rows, cols
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "wb") as f:
        f.write(header)
        f.write(bits)
    return len(header) + len(bits)


def write_png(path, width, height, rgb_rows):
    """rgb_rows: lista de bytes (3*width) de cima para baixo."""

    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    raw = b"".join(b"\x00" + row for row in rgb_rows)
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">2I5B", width, height, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "wb") as f:
        f.write(png)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--refresh", action="store_true", help="refaz o download")
    args = ap.parse_args()

    doc = fetch_coastline(args.refresh)
    ways = [e for e in doc.get("elements", []) if e.get("type") == "way"]
    nodes = sum(len(w.get("geometry", [])) for w in ways)
    print(f"{len(ways)} ways de coastline, {nodes} nós")
    if not ways:
        sys.exit("nenhuma coastline no JSON")

    g = Grid()
    rows, cols = g.rows, g.cols
    n = rows * cols
    print(
        f"grid {rows} x {cols} = {n} células; célula {g.cell_h_m:.1f} m (lat) x "
        f"{g.cell_w_m:.1f} m (lon, na lat média); dlat={g.dlat:.8f} dlon={g.dlon:.8f}"
    )

    t0 = time.time()
    blocked = bytearray(n)
    for w in ways:
        geom = [p for p in w.get("geometry", []) if p]
        pts = [g.to_xy(p["lat"], p["lon"]) for p in geom]
        for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
            raster_segment(blocked, rows, cols, x0, y0, x1, y1)
    print(f"costa rasterizada: {sum(blocked)} células bloqueadas ({time.time() - t0:.1f}s)")

    seed = g.cell(*SEED)
    if seed is None:
        sys.exit("semente fora do grid")
    seed_idx = seed[0] * cols + seed[1]
    if blocked[seed_idx]:
        sys.exit("semente caiu numa célula de costa")

    t0 = time.time()
    passable = bytearray(1 - b for b in blocked)
    water = flood_fill(passable, rows, cols, seed_idx)
    n_water = sum(water)
    print(f"flood fill: {n_water} células de água ({100.0 * n_water / n:.1f}%) ({time.time() - t0:.1f}s)")
    if n_water > 0.9 * (n - sum(blocked)):
        print("AVISO: quase tudo virou água — a costa pode ter vazado")

    t0 = time.time()
    eroded, removed, restored, relinked = erode_reconnect(water, rows, cols, seed_idx)
    final = flood_fill(eroded, rows, cols, seed_idx)
    lost = sum(eroded) - sum(final)
    print(
        f"erosão: {removed} removidas, {restored} restauradas para religar "
        f"{relinked} bolsões; {lost} células soltas ({time.time() - t0:.1f}s)"
    )
    if lost:
        sys.exit("BUG: sobrou água desconectada após religar")
    n_final = sum(final)
    print(f"água final: {n_final} células ({100.0 * n_final / n:.1f}%)")

    ok = True
    for lat, lon, want, name in CHECKS:
        rc = g.cell(lat, lon)
        got = bool(final[rc[0] * cols + rc[1]]) if rc else None
        flag = "ok " if got == want else "!! "
        if got != want:
            ok = False
        print(f"  {flag}{name} ({lat}, {lon}): {'água' if got else 'terra'}")

    size = write_bin(OUT_BIN, g, final)
    print(f"escrito {OUT_BIN} ({size} bytes)")

    # PNG de depuração: norte em cima
    water_c = bytes((40, 110, 190))
    margin_c = bytes((120, 170, 220))  # água removida pela erosão
    land_c = bytes((225, 205, 160))
    coast_c = bytes((150, 120, 80))
    seed_c = bytes((230, 30, 30))
    rows_rgb = []
    for r in range(rows - 1, -1, -1):
        base = r * cols
        row = bytearray()
        for c in range(cols):
            i = base + c
            if final[i]:
                row += water_c
            elif water[i]:
                row += margin_c
            elif blocked[i]:
                row += coast_c
            else:
                row += land_c
        rows_rgb.append(row)
    sr, sc = seed
    for dr in range(-3, 4):
        for dc in range(-3, 4):
            rr, cc = sr + dr, sc + dc
            if 0 <= rr < rows and 0 <= cc < cols:
                y = rows - 1 - rr
                rows_rgb[y][cc * 3 : cc * 3 + 3] = seed_c
    write_png(DEBUG_PNG, cols, rows, [bytes(r) for r in rows_rgb])
    print(f"escrito {DEBUG_PNG}")
    if not ok:
        print("AVISO: algum ponto de conferência não bateu")


if __name__ == "__main__":
    main()
