use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ApiResult};
use crate::events;
use crate::extract::{Crew, Device, JsonBody, OptCrew, PathParam};
use crate::push::{self, PushMsg, Target};
use crate::state::AppState;
use crate::util::{double_option, limited_text, now_str, nullable_text, required_text, ts};

pub const CATEGORIES: [&str; 3] = ["drink", "petisco", "bebida"];
pub const STATUSES: [&str; 5] = ["recebido", "preparando", "pronto", "entregue", "cancelado"];
const RECENT_HOURS: i64 = 12;

fn check_category(c: &str) -> ApiResult<String> {
    let c = c.trim().to_lowercase();
    if CATEGORIES.contains(&c.as_str()) {
        Ok(c)
    } else {
        Err(ApiError::bad_request("Categoria inválida (use drink, petisco ou bebida)"))
    }
}

// ---------- cardápio ----------

#[derive(Serialize)]
pub struct MenuOut {
    pub id: i64,
    pub name: String,
    pub category: String,
    pub description: String,
    pub recipe: String,
    pub photo: Option<String>,
    pub available: bool,
    pub sort: i64,
    pub created_at: String,
}

const MENU_COLS: &str = "id, name, category, description, recipe, photo, available, sort, created_at";

fn menu_row(r: &rusqlite::Row) -> rusqlite::Result<MenuOut> {
    Ok(MenuOut {
        id: r.get(0)?,
        name: r.get(1)?,
        category: r.get(2)?,
        description: r.get(3)?,
        recipe: r.get(4)?,
        photo: r.get(5)?,
        available: r.get(6)?,
        sort: r.get(7)?,
        created_at: r.get(8)?,
    })
}

fn get_menu_item(c: &Connection, id: i64) -> ApiResult<MenuOut> {
    c.query_row(&format!("SELECT {MENU_COLS} FROM menu_items WHERE id = ?1"), [id], menu_row)
        .optional()?
        .ok_or_else(|| ApiError::not_found("Item do cardápio não encontrado"))
}

/// `GET /api/menu`
pub async fn list(State(st): State<AppState>) -> ApiResult<Json<Vec<MenuOut>>> {
    let v = st
        .db
        .call(|c| {
            let mut s = c.prepare_cached(&format!("SELECT {MENU_COLS} FROM menu_items ORDER BY sort, id"))?;
            let v = s.query_map([], menu_row)?.collect::<Result<Vec<_>, _>>()?;
            Ok(v)
        })
        .await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct MenuIn {
    name: String,
    category: String,
    description: Option<String>,
    recipe: Option<String>,
    photo: Option<String>,
    available: Option<bool>,
    sort: Option<i64>,
}

/// `POST /api/menu` (crew)
pub async fn create(
    State(st): State<AppState>,
    _crew: Crew,
    JsonBody(b): JsonBody<MenuIn>,
) -> ApiResult<(StatusCode, Json<MenuOut>)> {
    let name = required_text(&b.name, "name", 80)?;
    let category = check_category(&b.category)?;
    let description = limited_text(b.description.as_deref().unwrap_or(""), "description", 500)?;
    let recipe = limited_text(b.recipe.as_deref().unwrap_or(""), "recipe", 5000)?;
    let photo = nullable_text(b.photo.as_deref(), "photo", 300)?;
    let available = b.available.unwrap_or(true);
    let sort = b.sort.unwrap_or(0);
    let out = st
        .db
        .call(move |c| {
            c.execute(
                "INSERT INTO menu_items(name, category, description, recipe, photo, available, sort, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![name, category, description, recipe, photo, available, sort, now_str()],
            )?;
            get_menu_item(c, c.last_insert_rowid())
        })
        .await?;
    events::publish_reload(&st, "menu");
    Ok((StatusCode::CREATED, Json(out)))
}

#[derive(Deserialize)]
pub struct MenuPatch {
    name: Option<String>,
    category: Option<String>,
    description: Option<String>,
    recipe: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    photo: Option<Option<String>>,
    available: Option<bool>,
    sort: Option<i64>,
}

/// `PATCH /api/menu/:id` (crew)
pub async fn update(
    State(st): State<AppState>,
    _crew: Crew,
    PathParam(id): PathParam<i64>,
    JsonBody(b): JsonBody<MenuPatch>,
) -> ApiResult<Json<MenuOut>> {
    let name = b.name.as_deref().map(|n| required_text(n, "name", 80)).transpose()?;
    let category = b.category.as_deref().map(check_category).transpose()?;
    let description = b.description.as_deref().map(|d| limited_text(d, "description", 500)).transpose()?;
    let recipe = b.recipe.as_deref().map(|r| limited_text(r, "recipe", 5000)).transpose()?;
    let photo = b.photo.map(|p| nullable_text(p.as_deref(), "photo", 300)).transpose()?;
    let out = st
        .db
        .call(move |c| {
            let cur = get_menu_item(c, id)?;
            c.execute(
                "UPDATE menu_items SET name = ?1, category = ?2, description = ?3, recipe = ?4, photo = ?5,
                    available = ?6, sort = ?7 WHERE id = ?8",
                params![
                    name.unwrap_or(cur.name),
                    category.unwrap_or(cur.category),
                    description.unwrap_or(cur.description),
                    recipe.unwrap_or(cur.recipe),
                    photo.unwrap_or(cur.photo),
                    b.available.unwrap_or(cur.available),
                    b.sort.unwrap_or(cur.sort),
                    id
                ],
            )?;
            get_menu_item(c, id)
        })
        .await?;
    events::publish_reload(&st, "menu");
    Ok(Json(out))
}

/// `DELETE /api/menu/:id` (crew) — pedidos antigos mantêm nome/foto copiados.
pub async fn delete(
    State(st): State<AppState>,
    _crew: Crew,
    PathParam(id): PathParam<i64>,
) -> ApiResult<StatusCode> {
    st.db
        .call(move |c| {
            if c.execute("DELETE FROM menu_items WHERE id = ?1", [id])? == 0 {
                return Err(ApiError::not_found("Item do cardápio não encontrado"));
            }
            Ok(())
        })
        .await?;
    events::publish_reload(&st, "menu");
    Ok(StatusCode::NO_CONTENT)
}

// ---------- pedidos ----------

#[derive(Serialize, Clone)]
pub struct OrderOut {
    pub id: i64,
    pub menu_item_id: i64,
    pub item_name: String,
    pub item_photo: Option<String>,
    pub category: String,
    pub recipe: String,
    pub for_name: String,
    pub notes: String,
    pub status: String,
    pub handled_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip)]
    pub device_id: Option<String>,
}

const ORDER_SELECT: &str = "SELECT o.id, o.menu_item_id, o.item_name, o.item_photo, o.category,
        COALESCE(m.recipe, ''), o.for_name, o.notes, o.status, o.handled_by, o.created_at, o.updated_at,
        o.device_id
    FROM orders o LEFT JOIN menu_items m ON m.id = o.menu_item_id";

fn order_row(r: &rusqlite::Row) -> rusqlite::Result<OrderOut> {
    Ok(OrderOut {
        id: r.get(0)?,
        menu_item_id: r.get(1)?,
        item_name: r.get(2)?,
        item_photo: r.get(3)?,
        category: r.get(4)?,
        recipe: r.get(5)?,
        for_name: r.get(6)?,
        notes: r.get(7)?,
        status: r.get(8)?,
        handled_by: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
        device_id: r.get(12)?,
    })
}

fn get_order(c: &Connection, id: i64) -> ApiResult<OrderOut> {
    c.query_row(&format!("{ORDER_SELECT} WHERE o.id = ?1"), [id], order_row)
        .optional()?
        .ok_or_else(|| ApiError::not_found("Pedido não encontrado"))
}

fn push_body(o: &OrderOut) -> String {
    if o.for_name.is_empty() {
        o.item_name.clone()
    } else {
        format!("{} para {}", o.item_name, o.for_name)
    }
}

/// `GET /api/orders`
pub async fn list_orders(
    State(st): State<AppState>,
    OptCrew(crew): OptCrew,
    Device(device): Device,
) -> ApiResult<Json<Vec<OrderOut>>> {
    let since = ts(Utc::now() - Duration::hours(RECENT_HOURS));
    let v = st
        .db
        .call(move |c| {
            let v = if crew.is_some() {
                let mut s = c.prepare_cached(&format!(
                    "{ORDER_SELECT}
                     WHERE o.status NOT IN ('entregue', 'cancelado') OR o.updated_at >= ?1
                     ORDER BY o.id DESC"
                ))?;
                s.query_map([&since], order_row)?.collect::<Result<Vec<_>, _>>()?
            } else if let Some(device) = device {
                let mut s = c.prepare_cached(&format!(
                    "{ORDER_SELECT} WHERE o.device_id = ?1 AND o.created_at >= ?2 ORDER BY o.id DESC"
                ))?;
                s.query_map(params![device, since], order_row)?.collect::<Result<Vec<_>, _>>()?
            } else {
                Vec::new()
            };
            Ok(v)
        })
        .await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct OrderIn {
    menu_item_id: i64,
    #[serde(default)]
    for_name: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

/// `POST /api/orders`
pub async fn create_order(
    State(st): State<AppState>,
    Device(device): Device,
    JsonBody(b): JsonBody<OrderIn>,
) -> ApiResult<(StatusCode, Json<OrderOut>)> {
    let for_name = required_text(b.for_name.as_deref().unwrap_or(""), "nome de quem recebe", 60)?;
    let device = device.ok_or_else(|| ApiError::bad_request("Identificação do aparelho ausente"))?;
    let notes = limited_text(b.notes.as_deref().unwrap_or(""), "notes", 500)?;
    let order = st
        .db
        .call(move |c| {
            let item = get_menu_item(c, b.menu_item_id)?;
            if !item.available {
                return Err(ApiError::bad_request("Item indisponível no momento"));
            }
            let now = now_str();
            c.execute(
                "INSERT INTO orders(menu_item_id, item_name, item_photo, category, for_name, notes, status,
                    device_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'recebido', ?7, ?8, ?8)",
                params![item.id, item.name, item.photo, item.category, for_name, notes, device, now],
            )?;
            get_order(c, c.last_insert_rowid())
        })
        .await?;
    events::publish_reload(&st, "order_new");
    push::notify(
        &st,
        Target::Crew,
        PushMsg { title: "Novo pedido 🍹".into(), body: push_body(&order), url: "/pedidos".into() },
    );
    tracing::info!("pedido {} criado: {}", order.id, order.item_name);
    Ok((StatusCode::CREATED, Json(order)))
}

#[derive(Deserialize)]
pub struct OrderPatch {
    status: String,
}

/// `PATCH /api/orders/:id` — tripulação muda o status; o dono pode cancelar enquanto `recebido`.
pub async fn update_order(
    State(st): State<AppState>,
    OptCrew(crew): OptCrew,
    Device(device): Device,
    PathParam(id): PathParam<i64>,
    JsonBody(b): JsonBody<OrderPatch>,
) -> ApiResult<Json<OrderOut>> {
    let status = b.status.trim().to_lowercase();
    if !STATUSES.contains(&status.as_str()) {
        return Err(ApiError::bad_request(
            "Status inválido (use recebido, preparando, pronto, entregue ou cancelado)",
        ));
    }
    let (order, changed) = st
        .db
        .call(move |c| {
            let cur = get_order(c, id)?;
            let handled_by = match &crew {
                Some(crew) => Some(crew.name.clone()),
                None => {
                    let owner = device.is_some() && cur.device_id == device;
                    if !owner || status != "cancelado" {
                        return Err(ApiError::forbidden());
                    }
                    if cur.status != "recebido" {
                        return Err(ApiError::conflict("O pedido já está em preparo e não pode mais ser cancelado"));
                    }
                    cur.handled_by.clone()
                }
            };
            let changed = cur.status != status;
            if changed && !matches!((cur.status.as_str(), status.as_str()),
                ("recebido", "preparando" | "cancelado") |
                ("preparando", "pronto" | "cancelado") |
                ("pronto", "entregue" | "cancelado")) {
                return Err(ApiError::conflict("O pedido já mudou de etapa. Confira o estado atual."));
            }
            c.execute(
                "UPDATE orders SET status = ?1, handled_by = ?2, updated_at = ?3 WHERE id = ?4",
                params![status, handled_by, now_str(), id],
            )?;
            Ok((get_order(c, id)?, changed))
        })
        .await?;
    events::publish_reload(&st, "order_update");
    if changed
        && order.status == "pronto"
        && let Some(dev) = order.device_id.clone()
    {
        push::notify(
            &st,
            Target::Device(dev),
            PushMsg { title: "Seu pedido está pronto! 🪽".into(), body: push_body(&order), url: "/bar".into() },
        );
    }
    Ok(Json(order))
}
