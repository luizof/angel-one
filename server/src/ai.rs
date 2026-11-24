use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{ApiError, ApiResult};
use crate::extract::{Crew, JsonBody, PathParam};
use crate::media::{self, OutFmt};
use crate::settings;
use crate::state::AppState;

const OPENROUTER: &str = "https://openrouter.ai/api/v1";
const JOB_TTL: Duration = Duration::from_secs(3600);
const LIST_TTL: Duration = Duration::from_secs(3600);
const GEN_TIMEOUT: Duration = Duration::from_secs(180);

// ---------- jobs ----------

pub struct Job {
    status: &'static str,
    result: Option<Value>,
    error: Option<String>,
    created: Instant,
}

fn prune_jobs(st: &AppState) {
    let mut jobs = st.jobs.lock().unwrap_or_else(|e| e.into_inner());
    jobs.retain(|_, j| j.created.elapsed() < JOB_TTL);
}

/// Tarefa periódica que remove jobs com mais de 1 h.
pub fn spawn_janitor(st: AppState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(600));
        loop {
            tick.tick().await;
            prune_jobs(&st);
        }
    });
}

fn start_job<F>(st: &AppState, kind: &'static str, fut: F) -> (StatusCode, Json<Value>)
where
    F: Future<Output = Result<Value, String>> + Send + 'static,
{
    prune_jobs(st);
    let id = uuid::Uuid::new_v4().to_string();
    st.jobs.lock().unwrap_or_else(|e| e.into_inner()).insert(
        id.clone(),
        Job { status: "pending", result: None, error: None, created: Instant::now() },
    );
    let st2 = st.clone();
    let id2 = id.clone();
    tokio::spawn(async move {
        let started = Instant::now();
        let res = fut.await;
        let secs = started.elapsed().as_secs_f32();
        let mut jobs = st2.jobs.lock().unwrap_or_else(|e| e.into_inner());
        let Some(job) = jobs.get_mut(&id2) else { return };
        match res {
            Ok(v) => {
                tracing::info!("job de IA {kind} {id2} concluído em {secs:.1}s");
                job.status = "done";
                job.result = Some(v);
            }
            Err(e) => {
                tracing::warn!("job de IA {kind} {id2} falhou em {secs:.1}s: {e}");
                job.status = "error";
                job.error = Some(e);
            }
        }
    });
    (StatusCode::ACCEPTED, Json(json!({ "job_id": id })))
}

/// `GET /api/ai/jobs/:id`
pub async fn job(State(st): State<AppState>, PathParam(id): PathParam<String>) -> ApiResult<Json<Value>> {
    prune_jobs(&st);
    let jobs = st.jobs.lock().unwrap_or_else(|e| e.into_inner());
    let j = jobs.get(&id).ok_or_else(|| ApiError::not_found("Tarefa não encontrada (ou expirada)"))?;
    Ok(Json(json!({ "id": id, "status": j.status, "result": j.result, "error": j.error })))
}

async fn require_key(st: &AppState) -> ApiResult<String> {
    settings::openrouter_key(st)
        .await?
        .ok_or_else(|| ApiError::unavailable("Chave do OpenRouter não configurada"))
}

// ---------- listas de modelos (públicas, cache 1 h) ----------

pub struct ListCache {
    at: Instant,
    data: Arc<Vec<Value>>,
}

async fn fetch_list(st: &AppState, cache_key: &'static str, url: String) -> Result<Arc<Vec<Value>>, String> {
    let mut lists = st.lists.lock().await;
    if let Some(c) = lists.get(cache_key).filter(|c| c.at.elapsed() < LIST_TTL) {
        return Ok(c.data.clone());
    }
    let fetched = async {
        let res = st
            .http
            .get(&url)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| format!("{url}: {e}"))?;
        let status = res.status();
        if !status.is_success() {
            return Err(format!("{url}: HTTP {status}"));
        }
        let v: Value = res.json().await.map_err(|e| format!("{url}: {e}"))?;
        v.get("data")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| format!("{url}: sem 'data'"))
    }
    .await;
    match fetched {
        Ok(data) => {
            let data = Arc::new(data);
            lists.insert(cache_key, ListCache { at: Instant::now(), data: data.clone() });
            Ok(data)
        }
        Err(e) => match lists.get(cache_key) {
            Some(c) => {
                tracing::warn!("usando lista em cache: {e}");
                Ok(c.data.clone())
            }
            None => Err(e),
        },
    }
}

async fn image_models(st: &AppState) -> Result<Arc<Vec<Value>>, String> {
    fetch_list(st, "image", format!("{OPENROUTER}/images/models")).await
}

fn str_list<'a>(v: &'a Value, path: &[&str]) -> Vec<&'a str> {
    let mut cur = v;
    for p in path {
        match cur.get(*p) {
            Some(n) => cur = n,
            None => return Vec::new(),
        }
    }
    cur.as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default()
}

fn summary(m: &Value) -> Option<(String, String, String)> {
    let id = m.get("id")?.as_str()?.to_string();
    let name = m.get("name").and_then(Value::as_str).unwrap_or(&id).to_string();
    let provider = id.split('/').next().unwrap_or("").to_string();
    Some((id, name, provider))
}

/// `GET /api/ai/models` (sem chave)
pub async fn models(State(st): State<AppState>) -> ApiResult<Json<Value>> {
    let (text, image) =
        tokio::join!(fetch_list(&st, "text", format!("{OPENROUTER}/models")), image_models(&st));
    let (text, image) = match (text, image) {
        (Ok(t), Ok(i)) => (t, i),
        (Err(e), _) | (_, Err(e)) => {
            tracing::warn!("lista de modelos indisponível: {e}");
            return Err(ApiError::bad_gateway("Lista de modelos do OpenRouter indisponível"));
        }
    };
    let sort_key = |v: &Value| {
        (
            v["provider"].as_str().unwrap_or("").to_lowercase(),
            v["name"].as_str().unwrap_or("").to_lowercase(),
        )
    };
    let mut text_out: Vec<Value> = text
        .iter()
        .filter(|m| str_list(m, &["architecture", "output_modalities"]).contains(&"text"))
        .filter_map(summary)
        .map(|(id, name, provider)| json!({ "id": id, "name": name, "provider": provider }))
        .collect();
    let mut image_out: Vec<Value> = image
        .iter()
        .filter_map(|m| {
            let (id, name, provider) = summary(m)?;
            let image_input = str_list(m, &["architecture", "input_modalities"]).contains(&"image");
            Some(json!({ "id": id, "name": name, "provider": provider, "image_input": image_input }))
        })
        .collect();
    text_out.sort_by_key(sort_key);
    image_out.sort_by_key(sort_key);
    Ok(Json(json!({ "text": text_out, "image": image_out })))
}

// ---------- chamadas ao OpenRouter ----------

fn openrouter_error(status: reqwest::StatusCode, body: &str) -> String {
    let msg = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            let e = v.get("error")?;
            e.get("message").and_then(Value::as_str).map(str::to_string).or_else(|| e.as_str().map(str::to_string))
        })
        .unwrap_or_else(|| body.chars().take(200).collect());
    format!("OpenRouter respondeu {}: {msg}", status.as_u16())
}

async fn post_openrouter(st: &AppState, key: &str, path: &str, body: &Value) -> Result<Value, String> {
    let res = st
        .http
        .post(format!("{OPENROUTER}{path}"))
        .bearer_auth(key)
        .header("X-Title", "Angel One")
        .timeout(GEN_TIMEOUT)
        .json(body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "Tempo esgotado esperando o OpenRouter".to_string()
            } else {
                format!("Falha ao contatar o OpenRouter: {e}")
            }
        })?;
    let status = res.status();
    let text = res.text().await.map_err(|e| format!("Falha ao ler a resposta do OpenRouter: {e}"))?;
    if !status.is_success() {
        return Err(openrouter_error(status, &text));
    }
    let v: Value = serde_json::from_str(&text).map_err(|_| "Resposta inválida do OpenRouter".to_string())?;
    if v.get("error").is_some_and(|e| !e.is_null()) {
        return Err(openrouter_error(status, &text));
    }
    Ok(v)
}

fn category_pt(c: &str) -> &'static str {
    match c {
        "drink" => "drink (coquetel preparado no bar de bordo)",
        "petisco" => "petisco (comida/aperitivo preparado na cozinha de bordo)",
        _ => "bebida (pronta: cerveja, água, refrigerante, vinho, suco etc.)",
    }
}

const MENU_TEXT_SYSTEM: &str = "Você é o bartender e chef de bordo do iate Angel One, que navega pelo norte da Ilha de Santa Catarina (Brasil). Escreva textos de cardápio em português do Brasil.

Responda SOMENTE com um objeto JSON válido, com exatamente duas chaves de texto: \"description\" e \"recipe\".

- \"description\": uma frase curta, convidativa e saborosa (até ~120 caracteres) para o cardápio. Não repita o nome do item, não use emojis, aspas nem preço.
- \"recipe\": receita prática para preparar a bordo, como texto simples (sem markdown, sem negrito), exatamente neste formato:
Ingredientes:
- quantidade + ingrediente
- ...

Preparo:
1. passo
2. passo

Use medidas de bar/caseiras (ml, doses, colheres, unidades), poucos passos e utensílios simples. Para drinks, inclua gelo, copo e guarnição. Para a categoria \"bebida\" (cerveja, água, refrigerante, vinho, suco pronto etc.) a receita pode ser uma string vazia ou apenas uma instrução curta de serviço, como \"Servir bem gelada.\".";

fn extract_json_object(s: &str) -> Option<Value> {
    if let Ok(v) = serde_json::from_str::<Value>(s.trim())
        && v.is_object()
    {
        return Some(v);
    }
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    serde_json::from_str::<Value>(s.get(start..=end)?).ok().filter(Value::is_object)
}

fn text_field(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim().to_string(),
        Value::Array(a) => a
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string).or_else(|| (!x.is_null()).then(|| x.to_string())))
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

async fn run_menu_text(st: AppState, key: String, model: String, name: String, category: String) -> Result<Value, String> {
    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": MENU_TEXT_SYSTEM },
            { "role": "user", "content": format!("Item: {name}\nCategoria: {}", category_pt(&category)) }
        ],
        "response_format": { "type": "json_object" },
        "temperature": 0.7
    });
    let v = post_openrouter(&st, &key, "/chat/completions", &body).await?;
    let content = &v["choices"][0]["message"]["content"];
    let content = match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join(""),
        _ => return Err("O modelo não devolveu texto".into()),
    };
    let obj = extract_json_object(&content).ok_or("O modelo não devolveu um JSON válido")?;
    let description = text_field(&obj["description"]);
    let recipe = text_field(&obj["recipe"]);
    if description.is_empty() {
        return Err("O modelo não devolveu a descrição".into());
    }
    Ok(json!({ "description": description, "recipe": recipe }))
}

/// Parâmetros suportados pelo modelo de imagem (`None` se a lista não estiver disponível).
async fn image_params(st: &AppState, model: &str) -> Option<Value> {
    let list = image_models(st).await.ok()?;
    list.iter().find(|m| m["id"].as_str() == Some(model)).map(|m| m["supported_parameters"].clone())
}

fn enum_supports(params: &Option<Value>, field: &str, value: &str) -> Option<bool> {
    let p = params.as_ref()?;
    let d = p.get(field)?;
    match d.get("values").and_then(Value::as_array) {
        Some(vals) => Some(vals.iter().any(|v| v.as_str() == Some(value))),
        None => Some(true),
    }
}

fn closest_ratio(params: &Option<Value>, w: u32, h: u32) -> String {
    const COMMON: [&str; 9] = ["1:1", "4:3", "3:4", "3:2", "2:3", "16:9", "9:16", "4:5", "5:4"];
    let allowed: Vec<String> = params
        .as_ref()
        .and_then(|p| p.get("aspect_ratio"))
        .and_then(|d| d.get("values"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_else(|| COMMON.iter().map(|s| s.to_string()).collect());
    let target = (w.max(1) as f64 / h.max(1) as f64).ln();
    allowed
        .iter()
        .filter_map(|r| {
            let (a, b) = r.split_once(':')?;
            let v = (a.parse::<f64>().ok()? / b.parse::<f64>().ok()?).ln();
            Some((r.clone(), (v - target).abs()))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(r, _)| r)
        .unwrap_or_else(|| "1:1".into())
}

enum Aspect {
    Fixed(&'static str),
    /// Proporção suportada mais próxima da foto original (largura, altura).
    Like(u32, u32),
}

struct ImageJob {
    model: String,
    prompt: String,
    aspect: Aspect,
    reference: Option<String>,
    out: OutFmt,
}

async fn run_image(st: AppState, key: String, job: ImageJob) -> Result<Value, String> {
    let params = image_params(&st, &job.model).await;
    let mut body = json!({ "model": job.model, "prompt": job.prompt });
    let aspect_ratio = match job.aspect {
        Aspect::Fixed(r) => r.to_string(),
        Aspect::Like(w, h) => closest_ratio(&params, w, h),
    };
    if enum_supports(&params, "aspect_ratio", &aspect_ratio) != Some(false) {
        body["aspect_ratio"] = json!(aspect_ratio);
    }
    if enum_supports(&params, "resolution", "1K") == Some(true) {
        body["resolution"] = json!("1K");
    }
    if let Some(r) = &job.reference {
        body["input_references"] = json!([{ "type": "image_url", "image_url": { "url": r } }]);
    }
    let v = post_openrouter(&st, &key, "/images", &body).await?;
    let first = v["data"].get(0).ok_or("O OpenRouter não devolveu nenhuma imagem")?;
    if first["media_type"].as_str().is_some_and(|t| t.contains("svg")) {
        return Err("O modelo devolveu SVG; escolha um modelo de foto".into());
    }
    let bytes = if let Some(b64) = first["b64_json"].as_str() {
        let b64 = b64.split_once("base64,").map(|(_, d)| d).unwrap_or(b64);
        STANDARD
            .decode(b64.trim())
            .map_err(|_| "Imagem do OpenRouter em base64 inválido".to_string())?
    } else if let Some(url) = first["url"].as_str() {
        let res = st.http.get(url).timeout(Duration::from_secs(60)).send().await.map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            return Err(format!("Falha ao baixar a imagem gerada: HTTP {}", res.status()));
        }
        res.bytes().await.map_err(|e| e.to_string())?.to_vec()
    } else {
        return Err("Resposta do OpenRouter sem imagem".into());
    };
    let dir = st.cfg.media_dir.clone();
    let saved = tokio::task::spawn_blocking(move || media::save_image(&dir, &bytes, job.out))
        .await
        .map_err(|_| "Falha ao processar a imagem".to_string())?
        .map_err(|e| e.message)?;
    Ok(json!({ "url": saved.url }))
}

/// Lê uma foto de `/media` e devolve (data URL, largura, altura).
async fn reference_image(st: &AppState, url: &str) -> ApiResult<(String, u32, u32)> {
    let path = media::media_path(&st.cfg.media_dir, url)?;
    tokio::task::spawn_blocking(move || {
        let bytes = std::fs::read(&path).map_err(|_| ApiError::not_found("Imagem não encontrada"))?;
        let format = image::guess_format(&bytes).map_err(|_| ApiError::bad_request("Arquivo não é uma imagem"))?;
        let mime = match format {
            image::ImageFormat::Png => "image/png",
            image::ImageFormat::WebP => "image/webp",
            image::ImageFormat::Jpeg => "image/jpeg",
            _ => return Err(ApiError::bad_request("Formato de imagem não suportado")),
        };
        let (w, h) = image::ImageReader::new(std::io::Cursor::new(&bytes))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.into_dimensions().ok())
            .unwrap_or((1, 1));
        Ok((format!("data:{mime};base64,{}", STANDARD.encode(&bytes)), w, h))
    })
    .await
    .map_err(|_| ApiError::internal("Falha ao ler a imagem"))?
}

// ---------- prompts de imagem ----------

fn menu_photo_prompt(name: &str, category: &str, description: &str) -> String {
    let subject = match category {
        "drink" => "a freshly made cocktail served in the most appropriate glass, with ice and a fitting fresh garnish, tiny condensation droplets on the glass",
        "petisco" => "an appetizing snack / finger-food portion, beautifully plated on a simple elegant serving dish",
        _ => "a perfectly chilled beverage served as it would be on board (bottle, can or glass as appropriate), with condensation droplets",
    };
    let desc = if description.trim().is_empty() {
        String::new()
    } else {
        format!(" Menu description (Brazilian Portuguese, for context): \"{}\".", description.trim())
    };
    format!(
        "Premium hand-drawn 2D food and beverage illustration for the tropical yacht Angel One's menu, NOT a photograph and NOT 3D. \
         Subject: \"{name}\" — {subject}.{desc} \
         Single appetizing illustrated hero centered and complete, recognizable actual ingredients and serving style. \
         Style: clean expressive ink contours, layered flat colors, subtly textured gouache/paper grain, elegant 2D editorial travel-poster illustration; no photographic textures, no rendering. \
         Warm late-afternoon sunshine and playful tropical palette; simplified yacht deck and turquoise sea behind the item, softly drawn but not blurred photographic bokeh. \
         Square 1:1 composition, polished joyful hand-painted illustration with plenty of visual clarity. \
         Absolutely no text, letters, labels with words, logos, watermarks, people or hands."
    )
}

const CLEAN_PHOTO_PROMPT: &str = "Transform the photographed product into a clean, charming HAND-DRAWN 2D editorial illustration on a pure white background. No photographic rendering or 3D. \
Use the product shown in the reference photo and keep it exactly as it is: identical shape, proportions, colors, materials, cap, label design, printed text and branding — do not redesign, re-letter, translate or alter anything on it. \
Remove everything else (hands, table, shelves, clutter, other objects and the original background). \
Place the complete product upright and centered on a pure white (#FFFFFF) seamless background. Use confident fine ink outlines, flat but beautifully layered tropical colors and subtle paper/grain texture, a tiny drawn contact shadow. \
Clearly readable silhouette, true-to-original branding where visible, polished 2D product illustration for a yacht inventory. Do not add props, new text or watermark.";

const ENHANCE_PHOTO_PROMPT: &str = "Turn this photograph of a real destination into a BEAUTIFUL HAND-DRAWN 2D illustrated travel postcard in the Angel One tropical visual language. \
Keep the same recognizable geographical location, island shapes, shoreline, vegetation, buildings, boats, composition/framing and time of day. Do NOT invent or move any landmark or structures. \
Draw simplified layered shapes, crisp ink contours, warm sunlight, vivid turquoise and emerald sea, subtly grained gouache/paper texture; polished flat 2D illustration (not photography, 3D or anime). \
No text, borders, watermark or invented people.";

// ---------- rotas ----------

#[derive(Deserialize)]
pub struct MenuTextIn {
    name: String,
    category: Option<String>,
}

/// `POST /api/ai/menu-text` (crew)
pub async fn menu_text(
    State(st): State<AppState>,
    _crew: Crew,
    JsonBody(b): JsonBody<MenuTextIn>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let name = crate::util::required_text(&b.name, "name", 80)?;
    let category = b.category.unwrap_or_else(|| "drink".into()).trim().to_lowercase();
    let key = require_key(&st).await?;
    let model = settings::get_settings(&st).await?.text_model;
    Ok(start_job(&st, "menu-text", run_menu_text(st.clone(), key, model, name, category)))
}

#[derive(Deserialize)]
pub struct MenuPhotoIn {
    name: String,
    category: Option<String>,
    description: Option<String>,
}

/// `POST /api/ai/menu-photo` (crew)
pub async fn menu_photo(
    State(st): State<AppState>,
    _crew: Crew,
    JsonBody(b): JsonBody<MenuPhotoIn>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let name = crate::util::required_text(&b.name, "name", 80)?;
    let category = b.category.unwrap_or_else(|| "drink".into()).trim().to_lowercase();
    let description = crate::util::limited_text(b.description.as_deref().unwrap_or(""), "description", 500)?;
    let key = require_key(&st).await?;
    let model = settings::get_settings(&st).await?.image_model;
    let job = ImageJob {
        model,
        prompt: menu_photo_prompt(&name, &category, &description),
        aspect: Aspect::Fixed("1:1"),
        reference: None,
        out: OutFmt::Jpeg,
    };
    Ok(start_job(&st, "menu-photo", run_image(st.clone(), key, job)))
}

#[derive(Deserialize)]
pub struct PhotoIn {
    url: String,
}

/// `POST /api/ai/clean-photo` (crew) — produto em fundo branco (PNG).
pub async fn clean_photo(
    State(st): State<AppState>,
    _crew: Crew,
    JsonBody(b): JsonBody<PhotoIn>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let key = require_key(&st).await?;
    let (data_url, _, _) = reference_image(&st, &b.url).await?;
    let model = settings::get_settings(&st).await?.edit_model;
    let job = ImageJob {
        model,
        prompt: CLEAN_PHOTO_PROMPT.into(),
        aspect: Aspect::Fixed("1:1"),
        reference: Some(data_url),
        out: OutFmt::Png,
    };
    Ok(start_job(&st, "clean-photo", run_image(st.clone(), key, job)))
}

/// `POST /api/ai/enhance-photo` (crew) — melhora a foto de um lugar (JPEG).
pub async fn enhance_photo(
    State(st): State<AppState>,
    _crew: Crew,
    JsonBody(b): JsonBody<PhotoIn>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let key = require_key(&st).await?;
    let (data_url, w, h) = reference_image(&st, &b.url).await?;
    let model = settings::get_settings(&st).await?.edit_model;
    let job = ImageJob {
        aspect: Aspect::Like(w, h),
        model,
        prompt: ENHANCE_PHOTO_PROMPT.into(),
        reference: Some(data_url),
        out: OutFmt::Jpeg,
    };
    Ok(start_job(&st, "enhance-photo", run_image(st.clone(), key, job)))
}
