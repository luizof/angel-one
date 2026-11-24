# water_routing

Distância pela água (desviando de ilhas, pontas e continente) de uma origem até
vários destinos, no norte da Ilha de Santa Catarina. Rust puro, sem
dependências.

```rust
use water_routing::WaterGrid;

let grid = WaterGrid::embedded(); // ou WaterGrid::load(path) (env WATER_GRID)
let routes = grid.routes_from((lat, lon), &[(lat1, lon1), (lat2, lon2)]);
// routes[i]: distance_m, straight_m, via_water, path: Vec<(lat, lon)>
```

- Um Dijkstra 8-conexo por chamada (custos métricos, sem cortar quina de
  terra), parando quando todos os destinos são resolvidos; destinos à vista
  saem direto, sem Dijkstra.
- Pontos em terra/na costa vão para a água mais próxima num raio de 1,5 km;
  a perna até o ponto real entra na distância. Fora do bbox, sem água no raio
  ou sem caminho: `via_water = false`, `distance_m = straight_m`,
  `path = [origem, destino]`.
- Caminho suavizado por linha de visada; `distance_m` é a soma haversine dos
  segmentos. `path` tem no máximo 80 pontos.
- ~15 ms para 20 destinos em release (pior caso, grid inteiro).

Testes: `cargo test --release -- --nocapture`. Linha de comando para inspecionar
rotas: `cargo run --release --example route -- <lat> <lon> <lat> <lon> ...`.

## Malha

`assets/water_grid.bin` é gerado por `../tools/build_water_grid.py` (stdlib
Python) a partir das linhas de costa do OpenStreetMap. Formato e detalhes no
cabeçalho do script e de `src/lib.rs`. Grid 1334 × 1135 células de 40 m,
bbox lat -27.66..-27.18, lon -48.72..-48.26, 1 célula de margem da costa.

Dados © colaboradores do OpenStreetMap, licença ODbL
(<https://www.openstreetmap.org/copyright>). Se o mapa/rota for exibido no app,
inclua essa atribuição.
