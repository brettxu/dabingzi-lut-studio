use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::Engine;
use lut_core::{build_lut as core_build_lut, MatchStats, Params};
use std::borrow::Cow;
use tauri::http::{header::CONTENT_TYPE, Response};
use tauri_plugin_dialog::DialogExt;

/// 前端资源解密密钥（AES-256-GCM）
const APP_KEY: [u8; 32] = [
    0x9c, 0x1f, 0x4a, 0x7e, 0x3b, 0x8d, 0x2f, 0x56, 0xa1, 0xc9, 0xe7, 0xd0, 0x4b, 0x6f, 0x8a, 0x23,
    0xe5, 0xd7, 0x1c, 0x9b, 0x4a, 0x8f, 0x0e, 0x3d, 0x2c, 0x5b, 0x7a, 0x19, 0xf4, 0xe8, 0xd3, 0xc6,
];
const APP_NONCE: [u8; 12] = [
    0xb3, 0xa7, 0xd9, 0x1e, 0x4c, 0x6f, 0x82, 0x05, 0x3d, 0x9a, 0x71, 0xc4,
];

/// 解密内嵌的加密前端资源（HTML 不落盘，运行时解密）
fn decrypt_frontend() -> Vec<u8> {
    let data = include_bytes!("../resources/app_data.enc");
    let cipher = Aes256Gcm::new_from_slice(&APP_KEY).expect("invalid key length");
    let nonce = Nonce::from_slice(&APP_NONCE);
    cipher
        .decrypt(nonce, data.as_ref())
        .expect("failed to decrypt frontend resource")
}

/// 核心命令：根据参数生成 LUT（算法在 Rust 机器码中，前端 JS 不可见）
/// 返回 base64 编码的 Float32Array（N*N*N*3 个 float，little-endian）
#[tauri::command]
fn build_lut(size: usize, params: Params, match_stats: Option<MatchStats>) -> String {
    let lut: Vec<f32> = core_build_lut(size, &params, match_stats.as_ref());
    let bytes: Vec<u8> = lut.iter().flat_map(|v| v.to_le_bytes()).collect();
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}

/// 保存 .cube LUT：弹出系统"另存为"对话框后写入文件
#[tauri::command]
async fn save_cube(app: tauri::AppHandle, filename: String, content: String) -> Result<String, String> {
    let path = app
        .dialog()
        .file()
        .set_file_name(&filename)
        .add_filter("LUT 文件", &["cube"])
        .blocking_save_file()
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

/// 保存效果图（base64 PNG）：弹出系统"另存为"对话框后写入文件
#[tauri::command]
async fn save_image(app: tauri::AppHandle, filename: String, data: String) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| e.to_string())?;
    let path = app
        .dialog()
        .file()
        .set_file_name(&filename)
        .add_filter("PNG 图片", &["png"])
        .blocking_save_file()
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![build_lut, save_cube, save_image])
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
