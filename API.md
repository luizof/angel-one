# Angel One — contrato da API

Backend: Rust (Axum + SQLite via rusqlite bundled). Frontend: Svelte 5 + Vite (TypeScript), compilado em `app/web/dist` e servido pelo próprio binário.

```
app/
  API.md            este contrato (fonte de verdade entre front e back)
  routing/          crate lib `water_routing` (std puro): malha de água + rota desviando de terra
  server/           crate bin `angel-one` (Axum)
  web/              Svelte + Vite
  data/             (runtime, ignorado) SQLite + mídia
```

## Convenções

- Tudo em JSON, prefixo `/api`. Datas em RFC 3339 UTC (`2026-11-19T02:10:00Z`). IDs inteiros.
- Erros: status HTTP adequado + `{"error": "mensagem em português"}`.
- Cabeçalhos enviados pelo front em toda requisição:
  - `X-Device-Id: <uuid>` — identifica o aparelho (votos, pedidos do passageiro, push).
  - `X-Crew-Id: <id>` — só quando o aparelho está em modo tripulante.
- Rotas marcadas **(crew)** exigem `X-Crew-Id` de um tripulante existente; senão `403 {"error":"Somente tripulação"}`. Não há senha (risco aceito).
- Ações de tripulante registram o nome do tripulante (`*_by`).
- Variáveis de ambiente: `PORT` (8080), `DATA_DIR` (`./data`), `WEB_DIR` (`./web/dist`), `OPENROUTER_API_KEY`, `WATER_GRID` (caminho do `water_grid.bin`), `TZ_NAME` (`America/Sao_Paulo`). Vento usa sempre o ponto fixo ao norte de Jurerê, independentemente da localização dos visitantes.

## Estáticos e mídia

- `GET /media/<arquivo>` → arquivos de `DATA_DIR/media` (cache longo, imutável).
- Qualquer outra rota fora de `/api` e `/media` → arquivo de `WEB_DIR`; se não existir, `index.html` (SPA). `index.html`, `sw.js` e `manifest.webmanifest` com `Cache-Control: no-cache`; `/assets/*` com cache imutável.

## Upload

- `POST /api/upload` (multipart, campo `file`, até 20 MB; JPEG/PNG/WebP) → `{"url": "/media/<uuid>.jpg", "width": 1600, "height": 1200}`
  O front já envia a imagem reduzida (≤ 2048 px, JPEG). O servidor redimensiona para no máx. 1600 px no lado maior e salva JPEG q85 (PNG com transparência fica PNG).

## Tripulação

- `GET /api/crew` → `[{"id":1,"name":"Luiz","emoji":"🧭","created_at":"..."}]`
- `POST /api/crew` `{"name":"Luiz","emoji":"🧭"}` → tripulante (sem auth, é o cadastro pela entrada oculta).
- `PATCH /api/crew/:id` **(crew)** `{"name"?, "emoji"?}`; `DELETE /api/crew/:id` **(crew)**.

## Configurações

- `GET /api/settings` →
```json
{
  "text_model": "google/gemini-3.8-flash",
  "image_model": "google/gemini-3.1-flash-image",
  "edit_model": "google/gemini-3.1-flash-image",
  "cruise_speed_kn": 18,
  "moving_threshold_kn": 1.5,
  "weather_lat": -27.425, "weather_lon": -48.495,
  "openrouter_key_set": true,
  "vapid_public_key": "B..."
}
```
- `PUT /api/settings` **(crew)** — atualização parcial dos modelos, velocidade de cruzeiro e limiar de movimento. `weather_lat/lon` são somente leitura (Jurerê fixo). A chave é configurada exclusivamente em `OPENROUTER_API_KEY` no servidor; mudanças da chave pela API retornam 400.
- `GET /api/ai/models` (cache 1 h; não precisa de chave) →
```json
{
  "text":  [{"id":"google/gemini-3.8-flash","name":"Google: Gemini 3.8 Flash","provider":"google"}],
  "image": [{"id":"google/gemini-3.1-flash-image","name":"...","provider":"google","image_input":true}]
}
```
  `text` vem de `https://openrouter.ai/api/v1/models` (output contém `text`); `image` de `https://openrouter.ai/api/v1/images/models`. `provider` = prefixo do id antes da `/`. Ordenado por provider, depois nome.

## IA (assíncrona — o túnel do Cloudflare corta requisições > 100 s)

Todas **(crew)**, retornam `202 {"job_id":"<uuid>"}` imediatamente. Resultado por polling:

- `GET /api/ai/jobs/:id` → `{"id":"...","status":"pending|done|error","result":{...}|null,"error":null|"..."}` (jobs ficam em memória por 1 h).
- `POST /api/ai/menu-text` `{"name":"Caipirinha de maracujá","category":"drink"}` → result `{"description":"frase curta e convidativa","recipe":"Ingredientes:\n- ...\n\nPreparo:\n1. ..."}`
  Chat completions (`/api/v1/chat/completions`, `text_model`), resposta em JSON (`response_format: json_object`), pt-BR. Para `bebida` (cerveja, água...) a receita pode ser vazia ou só "Servir gelada".
- `POST /api/ai/menu-photo` `{"name":"...","category":"drink","description":"..."}` → result `{"url":"/media/...jpg"}`
  Images API (`POST https://openrouter.ai/api/v1/images`, `image_model`, `aspect_ratio` 1:1, `resolution` 1K quando suportado): ilustração 2D apetitosa, traços desenhados, cores tropicais, fundo de convés/mar e sem texto.
- `POST /api/ai/clean-photo` `{"url":"/media/...jpg"}` → result `{"url":"/media/...png"}`
  Images API com `input_references` (data URL da foto) e `edit_model`: transformar o item em ilustração 2D sobre branco puro, mantendo proporções, formas e rótulos legíveis; preservar a foto original.
- `POST /api/ai/enhance-photo` `{"url":"/media/...jpg"}` → result `{"url":"/media/...jpg"}`
  Images API com `input_references` e `edit_model`: ilustrar o lugar em 2D sem alterar a costa nem inventar edifícios, ilhas ou objetos.

Resposta da Images API: `data[0].b64_json` (+ `media_type`). Salvar em `DATA_DIR/media`, redimensionado a ≤ 1600 px.

## Destinos e votação

- `GET /api/destinations` →
```json
[{"id":1,"name":"Ilha do Francês","lat":-27.4,"lon":-48.5,
  "photo":"/media/a.jpg","photo_original":"/media/b.jpg",
  "good_winds":["S","SE"],"votes":3,"my_vote":true,
  "created_by":"Luiz","created_at":"..."}]
```
  `good_winds` ⊂ `["N","NE","L","SE","S","SO","O","NO"]` (direções de onde vem o vento contra o qual o local é abrigado). `my_vote` usa `X-Device-Id`.
- `POST /api/destinations` **(crew)** `{"name","lat","lon","photo"?,"photo_original"?,"good_winds":[]}` → destino. `PATCH /api/destinations/:id` **(crew)** parcial; `DELETE` **(crew)** (apaga votos).
- `POST /api/destinations/:id/vote` → um voto por aparelho em cada turno; votar em outro move o voto. Turnos de 12 horas: 00:00 e 12:00 em `America/Sao_Paulo` (Florianópolis), independentemente de o app estar aberto; votos antigos não contam e são removidos. Devolve `{"votes":{"<id>":n,...},"my_vote":<id>}`.
- `DELETE /api/votes/mine` → remove o voto do aparelho, mesmo formato.
- `POST /api/votes/reset` **(crew)** → zera tudo.
- `GET /api/routes?lat=..&lon=..` → rotas da origem até todos os destinos:
```json
[{"destination_id":1,"distance_m":5230.4,"straight_m":4100.2,"via_water":true,
  "path":[[-27.43,-48.49],[-27.41,-48.47]]}]
```
  Usa o crate `water_routing` (um Dijkstra a partir da origem, lê todos os alvos). Se a origem ou o destino estiver fora da malha, ou não houver caminho: `via_water:false`, `distance_m = straight_m`, `path = [origem, destino]`.

## Previsão (Open-Meteo, cache 30 min, sempre ao norte de Jurerê Internacional)

- `GET /api/weather` →
```json
{"updated_at":"...","timezone":"America/Sao_Paulo","location":"Jurerê Internacional","lat":-27.425,"lon":-48.495,
 "hours":[{"time":"2026-11-19T09:00","wind_kn":12.3,"gust_kn":18.1,"wind_dir":160,
           "wave_m":0.6,"wave_dir":140,"swell_m":0.4,"swell_dir":130}]}
```
  Horas locais de hoje até o fim de amanhã. Vento: `api.open-meteo.com/v1/forecast` (`hourly=wind_speed_10m,wind_direction_10m,wind_gusts_10m`, `wind_speed_unit=kn`, `timezone=America/Sao_Paulo`, `forecast_days=2`). Ondas: `marine-api.open-meteo.com/v1/marine` (`hourly=wave_height,wave_direction,swell_wave_height,swell_wave_direction`); campos ficam `null` se a API marine falhar. A pontuação dos destinos é feita no front.

## Status do barco

- `POST /api/position` `{"lat","lon","accuracy_m","speed_mps":null|n,"heading":null|n}` → `204`. Descartar `accuracy_m > 150`.
- `GET /api/status` → `{"lat","lon","speed_kn":0.4,"moving":false,"heading":null,"updated_at":"...","stale":false}` (ou `{"lat":null,...,"stale":true}` sem dados).
  Velocidade: `speed_mps` informado (≥ 0) tem prioridade; senão, calculada com a posição anterior (de qualquer aparelho) se a diferença for de 20 s a 10 min. `moving = speed_kn > moving_threshold_kn`. `stale = updated_at` há mais de 30 min. Vale a leitura mais recente.

## Inventário

- `GET /api/inventory` →
```json
{"items":[{"id":1,"name":"Absolut 1L","photo":"/media/x.png","photo_original":"/media/y.jpg",
           "tag_ids":[2,5],"count_unit":"garrafa","content_amount":1000,"content_unit":"ml",
           "quantity":1.5,"min_qty":null,"fractional":true,
           "updated_at":"...","updated_by":"Luiz"}],
 "tags":[{"id":2,"name":"Vodka","emoji":"🍸","unit":"L","min_qty":2,
          "total":2.25,"below_min":false,"item_ids":[1,7]}]}
```
  `content_unit` ∈ `ml|L|g|kg|un|null`; tag `unit` ∈ `L|ml|kg|g|un`. Total da tag = Σ `quantity × content_amount` convertido para a unidade da tag (volume↔volume, massa↔massa). Tag `un` soma `quantity`. Itens incompatíveis não entram na soma. `below_min` = `min_qty` definido e `total < min_qty`. Itens: abaixo do mínimo se `min_qty` definido e `quantity < min_qty`.
- `POST /api/items` **(crew)** (mesmos campos, sem id/updated_*) → item. `PATCH /api/items/:id` **(crew)** parcial. `DELETE /api/items/:id` **(crew)**.
- `POST /api/items/:id/adjust` **(crew)** `{"quantity": 0.5}` (absoluto) ou `{"delta": 2}`, e `"reason": "conferencia|compra|ajuste"` → item atualizado; grava movimento.
- `GET /api/items/:id/history` → `[{"at","by","reason","before","after"}]` (últimos 50).
- `POST /api/tags` **(crew)** `{"name","emoji"?,"unit","min_qty"?}`; `PATCH /api/tags/:id`; `DELETE /api/tags/:id` (remove dos itens).

## Cardápio e pedidos

- `GET /api/menu` → `[{"id","name","category":"drink|petisco|bebida","description","recipe","photo","available":true,"sort":0,"created_at"}]`
- `POST /api/menu` **(crew)**, `PATCH /api/menu/:id` **(crew)** parcial (inclui `available`), `DELETE /api/menu/:id` **(crew)**.
- `POST /api/orders` `{"menu_item_id":3,"for_name":"Ana","notes":"sem açúcar"}` → pedido (`400` se item indisponível).
- `GET /api/orders` → sem `X-Crew-Id`: pedidos do aparelho nas últimas 12 h. Com `X-Crew-Id`: pedidos não finalizados + finalizados das últimas 12 h. Formato:
```json
[{"id":9,"menu_item_id":3,"item_name":"Caipirinha","item_photo":"/media/..","category":"drink",
  "recipe":"...","for_name":"Ana","notes":"","status":"recebido","handled_by":null,
  "created_at":"...","updated_at":"..."}]
```
  `status`: `recebido → preparando → pronto → entregue` ou `cancelado`.
- `PATCH /api/orders/:id` `{"status":"preparando"}` **(crew)**, registra `handled_by`. O passageiro dono (mesmo `X-Device-Id`) pode mudar para `cancelado` enquanto estiver `recebido`.

## Push (Web Push com VAPID; chaves geradas no 1º start e salvas em `DATA_DIR`)

- `POST /api/push/subscribe` `{"subscription":{endpoint,keys:{p256dh,auth}}}` → `204`. Guarda junto com `X-Device-Id` e `X-Crew-Id` (se houver). Reenvio atualiza.
- `POST /api/push/unsubscribe` `{"endpoint":"..."}` → `204`.
- Envio: pedido novo → todas as inscrições com tripulante (`{"title":"Novo pedido 🍹","body":"Caipirinha para Ana","url":"/pedidos"}`). Pedido `pronto` → inscrições do aparelho que pediu (`{"title":"Seu pedido está pronto! 🪽","body":"Caipirinha para Ana","url":"/bar"}`). Inscrições com 404/410 são removidas.

## Tempo real (SSE)

- `GET /api/events` → `text/event-stream`, comentário keep-alive a cada 15 s. Eventos (`event:` + `data:` JSON):
  - `order_new`, `order_update` → `{}`; os dados são lidos em `/api/orders` com o escopo do aparelho/tripulante
  - `menu`, `inventory`, `destinations`, `crew`, `settings` → `{}` (front recarrega)
  - `votes` → `{"votes":{"<id>":n}}`
  - `status` → mesmo formato de `GET /api/status`
