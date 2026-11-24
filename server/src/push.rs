use std::path::Path;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use web_push::{ContentEncoding, SubscriptionInfo, Urgency, VapidSignatureBuilder, WebPushMessageBuilder};

use crate::error::{ApiError, ApiResult};
use crate::extract::{Device, JsonBody, OptCrew};
use crate::state::AppState;
use crate::util::now_str;

const VAPID_SUB: &str = "mailto:contato@angelone.app";
const PUSH_TTL_S: u32 = 3600;

/// Par VAPID P-256 (base64url sem padding). Pública: 65 bytes não comprimidos.
#[derive(Clone, Serialize, Deserialize)]
pub struct Vapid {
    pub private_b64: String,
    pub public_b64: String,
}

/// Lê `DATA_DIR/vapid.json` ou gera um par novo no primeiro start.
pub fn load_or_create(data_dir: &Path) -> std::io::Result<Vapid> {
    let path = data_dir.join("vapid.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        match serde_json::from_str::<Vapid>(&text) {
            Ok(v) if VapidSignatureBuilder::from_base64_no_sub(&v.private_b64).is_ok() => return Ok(v),
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("{} inválido; apague-o para gerar outro par (inscrições atuais deixarão de valer)", path.display()),
                ));
            }
        }
    }
    let kp = jwt_simple::algorithms::ES256KeyPair::generate();
    let private_b64 = URL_SAFE_NO_PAD.encode(kp.to_bytes());
    let public = VapidSignatureBuilder::from_base64_no_sub(&private_b64)
        .map_err(|e| std::io::Error::other(format!("VAPID: {e:?}")))?
        .get_public_key();
    let v = Vapid { private_b64, public_b64: URL_SAFE_NO_PAD.encode(public) };
    let tmp = data_dir.join(".vapid.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&v)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, &path)?;
    tracing::info!("par VAPID gerado em {}", path.display());
    Ok(v)
}

// ---------- inscrição ----------

#[derive(Deserialize)]
pub struct Keys {
    p256dh: String,
    auth: String,
}

#[derive(Deserialize)]
pub struct Subscription {
    endpoint: String,
    keys: Keys,
}

#[derive(Deserialize)]
pub struct SubscribeIn {
    subscription: Subscription,
}

fn b64_len(s: &str) -> Option<usize> {
    URL_SAFE_NO_PAD.decode(s.trim().trim_end_matches('=')).ok().map(|v| v.len())
}

fn valid_endpoint(endpoint: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(endpoint) else { return false };
    endpoint.len() <= 2048 && url.scheme() == "https" && url.port().is_none()
        && url.username().is_empty() && url.password().is_none() && url.fragment().is_none()
        && url.host_str().is_some_and(|h| matches!(h, "web.push.apple.com" | "fcm.googleapis.com" | "updates.push.services.mozilla.com")
            || h.ends_with(".push.apple.com") || h.ends_with(".notify.windows.com"))
}

/// `POST /api/push/subscribe` → 204 (reenvio atualiza)
pub async fn subscribe(
    State(st): State<AppState>,
    OptCrew(crew): OptCrew,
    Device(device): Device,
    JsonBody(b): JsonBody<SubscribeIn>,
) -> ApiResult<StatusCode> {
    let s = b.subscription;
    let endpoint = s.endpoint.trim().to_string();
    if !valid_endpoint(&endpoint) {
        return Err(ApiError::bad_request("Endpoint de push inválido"));
    }
    if b64_len(&s.keys.p256dh) != Some(65) || b64_len(&s.keys.auth) != Some(16) {
        return Err(ApiError::bad_request("Chaves de push inválidas"));
    }
    let p256dh = s.keys.p256dh.trim().trim_end_matches('=').to_string();
    let auth = s.keys.auth.trim().trim_end_matches('=').to_string();
    let crew_id = crew.map(|c| c.id);
    st.db
        .call(move |c| {
            let now = now_str();
            c.execute(
                "INSERT INTO push_subs(endpoint, p256dh, auth, device_id, crew_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
                 ON CONFLICT(endpoint) DO UPDATE SET p256dh = excluded.p256dh, auth = excluded.auth,
                    device_id = excluded.device_id, crew_id = excluded.crew_id, updated_at = excluded.updated_at",
                params![endpoint, p256dh, auth, device, crew_id, now],
            )?;
            Ok(())
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct UnsubscribeIn {
    endpoint: String,
}

/// `POST /api/push/unsubscribe` → 204
pub async fn unsubscribe(State(st): State<AppState>, JsonBody(b): JsonBody<UnsubscribeIn>) -> ApiResult<StatusCode> {
    let endpoint = b.endpoint.trim().to_string();
    st.db
        .call(move |c| {
            c.execute("DELETE FROM push_subs WHERE endpoint = ?1", [endpoint])?;
            Ok(())
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------- envio ----------

#[derive(Serialize)]
pub struct PushMsg {
    pub title: String,
    pub body: String,
    pub url: String,
}

pub enum Target {
    /// Todas as inscrições ligadas a um tripulante existente.
    Crew,
    /// Inscrições de um aparelho.
    Device(String),
}

struct Sub {
    endpoint: String,
    p256dh: String,
    auth: String,
}

enum SendError {
    Gone(u16),
    Other(String),
}

/// Dispara o push em segundo plano (não atrasa a resposta HTTP).
pub fn notify(st: &AppState, target: Target, msg: PushMsg) {
    let st = st.clone();
    tokio::spawn(async move {
        if let Err(e) = send_all(&st, target, msg).await {
            tracing::warn!("falha ao enviar push: {e}");
        }
    });
}

async fn send_all(st: &AppState, target: Target, msg: PushMsg) -> ApiResult<()> {
    let subs = st
        .db
        .call(move |c| {
            let map = |r: &rusqlite::Row| Ok(Sub { endpoint: r.get(0)?, p256dh: r.get(1)?, auth: r.get(2)? });
            let v = match target {
                Target::Crew => {
                    let mut s = c.prepare_cached(
                        "SELECT p.endpoint, p.p256dh, p.auth FROM push_subs p
                         JOIN crew c ON c.id = p.crew_id",
                    )?;
                    s.query_map([], map)?.collect::<Result<Vec<_>, _>>()?
                }
                Target::Device(d) => {
                    let mut s =
                        c.prepare_cached("SELECT endpoint, p256dh, auth FROM push_subs WHERE device_id = ?1")?;
                    s.query_map([d], map)?.collect::<Result<Vec<_>, _>>()?
                }
            };
            Ok(v)
        })
        .await?;
    if subs.is_empty() {
        return Ok(());
    }
    let payload = serde_json::to_vec(&msg).unwrap_or_default();
    let mut gone = Vec::new();
    let mut ok = 0usize;
    for sub in &subs {
        match send_one(st, sub, &payload).await {
            Ok(()) => ok += 1,
            Err(SendError::Gone(code)) => {
                tracing::info!("inscrição de push expirada ({code}) em {}; removendo", host(&sub.endpoint));
                gone.push(sub.endpoint.clone());
            }
            Err(SendError::Other(e)) => tracing::warn!("push para {} falhou: {e}", host(&sub.endpoint)),
        }
    }
    tracing::info!("push '{}' enviado para {ok}/{} inscrições", msg.title, subs.len());
    if !gone.is_empty() {
        st.db
            .call(move |c| {
                for e in gone {
                    c.execute("DELETE FROM push_subs WHERE endpoint = ?1", [e])?;
                }
                Ok(())
            })
            .await?;
    }
    Ok(())
}

fn host(endpoint: &str) -> String {
    endpoint
        .parse::<axum::http::Uri>()
        .ok()
        .and_then(|u| u.host().map(str::to_string))
        .unwrap_or_else(|| "?".into())
}

/// Monta a mensagem criptografada (aes128gcm) com JWT VAPID (`aud` = origem do endpoint,
/// `sub` = mailto, `exp` = +1 h), `TTL` e `Urgency: high`.
fn build_message(vapid: &Vapid, sub: &Sub, payload: &[u8]) -> Result<web_push::WebPushMessage, String> {
    let info = SubscriptionInfo::new(sub.endpoint.as_str(), sub.p256dh.as_str(), sub.auth.as_str());
    let mut sig = VapidSignatureBuilder::from_base64(&vapid.private_b64, &info).map_err(|e| format!("VAPID: {e:?}"))?;
    sig.add_claim("sub", VAPID_SUB);
    // Expiração curta (1 h): aceita por FCM, Mozilla e Apple.
    let exp = chrono::Utc::now().timestamp() as u64 + 3600;
    sig.add_claim("exp", exp);
    let sig = sig.build().map_err(|e| format!("VAPID: {e:?}"))?;
    let mut b = WebPushMessageBuilder::new(&info);
    b.set_payload(ContentEncoding::Aes128Gcm, payload);
    b.set_vapid_signature(sig);
    b.set_ttl(PUSH_TTL_S);
    b.set_urgency(Urgency::High);
    b.build().map_err(|e| format!("criptografia: {e:?}"))
}

async fn send_one(st: &AppState, sub: &Sub, payload: &[u8]) -> Result<(), SendError> {
    if !valid_endpoint(&sub.endpoint) {
        return Err(SendError::Gone(410));
    }
    let message = build_message(&st.vapid, sub, payload).map_err(SendError::Other)?;

    let mut req = st
        .http
        .post(message.endpoint.to_string())
        .timeout(Duration::from_secs(20))
        .header("TTL", message.ttl.to_string());
    if let Some(u) = message.urgency {
        req = req.header("Urgency", u.to_string());
    }
    if let Some(t) = message.topic {
        req = req.header("Topic", t);
    }
    if let Some(p) = message.payload {
        req = req
            .header("Content-Encoding", p.content_encoding.to_str())
            .header("Content-Type", "application/octet-stream");
        for (k, v) in p.crypto_headers {
            req = req.header(k, v);
        }
        req = req.body(p.content);
    }
    let res = req.send().await.map_err(|e| SendError::Other(e.to_string()))?;
    let status = res.status();
    if status.is_success() {
        return Ok(());
    }
    let code = status.as_u16();
    if code == 404 || code == 410 {
        return Err(SendError::Gone(code));
    }
    let body = res.text().await.unwrap_or_default();
    let body: String = body.chars().take(300).collect();
    Err(SendError::Other(format!("HTTP {code}: {body}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_message_has_vapid_jwt_and_headers() {
        let dir = std::env::temp_dir().join(format!("angel-one-test-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let vapid = load_or_create(&dir).unwrap();
        // Reabre do arquivo: mesmo par.
        let again = load_or_create(&dir).unwrap();
        assert_eq!(vapid.public_b64, again.public_b64);
        let public = URL_SAFE_NO_PAD.decode(&vapid.public_b64).unwrap();
        assert_eq!(public.len(), 65);
        assert_eq!(public[0], 4);

        // Chaves de inscrição de um cliente de teste (P-256 + auth de 16 bytes).
        let client = jwt_simple::algorithms::ES256KeyPair::generate();
        let client_pub = VapidSignatureBuilder::from_base64_no_sub(&URL_SAFE_NO_PAD.encode(client.to_bytes()))
            .unwrap()
            .get_public_key();
        let sub = Sub {
            endpoint: "https://web.push.apple.com/QGuQyavXutnMH5fk8cE3/abc".into(),
            p256dh: URL_SAFE_NO_PAD.encode(client_pub),
            auth: URL_SAFE_NO_PAD.encode([7u8; 16]),
        };
        let m = build_message(&vapid, &sub, br#"{"title":"t","body":"b","url":"/"}"#).unwrap();
        assert_eq!(m.ttl, PUSH_TTL_S);
        assert_eq!(m.urgency.map(|u| u.to_string()).as_deref(), Some("high"));
        let p = m.payload.unwrap();
        assert_eq!(p.content_encoding.to_str(), "aes128gcm");
        let auth = p.crypto_headers.iter().find(|(k, _)| *k == "Authorization").unwrap().1.clone();
        let rest = auth.strip_prefix("vapid t=").unwrap();
        let (jwt, k) = rest.split_once(", k=").unwrap();
        assert_eq!(k, vapid.public_b64);
        let claims: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(jwt.split('.').nth(1).unwrap()).unwrap()).unwrap();
        assert_eq!(claims["aud"], "https://web.push.apple.com");
        assert_eq!(claims["sub"], VAPID_SUB);
        let exp = claims["exp"].as_i64().unwrap();
        let now = chrono::Utc::now().timestamp();
        assert!(exp > now && exp <= now + 3600 + 5, "exp = {exp}, agora = {now}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
