use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

#[derive(Serialize)]
pub struct VideoInfo {
    title: String,
    uploader: String,
    duration: f64,
    thumbnail: String,
    webpage_url: String,
}

#[derive(Deserialize)]
struct YtDlpJson {
    title: Option<String>,
    uploader: Option<String>,
    channel: Option<String>,
    duration: Option<f64>,
    thumbnail: Option<String>,
    webpage_url: Option<String>,
}

fn format_timestamp(seconds: u64) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

#[tauri::command]
async fn get_video_info(app: AppHandle, url: String) -> Result<VideoInfo, String> {
    let trimmed = url.trim().to_string();
    if trimmed.is_empty() {
        return Err("Tautan YouTube tidak boleh kosong.".into());
    }

    let output = app
        .shell()
        .sidecar("yt-dlp")
        .map_err(|e| format!("Gagal memuat yt-dlp: {}", e))?
        .args([
            "--dump-single-json",
            "--no-playlist",
            "--no-warnings",
            &trimmed,
        ])
        .output()
        .await
        .map_err(|e| format!("Gagal menjalankan yt-dlp: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Gagal mengambil info video: {}", stderr.trim()));
    }

    let parsed: YtDlpJson = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Gagal memproses data video: {}", e))?;

    let uploader_name = parsed
        .uploader
        .or(parsed.channel)
        .unwrap_or_default();

    Ok(VideoInfo {
        title: parsed.title.unwrap_or_else(|| "Video YouTube".into()),
        uploader: uploader_name,
        duration: parsed.duration.unwrap_or(0.0),
        thumbnail: parsed.thumbnail.unwrap_or_default(),
        webpage_url: parsed.webpage_url.unwrap_or(trimmed),
    })
}

#[tauri::command]
async fn download_clip(
    app: AppHandle,
    url: String,
    start_seconds: u64,
    end_seconds: u64,
    output_path: String,
) -> Result<String, String> {
    if end_seconds <= start_seconds {
        return Err("Waktu berakhir harus lebih besar dari waktu mulai.".into());
    }
    if end_seconds - start_seconds > 3600 {
        return Err("Durasi maksimal clip adalah 60 menit.".into());
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let section = format!(
        "*{}-{}",
        format_timestamp(start_seconds),
        format_timestamp(end_seconds)
    );

    let output = app
        .shell()
        .sidecar("yt-dlp")
        .map_err(|e| format!("Gagal memuat yt-dlp: {}", e))?
        .args([
            "--no-playlist",
            "--no-warnings",
            "--force-overwrites",
            "--ffmpeg-location",
            exe_dir.to_str().unwrap_or("."),
            "--download-sections",
            &section,
            "--force-keyframes-at-cuts",
            "-f",
            "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/bv*+ba/b",
            "--merge-output-format",
            "mp4",
            "-o",
            &output_path,
            url.trim(),
        ])
        .output()
        .await
        .map_err(|e| format!("Proses pemotongan gagal: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Gagal memotong video: {}", stderr.trim()));
    }

    Ok(output_path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![get_video_info, download_clip])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}