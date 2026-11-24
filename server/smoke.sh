#!/usr/bin/env bash
# Smoke test do backend Angel One.
# Sobe o binário com DATA_DIR/WEB_DIR temporários numa porta livre e exercita a API.
# Uso: ./smoke.sh            (compila se preciso; BIN=/caminho/angel-one para usar outro binário)
# Requisitos: curl, python3, magick (ImageMagick), sqlite3. Chama Open-Meteo e a lista
# pública de modelos do OpenRouter; nenhuma API paga é usada (sem OPENROUTER_API_KEY).
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/angel-one/target}"
BIN="${BIN:-$CARGO_TARGET_DIR/release/angel-one}"

if [ -z "${SKIP_BUILD:-}" ]; then
  command -v cargo >/dev/null 2>&1 || source "$HOME/.cargo/env"
  (cd "$HERE" && cargo build --release --quiet) || { echo "falha no build"; exit 1; }
fi
[ -x "$BIN" ] || { echo "binário não encontrado: $BIN"; exit 1; }

TMP="$(mktemp -d -t angel-one-smoke.XXXXXX)"
SERVER_PID=""
SSE_PID=""
cleanup() {
  [ -n "$SSE_PID" ] && kill "$SSE_PID" 2>/dev/null
  if [ -n "$SERVER_PID" ]; then
    kill "$SERVER_PID" 2>/dev/null
    for _ in $(seq 1 50); do kill -0 "$SERVER_PID" 2>/dev/null || break; sleep 0.1; done
    kill -9 "$SERVER_PID" 2>/dev/null
  fi
  wait 2>/dev/null
  rm -rf "$TMP"
}
trap cleanup EXIT

# Porta livre a partir de 18080.
PORT="${PORT:-18080}"
while (exec 3<>"/dev/tcp/127.0.0.1/$PORT") 2>/dev/null; do PORT=$((PORT + 1)); done
BASE="http://127.0.0.1:$PORT"

mkdir -p "$TMP/web/assets" "$TMP/data"
echo '<!doctype html><title>FAKE INDEX</title>' > "$TMP/web/index.html"
echo 'console.log("app")' > "$TMP/web/assets/app-abc123.js"

env -u OPENROUTER_API_KEY -u WATER_GRID PORT="$PORT" DATA_DIR="$TMP/data" WEB_DIR="$TMP/web" \
  "$BIN" > "$TMP/server.log" 2>&1 &
SERVER_PID=$!
for _ in $(seq 1 100); do curl -sf "$BASE/api/health" >/dev/null 2>&1 && break; sleep 0.1; done

PASS=0
FAIL=0
pass() { PASS=$((PASS + 1)); printf '  \033[32m✔\033[0m %s\n' "$1"; }
fail() { FAIL=$((FAIL + 1)); printf '  \033[31m✘ %s\033[0m\n' "$1"; }
section() { printf '\n\033[1m== %s\033[0m\n' "$1"; }

DEV_A="11111111-aaaa-4aaa-8aaa-000000000001"
DEV_B="22222222-bbbb-4bbb-8bbb-000000000002"
DEV_C="33333333-cccc-4ccc-8ccc-000000000003"
CREW=""

# call MÉTODO CAMINHO [JSON] [dev=<id>] [crew=<id>]  → $CODE e $TMP/body
call() {
  local method=$1 path=$2 data="" dev="$DEV_A" crew=""
  shift 2
  if [ $# -gt 0 ] && [[ "$1" != dev=* && "$1" != crew=* ]]; then data=$1; shift; fi
  for a in "$@"; do
    case "$a" in dev=*) dev="${a#dev=}" ;; crew=*) crew="${a#crew=}" ;; esac
  done
  local args=(-s -o "$TMP/body" -D "$TMP/headers" -w '%{http_code}' -X "$method" "$BASE$path")
  [ -n "$dev" ] && args+=(-H "X-Device-Id: $dev")
  [ -n "$crew" ] && args+=(-H "X-Crew-Id: $crew")
  [ -n "$data" ] && args+=(-H 'Content-Type: application/json' --data-binary "$data")
  CODE=$(curl "${args[@]}")
}

# check "descrição" CÓDIGO_ESPERADO 'expressão python sobre j (JSON), raw, h (headers)'
check() {
  local desc=$1 expected=$2 expr=${3:-True}
  if python3 -I - "$TMP/body" "$TMP/headers" "$CODE" "$expected" "$expr" <<'PY'
import json, sys
body, headers, code, expected, expr = sys.argv[1:6]
raw = open(body, encoding="utf-8", errors="replace").read()
h = {}
for line in open(headers, encoding="latin-1").read().splitlines()[1:]:
    if ":" in line:
        k, v = line.split(":", 1)
        h[k.strip().lower()] = v.strip()
try:
    j = json.loads(raw) if raw.strip() else None
except Exception:
    j = None
ok = code == expected
if ok:
    try:
        ok = bool(eval(expr, {"j": j, "raw": raw, "h": h}))
    except Exception as e:
        print("    expressão falhou:", repr(e), file=sys.stderr)
        ok = False
sys.exit(0 if ok else 1)
PY
  then pass "$desc"; else fail "$desc — HTTP $CODE: $(head -c 400 "$TMP/body")"; fi
}

# jget 'expressão python sobre j' → imprime o valor
jget() { python3 -I -c 'import json,sys; j=json.load(open(sys.argv[1])); v=eval(sys.argv[2]); print(v if not isinstance(v,(dict,list)) else json.dumps(v))' "$TMP/body" "$1"; }

sql() { sqlite3 "$TMP/data/angel-one.sqlite3" "$1"; }

# SSE em segundo plano durante todo o teste.
curl -N -s --max-time 120 -H "X-Device-Id: $DEV_A" "$BASE/api/events" > "$TMP/sse.txt" &
SSE_PID=$!
sleep 0.3

section "Saúde, SPA, estáticos"
call GET /api/health
check "GET /api/health" 200 'j == {"ok": True}'
call GET /pedidos/123
check "fallback SPA devolve index.html (no-cache)" 200 '"FAKE INDEX" in raw and h.get("cache-control") == "no-cache"'
call GET /index.html
check "index.html com no-cache" 200 '"FAKE INDEX" in raw and h.get("cache-control") == "no-cache"'
call GET /assets/app-abc123.js
check "/assets com cache imutável" 200 '"immutable" in h.get("cache-control", "")'
call GET /assets/nao-existe.js
check "/assets inexistente → 404 (sem SPA)" 404
call GET /api/nao-existe
check "/api desconhecida → 404 JSON" 404 'j["error"] == "Rota não encontrada"'

section "Status inicial e primeira posição"
call GET /api/status
check "status sem dados → nulos e stale" 200 'j["lat"] is None and j["stale"] is True and j["moving"] is False'
T_A=$(date +%s.%N)
call POST /api/position '{"lat":-27.43,"lon":-48.49,"accuracy_m":10,"speed_mps":null,"heading":null}'
check "POST /api/position A → 204" 204
call GET /api/status
check "status após A (sem velocidade ainda)" 200 'abs(j["lat"] + 27.43) < 1e-9 and j["speed_kn"] is None and j["stale"] is False and j["updated_at"].endswith("Z")'
call POST /api/position '{"lat":-27.0,"lon":-48.0,"accuracy_m":500}'
check "posição com accuracy_m > 150 → 204 (descartada)" 204
call GET /api/status
check "status ignora a posição imprecisa" 200 'abs(j["lat"] + 27.43) < 1e-9'
call POST /api/position '{"lat":200,"lon":0}'
check "posição inválida → 400" 400 '"error" in j'

section "Tripulação"
call POST /api/crew '{"name":"Luiz","emoji":"🧭"}'
check "POST /api/crew (sem auth)" 201 'j["name"] == "Luiz" and j["emoji"] == "🧭" and j["created_at"].endswith("Z")'
CREW=$(jget 'j["id"]')
call POST /api/crew '{"name":"Ana"}'
check "POST /api/crew sem emoji" 201 'j["emoji"] == ""'
CREW2=$(jget 'j["id"]')
call GET /api/crew
check "GET /api/crew lista 2" 200 'len(j) == 2 and j[0]["name"] == "Luiz"'
call PATCH "/api/crew/$CREW2" '{"emoji":"⚓"}'
check "PATCH crew sem X-Crew-Id → 403" 403 'j["error"] == "Somente tripulação"'
call PATCH "/api/crew/$CREW2" '{"emoji":"⚓"}' crew=999
check "PATCH crew com X-Crew-Id inexistente → 403" 403
call PATCH "/api/crew/$CREW2" '{"emoji":"⚓","name":"Ana Paula"}' crew="$CREW"
check "PATCH crew (crew)" 200 'j["emoji"] == "⚓" and j["name"] == "Ana Paula"'
call POST /api/crew '{"name":"  "}'
check "POST crew com nome vazio → 400" 400
call DELETE "/api/crew/$CREW2" crew="$CREW"
check "DELETE crew (crew) → 204" 204
call GET /api/settings crew="$CREW2"
call PUT /api/settings '{"cruise_speed_kn":10}' crew="$CREW2"
check "tripulante apagado perde acesso → 403" 403

section "Configurações"
call GET /api/settings
check "GET /api/settings (padrões)" 200 'j["text_model"] == "google/gemini-3.8-flash" and j["image_model"] == "google/gemini-3.1-flash-image" and j["edit_model"] == "google/gemini-3.1-flash-image" and j["cruise_speed_kn"] == 18 and j["moving_threshold_kn"] == 1.5 and j["weather_lat"] == -27.425 and j["weather_lon"] == -48.495 and j["openrouter_key_set"] is False'
check "vapid_public_key = 65 bytes não comprimidos (base64url)" 200 '(lambda b: len(b) == 65 and b[0] == 4)(__import__("base64").urlsafe_b64decode(j["vapid_public_key"] + "=" * (-len(j["vapid_public_key"]) % 4))) and "=" not in j["vapid_public_key"]'
call PUT /api/settings '{"cruise_speed_kn":20}'
check "PUT /api/settings sem crew → 403" 403
call PUT /api/settings '{"cruise_speed_kn":20,"moving_threshold_kn":2,"text_model":"openai/gpt-test","openrouter_key_set":true,"vapid_public_key":"x"}' crew="$CREW"
check "PUT parcial (ignora campos somente leitura)" 200 'j["cruise_speed_kn"] == 20 and j["moving_threshold_kn"] == 2 and j["text_model"] == "openai/gpt-test" and j["openrouter_key_set"] is False and j["vapid_public_key"] != "x"'
call PUT /api/settings '{"cruise_speed_kn":-1}' crew="$CREW"
check "PUT com valor inválido → 400" 400
call PUT /api/settings '{"openrouter_key":"sk-or-teste-123"}' crew="$CREW"
check "chave somente no servidor" 400 '"sk-or-teste" not in raw'
call PUT /api/settings '{"openrouter_key":""}' crew="$CREW"
check "chave não pode ser apagada pelo app" 400
call PUT /api/settings '{"moving_threshold_kn":1.5,"text_model":"google/gemini-3.8-flash"}' crew="$CREW"
check "restaura limiar e modelo" 200 'j["moving_threshold_kn"] == 1.5'

section "Upload e /media"
magick -size 2400x1800 gradient:navy-skyblue -quality 92 "$TMP/big.jpg"
CODE=$(curl -s -o "$TMP/body" -D "$TMP/headers" -w '%{http_code}' -H "X-Device-Id: $DEV_A" -F "file=@$TMP/big.jpg;type=image/jpeg" "$BASE/api/upload")
check "upload JPEG 2400×1800 → 1600×1200 .jpg" 200 'j["width"] == 1600 and j["height"] == 1200 and j["url"].startswith("/media/") and j["url"].endswith(".jpg")'
PHOTO=$(jget 'j["url"]')
call GET "$PHOTO"
check "GET /media/... com cache imutável" 200 'h.get("content-type") == "image/jpeg" and "immutable" in h.get("cache-control", "")'
python3 -I -c 'import sys; d=open(sys.argv[1],"rb").read(2); sys.exit(0 if d==b"\xff\xd8" else 1)' "$TMP/body" && pass "arquivo servido é JPEG" || fail "arquivo servido não é JPEG"
magick -size 300x300 xc:none -fill red -draw "circle 150,150 150,60" "$TMP/alpha.png"
CODE=$(curl -s -o "$TMP/body" -D "$TMP/headers" -w '%{http_code}' -F "file=@$TMP/alpha.png;type=image/png" "$BASE/api/upload")
check "PNG com transparência continua PNG" 200 'j["url"].endswith(".png") and j["width"] == 300'
PNG_URL=$(jget 'j["url"]')
magick -size 64x48 xc:orange "$TMP/opaque.webp"
CODE=$(curl -s -o "$TMP/body" -D "$TMP/headers" -w '%{http_code}' -F "file=@$TMP/opaque.webp;type=image/webp" "$BASE/api/upload")
check "WebP opaco vira JPEG" 200 'j["url"].endswith(".jpg") and j["width"] == 64 and j["height"] == 48'
echo "não sou imagem" > "$TMP/x.txt"
CODE=$(curl -s -o "$TMP/body" -D "$TMP/headers" -w '%{http_code}' -F "file=@$TMP/x.txt" "$BASE/api/upload")
check "upload que não é imagem → 415" 415 '"error" in j'
CODE=$(curl -s -o "$TMP/body" -D "$TMP/headers" -w '%{http_code}' -F "outro=@$TMP/x.txt" "$BASE/api/upload")
check "upload sem campo file → 400" 400
call GET /media/nao-existe.jpg
check "/media inexistente → 404" 404

section "Destinos e votação"
call POST /api/destinations '{"name":"Ilha do Francês","lat":-27.41,"lon":-48.47,"good_winds":["S"]}'
check "POST destino sem crew → 403" 403
call POST /api/destinations "{\"name\":\"Ilha do Francês\",\"lat\":-27.41,\"lon\":-48.47,\"photo\":\"$PHOTO\",\"photo_original\":\"$PHOTO\",\"good_winds\":[\"SE\",\"S\",\"S\"]}" crew="$CREW"
check "POST destino (crew)" 201 'j["name"] == "Ilha do Francês" and j["good_winds"] == ["SE", "S"] and j["votes"] == 0 and j["my_vote"] is False and j["created_by"] == "Luiz"'
D1=$(jget 'j["id"]')
call POST /api/destinations '{"name":"Jurerê","lat":-27.435,"lon":-48.50,"good_winds":["S","SO","O"]}' crew="$CREW"
check "POST destino 2" 201 'j["photo"] is None'
D2=$(jget 'j["id"]')
call POST /api/destinations '{"name":"Arraial do Cabo","lat":-22.97,"lon":-42.02,"good_winds":[]}' crew="$CREW"
check "POST destino fora da malha" 201
D3=$(jget 'j["id"]')
call POST /api/destinations '{"name":"X","lat":-27.4,"lon":-48.5,"good_winds":["Z"]}' crew="$CREW"
check "vento inválido → 400" 400
call PATCH "/api/destinations/$D2" '{"good_winds":["NO","N"],"photo":null}' crew="$CREW"
check "PATCH destino parcial" 200 'j["good_winds"] == ["N", "NO"] and j["name"] == "Jurerê"'
call POST "/api/destinations/$D1/vote" dev="$DEV_A"
check "voto A → D1" 200 "j['votes']['$D1'] == 1 and j['my_vote'] == $D1"
call POST "/api/destinations/$D1/vote" dev="$DEV_B"
check "voto B → D1" 200 "j['votes']['$D1'] == 2"
call POST "/api/destinations/$D2/vote" dev="$DEV_A"
check "A muda o voto para D2" 200 "j['votes']['$D1'] == 1 and j['votes']['$D2'] == 1 and j['my_vote'] == $D2"
call GET /api/destinations dev="$DEV_A"
check "GET destinos: votes e my_vote por aparelho" 200 "[(d['id'], d['votes'], d['my_vote']) for d in j][:2] == [($D1, 1, False), ($D2, 1, True)]"
call POST /api/destinations/99999/vote
check "voto em destino inexistente → 404" 404
call POST "/api/destinations/$D1/vote" dev=""
check "voto sem X-Device-Id → 400" 400
call DELETE /api/votes/mine dev="$DEV_A"
check "DELETE /api/votes/mine" 200 "j['my_vote'] is None and j['votes']['$D2'] == 0 and j['votes']['$D1'] == 1"
call POST /api/votes/reset
check "reset sem crew → 403" 403
call POST /api/votes/reset crew="$CREW"
check "reset (crew) zera tudo" 200 'all(v == 0 for v in j["votes"].values())'
call POST "/api/destinations/$D1/vote" dev="$DEV_A"
check "novo voto no turno atual" 200 "j['votes']['$D1'] == 1"
sql "UPDATE votes SET created_at = '2020-01-01T00:00:00Z' WHERE device_id = '$DEV_A'"
call GET /api/destinations dev="$DEV_A"
check "voto do turno anterior desaparece sem depender de cron" 200 "j[0]['votes'] == 0 and j[0]['my_vote'] is False"
call POST "/api/destinations/$D1/vote" dev="$DEV_A"
check "aparelho pode votar novamente no novo turno" 200 "j['votes']['$D1'] == 1 and j['my_vote'] == $D1"
call POST /api/votes/reset crew="$CREW"

section "Rotas (water_routing)"
call GET "/api/routes?lat=-27.425&lon=-48.495"
check "GET /api/routes: um resultado por destino" 200 "sorted(r['destination_id'] for r in j) == sorted([$D1, $D2, $D3])"
check "rotas: path começa na origem e termina no destino; distance ≥ straight" 200 'all(len(r["path"]) >= 2 and abs(r["path"][0][0] + 27.425) < 1e-6 and abs(r["path"][0][1] + 48.495) < 1e-6 and r["distance_m"] >= r["straight_m"] - 0.5 and isinstance(r["via_water"], bool) for r in j)'
check "destino fora da malha → via_water false, linha reta" 200 "(lambda r: r['via_water'] is False and r['distance_m'] == r['straight_m'] and len(r['path']) == 2 and abs(r['path'][1][0] + 22.97) < 1e-6)([r for r in j if r['destination_id'] == $D3][0])"
echo "    rotas: $(python3 -I -c 'import json,sys; print([(r["destination_id"], round(r["distance_m"]), round(r["straight_m"]), r["via_water"], len(r["path"])) for r in json.load(open(sys.argv[1]))])' "$TMP/body")"
call GET "/api/routes?lat=abc&lon=1"
check "rotas com lat inválida → 400" 400
call GET "/api/routes?lat=NaN&lon=-48.5"
check "rotas com NaN → 400" 400

section "Previsão (Open-Meteo real)"
call GET /api/weather
check "GET /api/weather: horas de hoje e amanhã" 200 'j["timezone"] == "America/Sao_Paulo" and len(j["hours"]) >= 24 and all(set(x) == {"time","wind_kn","gust_kn","wind_dir","wave_m","wave_dir","swell_m","swell_dir"} for x in j["hours"]) and any(isinstance(x["wind_kn"], (int, float)) for x in j["hours"]) and j["updated_at"].endswith("Z")'
check "vento sempre no ponto ao norte de Jurerê, não no GPS" 200 'j["location"] == "Jurerê Internacional" and j["lat"] == -27.425 and j["lon"] == -48.495'
echo "    $(python3 -I -c 'import json,sys; j=json.load(open(sys.argv[1])); hs=j["hours"]; print(len(hs), "horas;", sum(1 for x in hs if x["wave_m"] is not None), "com ondas; primeira:", hs[0])' "$TMP/body")"
call GET /api/weather
check "segunda chamada vem do cache" 200 'len(j["hours"]) >= 24'

section "Inventário"
call POST /api/tags '{"name":"Vodka","emoji":"🍸","unit":"L","min_qty":2}'
check "POST tag sem crew → 403" 403
call POST /api/tags '{"name":"Vodka","emoji":"🍸","unit":"L","min_qty":2}' crew="$CREW"
check "POST tag (crew)" 201 'j["unit"] == "L" and j["total"] == 0 and j["below_min"] is True and j["item_ids"] == []'
TAG=$(jget 'j["id"]')
call POST /api/tags '{"name":"Garrafas","unit":"un"}' crew="$CREW"
check "POST tag un" 201 'j["min_qty"] is None and j["below_min"] is False'
TAG_UN=$(jget 'j["id"]')
call POST /api/tags '{"name":"X","unit":"barril"}' crew="$CREW"
check "tag com unidade inválida → 400" 400
call POST /api/items "{\"name\":\"Absolut 750 ml\",\"tag_ids\":[$TAG,$TAG_UN],\"count_unit\":\"garrafa\",\"content_amount\":750,\"content_unit\":\"ml\",\"quantity\":1,\"min_qty\":2,\"fractional\":true,\"photo\":\"$PNG_URL\"}" crew="$CREW"
check "POST item vodka 750 ml ×1" 201 "j['quantity'] == 1 and j['tag_ids'] == sorted([$TAG, $TAG_UN]) and j['updated_by'] == 'Luiz' and j['below_min'] is True and j['photo_original'] is None"
I1=$(jget 'j["id"]')
call POST /api/items "{\"name\":\"Smirnoff 1 L\",\"tag_ids\":[$TAG],\"count_unit\":\"garrafa\",\"content_amount\":1,\"content_unit\":\"L\",\"quantity\":0.5,\"fractional\":true}" crew="$CREW"
check "POST item vodka 1 L ×0,5" 201 'j["quantity"] == 0.5 and j["min_qty"] is None and j["below_min"] is False'
I2=$(jget 'j["id"]')
call POST /api/items "{\"name\":\"Gelo\",\"tag_ids\":[$TAG],\"count_unit\":\"saco\",\"content_amount\":5,\"content_unit\":\"kg\",\"quantity\":3}" crew="$CREW"
check "POST item incompatível (kg) na tag em L" 201
I3=$(jget 'j["id"]')
call POST /api/items '{"name":"Teste","tag_ids":[99999]}' crew="$CREW"
check "item com tag inexistente → 400" 400
call GET /api/inventory
check "total da tag Vodka = 1,25 L (kg fica de fora) e abaixo do mínimo" 200 "(lambda t: t['total'] == 1.25 and t['below_min'] is True and sorted(t['item_ids']) == sorted([$I1, $I2, $I3]))([t for t in j['tags'] if t['id'] == $TAG][0])"
check "tag un soma quantity" 200 "[t for t in j['tags'] if t['id'] == $TAG_UN][0]['total'] == 1"
call POST "/api/items/$I1/adjust" '{"delta":2,"reason":"compra"}' crew="$CREW"
check "adjust delta +2 (compra)" 200 'j["quantity"] == 3 and j["below_min"] is False'
call POST "/api/items/$I2/adjust" '{"quantity":0.25,"reason":"conferencia"}' crew="$CREW"
check "adjust absoluto 0,25 (conferência)" 200 'j["quantity"] == 0.25'
call POST "/api/items/$I2/adjust" '{"delta":-5}' crew="$CREW"
check "adjust abaixo de zero fica em 0" 200 'j["quantity"] == 0'
call POST "/api/items/$I2/adjust" '{"quantity":1,"delta":1}' crew="$CREW"
check "adjust com quantity e delta → 400" 400
call POST "/api/items/$I2/adjust" '{"quantity":1,"reason":"furto"}' crew="$CREW"
check "adjust com motivo inválido → 400" 400
call POST "/api/items/$I2/adjust" '{"quantity":0.5}'
check "adjust sem crew → 403" 403
call GET /api/inventory
check "total após ajustes = 3×0,75 + 0 = 2,25 L, acima do mínimo" 200 "(lambda t: t['total'] == 2.25 and t['below_min'] is False)([t for t in j['tags'] if t['id'] == $TAG][0])"
call GET "/api/items/$I1/history"
check "histórico do item (mais recente primeiro)" 200 'len(j) == 2 and j[0]["reason"] == "compra" and j[0]["before"] == 1 and j[0]["after"] == 3 and j[0]["by"] == "Luiz" and j[0]["at"].endswith("Z")'
call PATCH "/api/items/$I3" '{"quantity":1.5,"tag_ids":[],"content_unit":null,"min_qty":null}' crew="$CREW"
check "PATCH item parcial" 200 'j["quantity"] == 1.5 and j["tag_ids"] == [] and j["content_unit"] is None and j["name"] == "Gelo"'
call PATCH "/api/tags/$TAG" '{"unit":"ml","min_qty":null}' crew="$CREW"
check "PATCH tag (L → ml)" 200 'j["unit"] == "ml" and j["total"] == 2250 and j["below_min"] is False'
call DELETE "/api/tags/$TAG_UN" crew="$CREW"
check "DELETE tag → 204" 204
call GET /api/inventory
check "tag removida dos itens" 200 "[i for i in j['items'] if i['id'] == $I1][0]['tag_ids'] == [$TAG] and len(j['tags']) == 1"
call DELETE "/api/items/$I3" crew="$CREW"
check "DELETE item → 204" 204
call GET /api/items/$I3/history
check "histórico de item apagado → 404" 404

section "Cardápio e pedidos"
call POST /api/menu '{"name":"Caipirinha","category":"drink"}'
check "POST menu sem crew → 403" 403
call POST /api/menu "{\"name\":\"Caipirinha\",\"category\":\"drink\",\"description\":\"Clássica\",\"recipe\":\"Ingredientes:\\n- 1 limão\",\"photo\":\"$PHOTO\",\"sort\":1}" crew="$CREW"
check "POST menu (crew)" 201 'j["available"] is True and j["sort"] == 1 and j["recipe"].startswith("Ingredientes")'
M1=$(jget 'j["id"]')
call POST /api/menu '{"name":"Cerveja","category":"bebida","available":false}' crew="$CREW"
check "POST menu indisponível" 201 'j["available"] is False and j["description"] == "" and j["photo"] is None'
M2=$(jget 'j["id"]')
call POST /api/menu '{"name":"X","category":"sobremesa"}' crew="$CREW"
check "categoria inválida → 400" 400
call GET /api/menu
check "GET /api/menu ordenado por sort" 200 "[m['id'] for m in j] == [$M2, $M1]"
call POST /api/orders "{\"menu_item_id\":$M2,\"for_name\":\"Ana\"}" dev="$DEV_B"
check "pedido de item indisponível → 400" 400
call POST /api/orders "{\"menu_item_id\":$M1,\"for_name\":\"Ana\",\"notes\":\"sem açúcar\"}" dev="$DEV_B"
check "POST /api/orders (passageiro)" 201 "j['status'] == 'recebido' and j['item_name'] == 'Caipirinha' and j['item_photo'] == '$PHOTO' and j['category'] == 'drink' and j['recipe'].startswith('Ingredientes') and j['handled_by'] is None and j['notes'] == 'sem açúcar' and 'device_id' not in j"
O1=$(jget 'j["id"]')
call PATCH "/api/menu/$M1" '{"recipe":"Receita nova"}' crew="$CREW"
check "PATCH menu (receita)" 200 'j["recipe"] == "Receita nova" and j["name"] == "Caipirinha"'
call GET /api/orders dev="$DEV_B"
check "passageiro vê o próprio pedido (com receita atual)" 200 "len(j) == 1 and j[0]['id'] == $O1 and j[0]['recipe'] == 'Receita nova'"
call GET /api/orders dev="$DEV_C"
check "outro aparelho não vê" 200 'j == []'
call GET /api/orders dev="$DEV_C" crew="$CREW"
check "tripulação vê todos" 200 "[o['id'] for o in j] == [$O1]"
call PATCH "/api/orders/$O1" '{"status":"preparando"}' dev="$DEV_B"
check "passageiro não pode mudar para preparando → 403" 403
call PATCH "/api/orders/$O1" '{"status":"cancelado"}' dev="$DEV_C"
check "outro aparelho não pode cancelar → 403" 403
call PATCH "/api/orders/$O1" '{"status":"preparando"}' crew="$CREW"
check "crew → preparando (handled_by)" 200 'j["status"] == "preparando" and j["handled_by"] == "Luiz"'
call PATCH "/api/orders/$O1" '{"status":"cancelado"}' dev="$DEV_B"
check "dono não cancela depois de preparando → 409" 409
call PATCH "/api/orders/$O1" '{"status":"pronto"}' crew="$CREW"
check "crew → pronto" 200 'j["status"] == "pronto"'
call PATCH "/api/orders/$O1" '{"status":"entregue"}' crew="$CREW"
check "crew → entregue" 200 'j["status"] == "entregue"'
call PATCH "/api/orders/$O1" '{"status":"voando"}' crew="$CREW"
check "status inválido → 400" 400
call POST /api/orders "{\"menu_item_id\":$M1,\"for_name\":\"Bia\"}" dev="$DEV_B"
check "segundo pedido" 201
O2=$(jget 'j["id"]')
call PATCH "/api/orders/$O2" '{"status":"cancelado"}' dev="$DEV_B"
check "dono cancela enquanto recebido" 200 'j["status"] == "cancelado" and j["handled_by"] is None'
call GET /api/orders crew="$CREW"
check "crew vê finalizados das últimas 12 h" 200 "sorted(o['id'] for o in j) == sorted([$O1, $O2])"
call DELETE "/api/menu/$M1" crew="$CREW"
check "DELETE menu → 204" 204
call GET /api/orders dev="$DEV_B"
check "pedido mantém nome/foto após apagar o item" 200 "[o for o in j if o['id'] == $O1][0]['item_name'] == 'Caipirinha' and [o for o in j if o['id'] == $O1][0]['recipe'] == ''"
call POST /api/orders '{"menu_item_id":99999,"for_name":"Ana"}'
check "pedido de item inexistente → 404" 404

section "Push (inscrição)"
P256='BGa4N1PI79lboMR_YrwCiCsgp35DRvedt7opHcf0yM3iOBTSoQYqQLwWxAfRKE6tsDnReWmhsImkhDF_DBdkNSU'
AUTH='EvcWjEgzr4rbvhfi3yds0A'
call POST /api/push/subscribe "{\"subscription\":{\"endpoint\":\"https://web.push.apple.com/test-fixture\",\"expirationTime\":null,\"keys\":{\"p256dh\":\"$P256\",\"auth\":\"$AUTH\"}}}" crew="$CREW"
check "subscribe → 204" 204
[ "$(sql "SELECT device_id || '|' || crew_id FROM push_subs")" = "$DEV_A|$CREW" ] && pass "inscrição gravada com device e crew" || fail "inscrição não gravada corretamente"
call POST /api/push/subscribe "{\"subscription\":{\"endpoint\":\"https://web.push.apple.com/test-fixture\",\"keys\":{\"p256dh\":\"$P256\",\"auth\":\"$AUTH\"}}}" dev="$DEV_B"
check "reenvio atualiza → 204" 204
[ "$(sql "SELECT COUNT(*) || '|' || device_id || '|' || IFNULL(crew_id, 'null') FROM push_subs")" = "1|$DEV_B|null" ] && pass "reenvio atualizou (sem duplicar)" || fail "reenvio não atualizou"
call POST /api/push/subscribe '{"subscription":{"endpoint":"https://web.push.apple.com/test-invalid-keys","keys":{"p256dh":"abc","auth":"def"}}}'
check "chaves inválidas → 400" 400
call POST /api/push/unsubscribe '{"endpoint":"https://web.push.apple.com/test-fixture"}'
check "unsubscribe → 204" 204
[ "$(sql "SELECT COUNT(*) FROM push_subs")" = "0" ] && pass "inscrição removida" || fail "inscrição não removida"

section "IA (sem chave)"
call POST /api/ai/menu-text '{"name":"Caipirinha de maracujá","category":"drink"}'
check "menu-text sem crew → 403" 403
call POST /api/ai/menu-text '{"name":"Caipirinha de maracujá","category":"drink"}' crew="$CREW"
check "menu-text sem chave → 503" 503 'j == {"error": "Chave do OpenRouter não configurada"}'
call POST /api/ai/menu-photo '{"name":"Caipirinha","category":"drink","description":""}' crew="$CREW"
check "menu-photo sem chave → 503" 503 'j["error"] == "Chave do OpenRouter não configurada"'
call POST /api/ai/clean-photo "{\"url\":\"$PHOTO\"}" crew="$CREW"
check "clean-photo sem chave → 503" 503
call POST /api/ai/enhance-photo "{\"url\":\"$PHOTO\"}" crew="$CREW"
check "enhance-photo sem chave → 503" 503
call GET /api/ai/jobs/00000000-0000-0000-0000-000000000000
check "job inexistente → 404" 404
call GET /api/ai/models
check "GET /api/ai/models: listas não vazias" 200 'len(j["text"]) > 0 and len(j["image"]) > 0 and all(set(m) == {"id","name","provider"} for m in j["text"]) and all(set(m) == {"id","name","provider","image_input"} for m in j["image"])'
check "modelos ordenados por provider/nome e provider = prefixo" 200 'all(m["provider"] == m["id"].split("/")[0] for m in j["text"] + j["image"]) and [(m["provider"].lower(), m["name"].lower()) for m in j["text"]] == sorted((m["provider"].lower(), m["name"].lower()) for m in j["text"])'
echo "    $(jget '(len(j["text"]), len(j["image"]), sum(m["image_input"] for m in j["image"]))') (texto, imagem, imagem com entrada de imagem)"

section "Velocidade (2 posições)"
call POST /api/position '{"lat":-27.429,"lon":-48.49,"accuracy_m":8,"speed_mps":5,"heading":90}'
check "posição com speed_mps" 204
call GET /api/status
check "speed_mps tem prioridade: 5 m/s = 9,7 kn, moving" 200 'j["speed_kn"] == 9.7 and j["moving"] is True and j["heading"] == 90'
NOW=$(date +%s.%N)
WAIT=$(python3 -I -c 'import sys; print(max(0.0, 21.5 - (float(sys.argv[2]) - float(sys.argv[1]))))' "$T_A" "$NOW")
echo "    aguardando ${WAIT}s para completar 20 s desde a posição A..."
sleep "$WAIT"
T_B=$(date +%s.%N)
call POST /api/position '{"lat":-27.428,"lon":-48.49,"accuracy_m":8}'
check "posição B sem speed_mps" 204
call GET /api/status
EXPECTED=$(python3 -I -c 'import math,sys; d=2*6371008.8*math.asin(abs(math.sin(math.radians(0.002)/2))); print(d/(float(sys.argv[2])-float(sys.argv[1]))*3600/1852)' "$T_A" "$T_B")
check "velocidade calculada a partir de A (~${EXPECTED%.*} kn), rumo norte" 200 "abs(j['speed_kn'] - $EXPECTED) <= max(0.6, 0.1 * $EXPECTED) and j['moving'] is True and j['heading'] in (0, 360)"

section "SSE"
sleep 0.5
kill "$SSE_PID" 2>/dev/null
wait "$SSE_PID" 2>/dev/null
SSE_PID=""
for ev in crew settings destinations votes inventory menu order_new order_update status; do
  grep -q "^event: $ev$" "$TMP/sse.txt" && pass "SSE recebeu '$ev'" || fail "SSE não recebeu '$ev'"
done
python3 -I - "$TMP/sse.txt" <<'PY' && pass "eventos SSE com data JSON válido" || fail "data inválido no SSE"
import json, sys
lines = open(sys.argv[1], encoding="utf-8").read().splitlines()
datas = [l[5:].strip() for l in lines if l.startswith("data:")]
assert datas, "nenhum data"
for d in datas:
    json.loads(d)
ev = None
for l in lines:
    if l.startswith("event:"):
        ev = l[6:].strip()
    elif l.startswith("data:") and ev == "status":
        s = json.loads(l[5:])
        assert {"lat", "lon", "speed_kn", "moving", "heading", "updated_at", "stale"} <= set(s), s
PY

section "Resultado"
[ "$(sql "SELECT count(*) FROM crew_activity WHERE crew_name='Luiz' AND method='POST' AND path='/api/menu'")" = 2 ] && pass "cadastros com autoria" || fail "autoria dos cadastros"
echo "  $PASS ok, $FAIL falha(s)"
if [ "$FAIL" -gt 0 ]; then
  echo "--- últimas linhas do log do servidor ---"
  tail -n 40 "$TMP/server.log"
  exit 1
fi
grep -E "ERROR|panicked" "$TMP/server.log" && { echo "erros no log do servidor"; exit 1; }
exit 0
