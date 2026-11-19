use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Serialize)]
pub struct VideoInfo {
    title: String,
    uploader: String,
    duration: f64,
    thumbnail: String,
    webpage_url: String,
}

fn format_timestamp(seconds: u64) -> String {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

// ============================================================================
// IMPLEMENTASI DESKTOP (WINDOWS / MACOS / LINUX) - MENGGUNAKAN YT-DLP SIDECAR
// ============================================================================
#[cfg(not(target_os = "android"))]
mod desktop {
    use super::*;
    use std::path::PathBuf;
    use tauri_plugin_shell::ShellExt;

    #[derive(Deserialize)]
    struct YtDlpJson {
        title: Option<String>,
        uploader: Option<String>,
        channel: Option<String>,
        duration: Option<f64>,
        thumbnail: Option<String>,
        webpage_url: Option<String>,
    }

    pub async fn get_video_info(app: AppHandle, url: String) -> Result<VideoInfo, String> {
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

        let uploader_name = parsed.uploader.or(parsed.channel).unwrap_or_default();

        Ok(VideoInfo {
            title: parsed.title.unwrap_or_else(|| "Video YouTube".into()),
            uploader: uploader_name,
            duration: parsed.duration.unwrap_or(0.0),
            thumbnail: parsed.thumbnail.unwrap_or_default(),
            webpage_url: parsed.webpage_url.unwrap_or(trimmed),
        })
    }

    pub async fn download_clip(
        app: AppHandle,
        url: String,
        start_seconds: u64,
        end_seconds: u64,
        output_path: String,
    ) -> Result<String, String> {
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
}

// ============================================================================
// IMPLEMENTASI ANDROID - VISITOR TOKEN + IOS/TV CLIENT + LIBFFMPEG.SO
// ============================================================================
#[cfg(target_os = "android")]
mod android {
    use super::*;
    use serde_json::{json, Value};
    use std::path::PathBuf;
    use std::time::Duration;
    use tokio::process::Command;

    fn extract_video_id(url: &str) -> Option<String> {
        let trimmed = url.trim();
        if let Some(pos) = trimmed.find("v=") {
            let id_part = &trimmed[pos + 2..];
            let id: String = id_part.chars().take_while(|&c| c != '&' && c != '#').collect();
            if id.len() == 11 {
                return Some(id);
            }
        }
        for prefix in ["youtu.be/", "youtube.com/shorts/", "youtube.com/embed/", "youtube.com/live/"] {
            if let Some(pos) = trimmed.find(prefix) {
                let id_part = &trimmed[pos + prefix.len()..];
                let id: String = id_part
                    .chars()
                    .take_while(|&c| c != '?' && c != '&' && c != '/')
                    .collect();
                if id.len() == 11 {
                    return Some(id);
                }
            }
        }
        if trimmed.len() == 11 && trimmed.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Some(trimmed.to_string());
        }
        None
    }

    fn find_libffmpeg() -> Result<PathBuf, String> {
        if let Ok(maps) = std::fs::read_to_string("/proc/self/maps") {
            for line in maps.lines() {
                if let Some(idx) = line.find("libapp_lib.so") {
                    let path_part = &line[..idx];
                    if let Some(start) = path_part.find('/') {
                        let lib_dir = PathBuf::from(&path_part[start..]);
                        let candidate = lib_dir.join("libffmpeg.so");
                        if candidate.exists() {
                            return Ok(candidate);
                        }
                    }
                }
            }
        }

        if let Ok(entries) = std::fs::read_dir("/data/app") {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Ok(sub_entries) = std::fs::read_dir(&p) {
                    for sub in sub_entries.flatten() {
                        let sub_path = sub.path();
                        let s_str = sub_path.to_string_lossy();
                        if s_str.contains("clipyt") || s_str.contains("tauri") || s_str.contains("app") {
                            let candidate = sub_path.join("lib").join("arm64").join("libffmpeg.so");
                            if candidate.exists() {
                                return Ok(candidate);
                            }
                        }
                    }
                }
                let direct_candidate = p.join("lib").join("arm64").join("libffmpeg.so");
                if direct_candidate.exists() {
                    return Ok(direct_candidate);
                }
            }
        }

        Err("Binary libffmpeg.so tidak ditemukan. Pastikan useLegacyPackaging = true aktif.".into())
    }

    fn build_http_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .cookie_store(true)
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    }

    fn extract_quoted_value(html: &str, key: &str) -> Option<String> {
        let pattern = format!("\"{}\":\"", key);
        let pos = html.find(&pattern)?;
        let rest = &html[pos + pattern.len()..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    pub async fn get_video_info(url: String) -> Result<VideoInfo, String> {
        let video_id = extract_video_id(&url)
            .ok_or_else(|| "Tautan YouTube tidak valid. Pastikan berisi ID video 11 karakter.".to_string())?;
        let watch_url = format!("https://www.youtube.com/watch?v={}", video_id);
        let client = build_http_client();

        let mut title = String::from("Video YouTube");
        let mut uploader = String::new();
        let mut thumbnail = format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", video_id);

        let oembed_url = format!("https://www.youtube.com/oembed?url={}&format=json", watch_url);
        if let Ok(res) = client.get(&oembed_url).send().await {
            if let Ok(o_json) = res.json::<Value>().await {
                if let Some(t) = o_json["title"].as_str() {
                    title = t.to_string();
                }
                if let Some(a) = o_json["author_name"].as_str() {
                    uploader = a.to_string();
                }
                if let Some(th) = o_json["thumbnail_url"].as_str() {
                    thumbnail = th.to_string();
                }
            }
        }

        let mut duration = 0.0;
        if let Ok(res) = client
            .get(&watch_url)
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await
        {
            if let Ok(html) = res.text().await {
                if let Some(secs_str) = extract_quoted_value(&html, "lengthSeconds") {
                    if let Ok(secs) = secs_str.parse::<f64>() {
                        duration = secs;
                    }
                }
            }
        }

        if duration <= 0.0 {
            return Err("Gagal membaca durasi video. Coba ganti jaringan Wi-Fi/Data seluler lalu muat ulang.".into());
        }

        Ok(VideoInfo {
            title,
            uploader,
            duration,
            thumbnail,
            webpage_url: watch_url,
        })
    }

    fn parse_streaming_data(player_json: &Value) -> Option<(String, Option<String>)> {
        let streaming = &player_json["streamingData"];

        // 1. Cek formats gabungan (video + audio MP4 langsung)
        if let Some(formats) = streaming["formats"].as_array() {
            if let Some(u) = formats.iter().rev().find_map(|f| {
                let mime = f["mimeType"].as_str().unwrap_or("");
                if mime.contains("video/mp4") {
                    f["url"].as_str().map(|s| s.to_string())
                } else {
                    None
                }
            }) {
                return Some((u, None));
            }
        }

        // 2. Cek adaptiveFormats (video MP4 + audio M4A terpisah)
        if let Some(adaptive) = streaming["adaptiveFormats"].as_array() {
            let v_url = adaptive.iter().find_map(|f| {
                let mime = f["mimeType"].as_str().unwrap_or("");
                if mime.contains("video/mp4") {
                    f["url"].as_str().map(|s| s.to_string())
                } else {
                    None
                }
            });
            let a_url = adaptive.iter().find_map(|f| {
                let mime = f["mimeType"].as_str().unwrap_or("");
                if mime.contains("audio/mp4") {
                    f["url"].as_str().map(|s| s.to_string())
                } else {
                    None
                }
            });
            if let (Some(v), Some(a)) = (v_url, a_url) {
                return Some((v, Some(a)));
            }
        }

        // 3. Cek hlsManifestUrl (stream m3u8 yang umum diberikan oleh klien IOS)
        if let Some(hls) = streaming["hlsManifestUrl"].as_str() {
            if !hls.is_empty() {
                return Some((hls.to_string(), None));
            }
        }

        None
    }

    async fn resolve_stream_url(video_id: &str) -> Result<(String, Option<String>), String> {
        let client = build_http_client();
        let watch_url = format!("https://www.youtube.com/watch?v={}", video_id);
        let mut status_notes: Vec<String> = Vec::new();

        // Ambil visitorData asli dari halaman watch YouTube yang terbukti lolos di jaringan HP
        let mut visitor_data = String::new();
        if let Ok(res) = client
            .get(&watch_url)
            .header("Accept-Language", "id-ID,id;q=0.9,en-US;q=0.8")
            .send()
            .await
        {
            if let Ok(html) = res.text().await {
                if let Some(vd) = extract_quoted_value(&html, "visitorData") {
                    visitor_data = vd;
                }
            }
        }

        // Daftar klien yang memberikan direct URL / HLS tanpa signatureCipher
        let clients = [
            (
                "IOS",
                "com.google.ios.youtube/19.45.4 (iPhone16,2; U; CPU iOS 18_1_0 like Mac OS X;)",
                json!({
                    "clientName": "IOS",
                    "clientVersion": "19.45.4",
                    "deviceMake": "Apple",
                    "deviceModel": "iPhone16,2",
                    "osName": "iPhone",
                    "osVersion": "18.1.0.22B83",
                    "visitorData": visitor_data,
                    "hl": "id",
                    "gl": "ID"
                }),
            ),
            (
                "ANDROID_VR",
                "com.google.android.apps.youtube.vr.oculus/1.60.19 (Linux; U; Android 12L; Quest 3 Build/SQ3A.220605.009.A1) gzip",
                json!({
                    "clientName": "ANDROID_VR",
                    "clientVersion": "1.60.19",
                    "deviceMake": "Oculus",
                    "deviceModel": "Quest 3",
                    "androidSdkVersion": 32,
                    "osName": "Android",
                    "osVersion": "12L",
                    "visitorData": visitor_data,
                    "hl": "id",
                    "gl": "ID"
                }),
            ),
            (
                "TVHTML5",
                "Mozilla/5.0 (ChromiumStylePlatform) Cobalt/Version",
                json!({
                    "clientName": "TVHTML5",
                    "clientVersion": "7.20241113.15.00",
                    "visitorData": visitor_data,
                    "hl": "id",
                    "gl": "ID"
                }),
            ),
        ];

        for (label, ua, client_ctx) in clients {
            let body = json!({
                "context": {
                    "client": client_ctx
                },
                "videoId": video_id,
                "contentCheckOk": true,
                "racyCheckOk": true
            });

            let mut req = client
                .post("https://www.youtube.com/youtubei/v1/player?prettyPrint=false")
                .header("User-Agent", ua)
                .header("Content-Type", "application/json")
                .header("Origin", "https://www.youtube.com");

            if !visitor_data.is_empty() {
                req = req.header("X-Goog-Visitor-Id", &visitor_data);
            }

            if let Ok(res) = req.json(&body).send().await {
                if let Ok(p_json) = res.json::<Value>().await {
                    if let Some(streams) = parse_streaming_data(&p_json) {
                        return Ok(streams);
                    }
                    let st = p_json["playabilityStatus"]["status"].as_str().unwrap_or("UNKNOWN");
                    let rsn = p_json["playabilityStatus"]["reason"].as_str().unwrap_or("");
                    status_notes.push(format!("{}({}): {}", label, st, rsn));
                }
            }
        }

        let summary = if status_notes.is_empty() {
            "Tidak mendapat respons streamingData dari YouTube.".to_string()
        } else {
            status_notes.into_iter().take(2).collect::<Vec<_>>().join(" | ")
        };

        Err(format!("Gagal mengekstrak stream video: {}", summary))
    }

    pub async fn download_clip(
        url: String,
        start_seconds: u64,
        end_seconds: u64,
        output_path: String,
    ) -> Result<String, String> {
        let video_id = extract_video_id(&url)
            .ok_or_else(|| "Tautan YouTube tidak valid.".to_string())?;
        let ffmpeg_bin = find_libffmpeg()?;
        let (stream_url, audio_opt) = resolve_stream_url(&video_id).await?;

        let final_path = if output_path.trim().is_empty() {
            let download_dir = PathBuf::from("/storage/emulated/0/Download/ClipYT");
            let _ = tokio::fs::create_dir_all(&download_dir).await;
            download_dir
                .join(format!("ClipYT_{}_{}-{}.mp4", video_id, start_seconds, end_seconds))
                .to_string_lossy()
                .to_string()
        } else {
            output_path
        };

        let duration_clip = (end_seconds - start_seconds).to_string();
        let start_str = format_timestamp(start_seconds);

        let output = if let Some(audio_url) = audio_opt {
            Command::new(&ffmpeg_bin)
                .args([
                    "-y",
                    "-ss",
                    &start_str,
                    "-i",
                    &stream_url,
                    "-ss",
                    &start_str,
                    "-i",
                    &audio_url,
                    "-t",
                    &duration_clip,
                    "-c",
                    "copy",
                    "-movflags",
                    "+faststart",
                    &final_path,
                ])
                .output()
                .await
                .map_err(|e| format!("Gagal menjalankan FFmpeg: {}", e))?
        } else {
            Command::new(&ffmpeg_bin)
                .args([
                    "-y",
                    "-ss",
                    &start_str,
                    "-i",
                    &stream_url,
                    "-t",
                    &duration_clip,
                    "-c",
                    "copy",
                    "-movflags",
                    "+faststart",
                    &final_path,
                ])
                .output()
                .await
                .map_err(|e| format!("Gagal menjalankan FFmpeg: {}", e))?
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("FFmpeg gagal memotong video: {}", stderr.trim()));
        }

        Ok(final_path)
    }
}

// ============================================================================
// TAURI COMMANDS (ENTRY POINT UNTUK FRONTEND)
// ============================================================================
#[tauri::command]
async fn get_video_info(app: AppHandle, url: String) -> Result<VideoInfo, String> {
    #[cfg(not(target_os = "android"))]
    return desktop::get_video_info(app, url).await;

    #[cfg(target_os = "android")]
    {
        let _ = app;
        android::get_video_info(url).await
    }
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

    #[cfg(not(target_os = "android"))]
    return desktop::download_clip(app, url, start_seconds, end_seconds, output_path).await;

    #[cfg(target_os = "android")]
    {
        let _ = app;
        android::download_clip(url, start_seconds, end_seconds, output_path).await
    }
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