use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::Engine;
use lut_core::{
    build_lut as core_build_lut,
    decode_raw as core_decode_raw,
    decode_raw_bytes as core_decode_raw_bytes,
    ImportLut, MatchStats, Params, UserCurves,
};
use std::borrow::Cow;
use tauri::http::{header::CONTENT_TYPE, Response};

mod config;

/// 解密内嵌的加密前端资源（HTML 不落盘，运行时解密）
fn decrypt_frontend() -> Vec<u8> {
    let data = include_bytes!("../resources/app_data.enc");
    let cipher = Aes256Gcm::new_from_slice(&config::APP_KEY).expect("invalid key length");
    let nonce = Nonce::from_slice(&config::APP_NONCE);
    cipher
        .decrypt(nonce, data.as_ref())
        .expect("failed to decrypt frontend resource")
}

/// 核心命令：根据参数生成 LUT（算法在 Rust 机器码中，前端 JS 不可见）
/// 返回 base64 编码的 Float32Array（N*N*N*3 个 float，little-endian）
#[tauri::command]
fn build_lut(size: usize, params: Params, match_stats: Option<MatchStats>, curves: Option<UserCurves>, import_lut: Option<ImportLut>) -> String {
    let lut: Vec<f32> = core_build_lut(size, &params, match_stats.as_ref(), curves.as_ref(), import_lut.as_ref());
    let bytes: Vec<u8> = lut.iter().flat_map(|v| v.to_le_bytes()).collect();
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}

/// 写文本文件（保存 .cube LUT）
#[tauri::command]
fn write_text(path: String, content: String) -> Result<String, String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path)
}

/// 写二进制文件（保存效果图，base64 编码的 PNG）
#[tauri::command]
fn write_b64(path: String, data: String) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

/// 返回应用版本号（标题栏显示）
#[tauri::command]
fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// RAW 解码结果（rgba 以 base64 传输）
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RawResult {
    width: usize,
    height: usize,
    data: String, // base64 编码的 sRGB RGBA
}

/// 解码 RAW（.cr2/.cr3/.nef/.arw/.raf/.dng 等）为中性 sRGB 图
/// 仅 exe（桌面端）提供；网页版不支持 RAW。
#[tauri::command]
fn decode_raw(path: String, max_dim: Option<usize>) -> Result<RawResult, String> {
    let r = core_decode_raw(&path, max_dim.unwrap_or(3840))?;
    Ok(RawResult {
        width: r.width,
        height: r.height,
        data: base64::engine::general_purpose::STANDARD.encode(&r.rgba),
    })
}

/// 解码 RAW（前端把文件字节以 base64 传入，复用现有上传入口）
#[tauri::command]
fn decode_raw_b64(data: String, max_dim: Option<usize>) -> Result<RawResult, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| e.to_string())?;
    let r = core_decode_raw_bytes(&bytes, max_dim.unwrap_or(3840))?;
    Ok(RawResult {
        width: r.width,
        height: r.height,
        data: base64::engine::general_purpose::STANDARD.encode(&r.rgba),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![build_lut, write_text, write_b64, get_version, decode_raw, decode_raw_b64])
        .register_uri_scheme_protocol("app", |_ctx, _req| {
            let html = decrypt_frontend();
            Response::builder()
                .header(CONTENT_TYPE, "text/html; charset=utf-8")
                .header("Content-Length", html.len().to_string())
                .body(Cow::Owned(html))
                .unwrap()
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
