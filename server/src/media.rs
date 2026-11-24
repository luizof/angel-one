use std::io::{BufWriter, Cursor};
use std::path::{Path, PathBuf};

use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::Json;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageEncoder, ImageFormat, ImageReader};
use serde::Serialize;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub const MAX_SIDE: u32 = 1600;
pub const MAX_UPLOAD: usize = 20 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub enum OutFmt {
    /// PNG se houver transparência real, senão JPEG.
    Auto,
    Png,
    Jpeg,
}

#[derive(Serialize, Debug)]
pub struct Saved {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

fn bad_image(e: impl std::fmt::Display) -> ApiError {
    ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, format!("Não foi possível ler a imagem: {e}"))
}

/// Decodifica (JPEG/PNG/WebP), aplica a orientação EXIF, reduz a ≤ 1600 px e grava em `media_dir`.
/// Bloqueante: chamar dentro de `spawn_blocking`.
pub fn save_image(media_dir: &Path, bytes: &[u8], fmt: OutFmt) -> ApiResult<Saved> {
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(bad_image)?;
    match reader.format() {
        Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP) => {}
        _ => {
            return Err(ApiError::new(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Formato não suportado (use JPEG, PNG ou WebP)",
            ));
        }
    }
    let mut decoder = reader.into_decoder().map_err(bad_image)?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(bad_image)?;
    img.apply_orientation(orientation);

    if img.width().max(img.height()) > MAX_SIDE {
        img = img.resize(MAX_SIDE, MAX_SIDE, FilterType::Lanczos3);
    }

    let transparent = img.color().has_alpha() && img.to_rgba8().pixels().any(|p| p.0[3] < 255);
    let png = match fmt {
        OutFmt::Png => true,
        OutFmt::Jpeg => false,
        OutFmt::Auto => transparent,
    };

    std::fs::create_dir_all(media_dir).map_err(|e| {
        tracing::error!("não foi possível criar {}: {e}", media_dir.display());
        ApiError::internal("Falha ao salvar a imagem")
    })?;
    let name = format!("{}.{}", uuid::Uuid::new_v4().simple(), if png { "png" } else { "jpg" });
    let final_path = media_dir.join(&name);
    let tmp_path = media_dir.join(format!(".{name}.tmp"));
    let (w, h) = (img.width(), img.height());

    let write = || -> Result<(), String> {
        let file = std::fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
        let mut out = BufWriter::new(file);
        if png {
            let enc = PngEncoder::new(&mut out);
            if transparent {
                let rgba = img.to_rgba8();
                enc.write_image(rgba.as_raw(), w, h, image::ExtendedColorType::Rgba8)
            } else {
                let rgb = img.to_rgb8();
                enc.write_image(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8)
            }
            .map_err(|e| e.to_string())?;
        } else {
            let rgb = img.to_rgb8();
            JpegEncoder::new_with_quality(&mut out, 85)
                .write_image(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8)
                .map_err(|e| e.to_string())?;
        }
        let file = out.into_inner().map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(&tmp_path, &final_path).map_err(|e| e.to_string())?;
        Ok(())
    };
    if let Err(e) = write() {
        let _ = std::fs::remove_file(&tmp_path);
        tracing::error!("falha ao gravar imagem {name}: {e}");
        return Err(ApiError::internal("Falha ao salvar a imagem"));
    }
    Ok(Saved { url: format!("/media/{name}"), width: w, height: h })
}

/// Converte `/media/<arquivo>` no caminho local, conferindo o nome e a existência.
pub fn media_path(media_dir: &Path, url: &str) -> ApiResult<PathBuf> {
    let name = url
        .trim()
        .strip_prefix("/media/")
        .ok_or_else(|| ApiError::bad_request("URL de mídia inválida (esperado /media/...)"))?;
    let ok = !name.is_empty()
        && !name.starts_with('.')
        && name.len() <= 200
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !ok {
        return Err(ApiError::bad_request("URL de mídia inválida"));
    }
    let p = media_dir.join(name);
    if !p.is_file() {
        return Err(ApiError::not_found("Imagem não encontrada"));
    }
    Ok(p)
}

/// `POST /api/upload` (multipart, campo `file`)
pub async fn upload(State(st): State<AppState>, mut mp: Multipart) -> ApiResult<Json<Saved>> {
    let mut data = None;
    while let Some(field) = mp.next_field().await? {
        if field.name() == Some("file") {
            data = Some(field.bytes().await?);
            break;
        }
    }
    let data = data.ok_or_else(|| ApiError::bad_request("Campo 'file' ausente"))?;
    if data.is_empty() {
        return Err(ApiError::bad_request("Arquivo vazio"));
    }
    if data.len() > MAX_UPLOAD {
        return Err(ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, "Arquivo grande demais (máx. 20 MB)"));
    }
    let dir = st.cfg.media_dir.clone();
    let saved = tokio::task::spawn_blocking(move || save_image(&dir, &data, OutFmt::Auto))
        .await
        .map_err(|_| ApiError::internal("Falha ao processar a imagem"))??;
    tracing::info!("upload salvo: {} ({}x{})", saved.url, saved.width, saved.height);
    Ok(Json(saved))
}
