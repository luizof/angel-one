use std::collections::HashMap;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ApiResult};
use crate::events;
use crate::extract::{Crew, JsonBody, PathParam};
use crate::state::AppState;
use crate::util::{check_non_negative, double_option, limited_text, now_str, nullable_text, required_text, round_to};

// ---------- unidades ----------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Volume,
    Mass,
    Count,
}

/// Grandeza e fator para a unidade base (ml, g, un).
fn unit_info(u: &str) -> Option<(Kind, f64)> {
    match u {
        "ml" => Some((Kind::Volume, 1.0)),
        "L" => Some((Kind::Volume, 1000.0)),
        "g" => Some((Kind::Mass, 1.0)),
        "kg" => Some((Kind::Mass, 1000.0)),
        "un" => Some((Kind::Count, 1.0)),
        _ => None,
    }
}

/// Normaliza grafias comuns (`l`, `ML`, `Kg`...) para `ml|L|g|kg|un`.
fn norm_unit(u: &str) -> Option<&'static str> {
    match u.trim().to_lowercase().as_str() {
        "ml" => Some("ml"),
        "l" | "lt" | "litro" | "litros" => Some("L"),
        "g" | "gr" => Some("g"),
        "kg" => Some("kg"),
        "un" | "und" | "unid" | "unidade" | "unidades" => Some("un"),
        _ => None,
    }
}

fn parse_unit(u: &str, field: &str) -> ApiResult<&'static str> {
    norm_unit(u).ok_or_else(|| ApiError::bad_request(format!("Unidade inválida em '{field}' (use ml, L, g, kg ou un)")))
}

// ---------- saída ----------

#[derive(Serialize, Clone, Debug)]
pub struct ItemOut {
    pub id: i64,
    pub name: String,
    pub photo: Option<String>,
    pub photo_original: Option<String>,
    pub tag_ids: Vec<i64>,
    pub count_unit: String,
    pub content_amount: Option<f64>,
    pub content_unit: Option<String>,
    pub quantity: f64,
    pub min_qty: Option<f64>,
    pub fractional: bool,
    pub below_min: bool,
    pub updated_at: String,
    pub updated_by: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct TagOut {
    pub id: i64,
    pub name: String,
    pub emoji: Option<String>,
    pub unit: String,
    pub min_qty: Option<f64>,
    pub total: f64,
    pub below_min: bool,
    pub item_ids: Vec<i64>,
}

#[derive(Serialize)]
pub struct Inventory {
    pub items: Vec<ItemOut>,
    pub tags: Vec<TagOut>,
}

const ITEM_COLS: &str = "id, name, photo, photo_original, count_unit, content_amount, content_unit,
    quantity, min_qty, fractional, updated_at, updated_by";

fn item_row(r: &rusqlite::Row) -> rusqlite::Result<ItemOut> {
    let quantity: f64 = r.get(7)?;
    let min_qty: Option<f64> = r.get(8)?;
    Ok(ItemOut {
        id: r.get(0)?,
        name: r.get(1)?,
        photo: r.get(2)?,
        photo_original: r.get(3)?,
        tag_ids: Vec::new(),
        count_unit: r.get(4)?,
        content_amount: r.get(5)?,
        content_unit: r.get(6)?,
        quantity,
        min_qty,
        fractional: r.get(9)?,
        below_min: min_qty.is_some_and(|m| quantity < m),
        updated_at: r.get(10)?,
        updated_by: r.get(11)?,
    })
}

/// Total da tag na unidade da tag. `un` soma `quantity`; volume↔volume e massa↔massa
/// convertem `quantity × content_amount`; itens incompatíveis ficam de fora.
fn tag_total(unit: &str, items: &[&ItemOut]) -> f64 {
    let Some((kind, factor)) = unit_info(unit) else { return 0.0 };
    let mut total = 0.0;
    for it in items {
        if kind == Kind::Count {
            total += it.quantity;
            continue;
        }
        let (Some(amount), Some(cu)) = (it.content_amount, it.content_unit.as_deref()) else { continue };
        match unit_info(cu) {
            Some((k, f)) if k == kind => total += it.quantity * amount * f / factor,
            _ => {}
        }
    }
    round_to(total, 4)
}

pub fn load_inventory(c: &Connection) -> rusqlite::Result<Inventory> {
    let mut links: HashMap<i64, Vec<i64>> = HashMap::new(); // item -> tags
    let mut tag_items: HashMap<i64, Vec<i64>> = HashMap::new(); // tag -> items
    {
        let mut s = c.prepare_cached("SELECT item_id, tag_id FROM item_tags ORDER BY item_id, tag_id")?;
        let rows = s.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
        for row in rows {
            let (i, t) = row?;
            links.entry(i).or_default().push(t);
            tag_items.entry(t).or_default().push(i);
        }
    }
    let mut items = {
        let mut s = c.prepare_cached(&format!("SELECT {ITEM_COLS} FROM items ORDER BY name COLLATE NOCASE, id"))?;
        s.query_map([], item_row)?.collect::<Result<Vec<_>, _>>()?
    };
    for it in &mut items {
        it.tag_ids = links.remove(&it.id).unwrap_or_default();
    }
    let by_id: HashMap<i64, &ItemOut> = items.iter().map(|i| (i.id, i)).collect();
    let tags = {
        let mut s =
            c.prepare_cached("SELECT id, name, emoji, unit, min_qty FROM tags ORDER BY name COLLATE NOCASE, id")?;
        let rows = s.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<f64>>(4)?,
            ))
        })?;
        let mut v = Vec::new();
        for row in rows {
            let (id, name, emoji, unit, min_qty) = row?;
            let item_ids = tag_items.remove(&id).unwrap_or_default();
            let members: Vec<&ItemOut> = item_ids.iter().filter_map(|i| by_id.get(i).copied()).collect();
            let total = tag_total(&unit, &members);
            v.push(TagOut {
                id,
                name,
                emoji,
                unit,
                min_qty,
                total,
                below_min: min_qty.is_some_and(|m| total < m),
                item_ids,
            });
        }
        v
    };
    Ok(Inventory { items, tags })
}

fn load_item(c: &Connection, id: i64) -> ApiResult<ItemOut> {
    let mut it = c
        .query_row(&format!("SELECT {ITEM_COLS} FROM items WHERE id = ?1"), [id], item_row)
        .optional()?
        .ok_or_else(|| ApiError::not_found("Item não encontrado"))?;
    let mut s = c.prepare_cached("SELECT tag_id FROM item_tags WHERE item_id = ?1 ORDER BY tag_id")?;
    it.tag_ids = s.query_map([id], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
    Ok(it)
}

fn load_tag(c: &Connection, id: i64) -> ApiResult<TagOut> {
    load_inventory(c)?
        .tags
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| ApiError::not_found("Tag não encontrada"))
}

fn set_tags(c: &Connection, item_id: i64, tag_ids: &[i64]) -> ApiResult<()> {
    for t in tag_ids {
        let ok: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM tags WHERE id = ?1)", [t], |r| r.get(0))?;
        if !ok {
            return Err(ApiError::bad_request(format!("Tag {t} não existe")));
        }
    }
    c.execute("DELETE FROM item_tags WHERE item_id = ?1", [item_id])?;
    for t in tag_ids {
        c.execute("INSERT OR IGNORE INTO item_tags(item_id, tag_id) VALUES (?1, ?2)", params![item_id, t])?;
    }
    Ok(())
}

fn movement(c: &Connection, item_id: i64, by: &str, reason: &str, before: f64, after: f64) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO movements(item_id, at, by_name, reason, before, after) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![item_id, now_str(), by, reason, before, after],
    )?;
    Ok(())
}

fn clean_qty(v: f64) -> f64 {
    round_to(v.max(0.0), 4)
}

// ---------- rotas ----------

/// `GET /api/inventory`
pub async fn get(State(st): State<AppState>) -> ApiResult<Json<Inventory>> {
    Ok(Json(st.db.call(|c| Ok(load_inventory(c)?)).await?))
}

#[derive(Deserialize)]
pub struct ItemIn {
    name: String,
    photo: Option<String>,
    photo_original: Option<String>,
    #[serde(default)]
    tag_ids: Vec<i64>,
    count_unit: Option<String>,
    content_amount: Option<f64>,
    content_unit: Option<String>,
    #[serde(default)]
    quantity: f64,
    min_qty: Option<f64>,
    #[serde(default)]
    fractional: bool,
}

struct ItemFields {
    name: String,
    photo: Option<String>,
    photo_original: Option<String>,
    count_unit: String,
    content_amount: Option<f64>,
    content_unit: Option<String>,
    quantity: f64,
    min_qty: Option<f64>,
    fractional: bool,
}

fn validate_item(f: ItemFields) -> ApiResult<ItemFields> {
    let count_unit = limited_text(&f.count_unit, "count_unit", 30)?;
    Ok(ItemFields {
        name: required_text(&f.name, "name", 80)?,
        photo: nullable_text(f.photo.as_deref(), "photo", 300)?,
        photo_original: nullable_text(f.photo_original.as_deref(), "photo_original", 300)?,
        count_unit: if count_unit.is_empty() { "un".into() } else { count_unit },
        content_amount: f.content_amount.map(|a| check_non_negative(a, "content_amount")).transpose()?,
        content_unit: match f.content_unit.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(u) => Some(parse_unit(u, "content_unit")?.to_string()),
            None => None,
        },
        quantity: clean_qty(check_non_negative(f.quantity, "quantity")?),
        min_qty: f.min_qty.map(|m| check_non_negative(m, "min_qty")).transpose()?,
        fractional: f.fractional,
    })
}

fn write_item(c: &Connection, id: Option<i64>, f: &ItemFields, by: &str) -> rusqlite::Result<i64> {
    let now = now_str();
    match id {
        None => {
            c.execute(
                "INSERT INTO items(name, photo, photo_original, count_unit, content_amount, content_unit,
                    quantity, min_qty, fractional, created_at, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10, ?11)",
                params![
                    f.name, f.photo, f.photo_original, f.count_unit, f.content_amount, f.content_unit,
                    f.quantity, f.min_qty, f.fractional, now, by
                ],
            )?;
            Ok(c.last_insert_rowid())
        }
        Some(id) => {
            c.execute(
                "UPDATE items SET name = ?1, photo = ?2, photo_original = ?3, count_unit = ?4,
                    content_amount = ?5, content_unit = ?6, quantity = ?7, min_qty = ?8, fractional = ?9,
                    updated_at = ?10, updated_by = ?11
                 WHERE id = ?12",
                params![
                    f.name, f.photo, f.photo_original, f.count_unit, f.content_amount, f.content_unit,
                    f.quantity, f.min_qty, f.fractional, now, by, id
                ],
            )?;
            Ok(id)
        }
    }
}

/// `POST /api/items` (crew)
pub async fn create_item(
    State(st): State<AppState>,
    crew: Crew,
    JsonBody(b): JsonBody<ItemIn>,
) -> ApiResult<(StatusCode, Json<ItemOut>)> {
    let f = validate_item(ItemFields {
        name: b.name,
        photo: b.photo,
        photo_original: b.photo_original,
        count_unit: b.count_unit.unwrap_or_default(),
        content_amount: b.content_amount,
        content_unit: b.content_unit,
        quantity: b.quantity,
        min_qty: b.min_qty,
        fractional: b.fractional,
    })?;
    let tag_ids = b.tag_ids;
    let out = st
        .db
        .call(move |c| {
            let tx = c.transaction()?;
            let id = write_item(&tx, None, &f, &crew.name)?;
            set_tags(&tx, id, &tag_ids)?;
            if f.quantity > 0.0 {
                movement(&tx, id, &crew.name, "ajuste", 0.0, f.quantity)?;
            }
            tx.commit()?;
            load_item(c, id)
        })
        .await?;
    events::publish_reload(&st, "inventory");
    Ok((StatusCode::CREATED, Json(out)))
}

#[derive(Deserialize)]
pub struct ItemPatch {
    name: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    photo: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    photo_original: Option<Option<String>>,
    tag_ids: Option<Vec<i64>>,
    count_unit: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    content_amount: Option<Option<f64>>,
    #[serde(default, deserialize_with = "double_option")]
    content_unit: Option<Option<String>>,
    quantity: Option<f64>,
    #[serde(default, deserialize_with = "double_option")]
    min_qty: Option<Option<f64>>,
    fractional: Option<bool>,
}

/// `PATCH /api/items/:id` (crew)
pub async fn update_item(
    State(st): State<AppState>,
    crew: Crew,
    PathParam(id): PathParam<i64>,
    JsonBody(b): JsonBody<ItemPatch>,
) -> ApiResult<Json<ItemOut>> {
    let out = st
        .db
        .call(move |c| {
            let cur = load_item(c, id)?;
            let f = validate_item(ItemFields {
                name: b.name.unwrap_or(cur.name),
                photo: b.photo.unwrap_or(cur.photo),
                photo_original: b.photo_original.unwrap_or(cur.photo_original),
                count_unit: b.count_unit.unwrap_or(cur.count_unit),
                content_amount: b.content_amount.unwrap_or(cur.content_amount),
                content_unit: b.content_unit.unwrap_or(cur.content_unit),
                quantity: b.quantity.unwrap_or(cur.quantity),
                min_qty: b.min_qty.unwrap_or(cur.min_qty),
                fractional: b.fractional.unwrap_or(cur.fractional),
            })?;
            let tx = c.transaction()?;
            write_item(&tx, Some(id), &f, &crew.name)?;
            if let Some(tags) = &b.tag_ids {
                set_tags(&tx, id, tags)?;
            }
            if (f.quantity - cur.quantity).abs() > 1e-9 {
                movement(&tx, id, &crew.name, "ajuste", cur.quantity, f.quantity)?;
            }
            tx.commit()?;
            load_item(c, id)
        })
        .await?;
    events::publish_reload(&st, "inventory");
    Ok(Json(out))
}

/// `DELETE /api/items/:id` (crew)
pub async fn delete_item(
    State(st): State<AppState>,
    _crew: Crew,
    PathParam(id): PathParam<i64>,
) -> ApiResult<StatusCode> {
    st.db
        .call(move |c| {
            if c.execute("DELETE FROM items WHERE id = ?1", [id])? == 0 {
                return Err(ApiError::not_found("Item não encontrado"));
            }
            Ok(())
        })
        .await?;
    events::publish_reload(&st, "inventory");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct AdjustIn {
    quantity: Option<f64>,
    delta: Option<f64>,
    reason: Option<String>,
}

/// `POST /api/items/:id/adjust` (crew) — `quantity` absoluta ou `delta`; grava movimento.
pub async fn adjust(
    State(st): State<AppState>,
    crew: Crew,
    PathParam(id): PathParam<i64>,
    JsonBody(b): JsonBody<AdjustIn>,
) -> ApiResult<Json<ItemOut>> {
    let reason = b.reason.as_deref().map(str::trim).filter(|r| !r.is_empty()).unwrap_or("ajuste").to_string();
    if !matches!(reason.as_str(), "conferencia" | "compra" | "ajuste") {
        return Err(ApiError::bad_request("Motivo inválido (use conferencia, compra ou ajuste)"));
    }
    let change = match (b.quantity, b.delta) {
        (Some(q), None) => Ok(check_non_negative(q, "quantity")?),
        (None, Some(d)) if d.is_finite() => Err(d),
        (None, Some(_)) => return Err(ApiError::bad_request("Campo 'delta' inválido")),
        _ => return Err(ApiError::bad_request("Informe 'quantity' ou 'delta' (apenas um)")),
    };
    let out = st
        .db
        .call(move |c| {
            let cur = load_item(c, id)?;
            let after = clean_qty(match change {
                Ok(q) => q,
                Err(d) => cur.quantity + d,
            });
            let tx = c.transaction()?;
            tx.execute(
                "UPDATE items SET quantity = ?1, updated_at = ?2, updated_by = ?3 WHERE id = ?4",
                params![after, now_str(), crew.name, id],
            )?;
            movement(&tx, id, &crew.name, &reason, cur.quantity, after)?;
            tx.commit()?;
            load_item(c, id)
        })
        .await?;
    events::publish_reload(&st, "inventory");
    Ok(Json(out))
}

#[derive(Serialize)]
pub struct MovementOut {
    at: String,
    by: Option<String>,
    reason: String,
    before: f64,
    after: f64,
}

/// `GET /api/items/:id/history` — últimos 50 movimentos (mais recente primeiro).
pub async fn history(State(st): State<AppState>, PathParam(id): PathParam<i64>) -> ApiResult<Json<Vec<MovementOut>>> {
    let v = st
        .db
        .call(move |c| {
            load_item(c, id)?;
            let mut s = c.prepare_cached(
                "SELECT at, by_name, reason, before, after FROM movements WHERE item_id = ?1 ORDER BY id DESC LIMIT 50",
            )?;
            let v = s
                .query_map([id], |r| {
                    Ok(MovementOut { at: r.get(0)?, by: r.get(1)?, reason: r.get(2)?, before: r.get(3)?, after: r.get(4)? })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(v)
        })
        .await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct TagIn {
    name: String,
    emoji: Option<String>,
    unit: String,
    min_qty: Option<f64>,
}

/// `POST /api/tags` (crew)
pub async fn create_tag(
    State(st): State<AppState>,
    _crew: Crew,
    JsonBody(b): JsonBody<TagIn>,
) -> ApiResult<(StatusCode, Json<TagOut>)> {
    let name = required_text(&b.name, "name", 60)?;
    let emoji = nullable_text(b.emoji.as_deref(), "emoji", 16)?;
    let unit = parse_unit(&b.unit, "unit")?;
    let min_qty = b.min_qty.map(|m| check_non_negative(m, "min_qty")).transpose()?;
    let out = st
        .db
        .call(move |c| {
            c.execute(
                "INSERT INTO tags(name, emoji, unit, min_qty) VALUES (?1, ?2, ?3, ?4)",
                params![name, emoji, unit, min_qty],
            )?;
            load_tag(c, c.last_insert_rowid())
        })
        .await?;
    events::publish_reload(&st, "inventory");
    Ok((StatusCode::CREATED, Json(out)))
}

#[derive(Deserialize)]
pub struct TagPatch {
    name: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    emoji: Option<Option<String>>,
    unit: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    min_qty: Option<Option<f64>>,
}

/// `PATCH /api/tags/:id` (crew)
pub async fn update_tag(
    State(st): State<AppState>,
    _crew: Crew,
    PathParam(id): PathParam<i64>,
    JsonBody(b): JsonBody<TagPatch>,
) -> ApiResult<Json<TagOut>> {
    let name = b.name.as_deref().map(|n| required_text(n, "name", 60)).transpose()?;
    let emoji = b.emoji.map(|e| nullable_text(e.as_deref(), "emoji", 16)).transpose()?;
    let unit = b.unit.as_deref().map(|u| parse_unit(u, "unit")).transpose()?;
    let min_qty = b.min_qty.map(|m| m.map(|v| check_non_negative(v, "min_qty")).transpose()).transpose()?;
    let out = st
        .db
        .call(move |c| {
            let cur = load_tag(c, id)?;
            c.execute(
                "UPDATE tags SET name = ?1, emoji = ?2, unit = ?3, min_qty = ?4 WHERE id = ?5",
                params![
                    name.unwrap_or(cur.name),
                    emoji.unwrap_or(cur.emoji),
                    unit.map(str::to_string).unwrap_or(cur.unit),
                    min_qty.unwrap_or(cur.min_qty),
                    id
                ],
            )?;
            load_tag(c, id)
        })
        .await?;
    events::publish_reload(&st, "inventory");
    Ok(Json(out))
}

/// `DELETE /api/tags/:id` (crew) — remove a tag dos itens.
pub async fn delete_tag(
    State(st): State<AppState>,
    _crew: Crew,
    PathParam(id): PathParam<i64>,
) -> ApiResult<StatusCode> {
    st.db
        .call(move |c| {
            let tx = c.transaction()?;
            tx.execute("DELETE FROM item_tags WHERE tag_id = ?1", [id])?;
            if tx.execute("DELETE FROM tags WHERE id = ?1", [id])? == 0 {
                return Err(ApiError::not_found("Tag não encontrada"));
            }
            tx.commit()?;
            Ok(())
        })
        .await?;
    events::publish_reload(&st, "inventory");
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(q: f64, amount: Option<f64>, unit: Option<&str>) -> ItemOut {
        ItemOut {
            id: 1,
            name: "x".into(),
            photo: None,
            photo_original: None,
            tag_ids: vec![],
            count_unit: "garrafa".into(),
            content_amount: amount,
            content_unit: unit.map(String::from),
            quantity: q,
            min_qty: None,
            fractional: true,
            below_min: false,
            updated_at: String::new(),
            updated_by: None,
        }
    }

    #[test]
    fn totals() {
        let a = item(1.0, Some(750.0), Some("ml"));
        let b = item(0.5, Some(1.0), Some("L"));
        let c = item(3.0, Some(500.0), Some("g")); // incompatível com L
        let d = item(2.0, None, None);
        assert_eq!(tag_total("L", &[&a, &b, &c, &d]), 1.25);
        assert_eq!(tag_total("ml", &[&a, &b]), 1250.0);
        assert_eq!(tag_total("kg", &[&c]), 1.5);
        assert_eq!(tag_total("un", &[&a, &b, &c, &d]), 6.5);
    }
}
