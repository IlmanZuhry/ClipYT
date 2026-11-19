# ClipYT

Aplikasi desktop lintas platform berbasis **Tauri 2.0** untuk mengambil metadata video dan memotong klip video (`.mp4`) secara presisi berdasarkan rentang waktu mulai dan berakhir tanpa perlu mengunduh seluruh durasi video secara manual.

## Fitur Utama

- **Pratinjau Metadata Otomatis:** Menampilkan judul video, nama kanal/pembuat, thumbnail, serta durasi total secara langsung dari tautan video.
- **Pemotongan Presisi:** Mengatur titik mulai (`start`) dan berakhir (`end`) menggunakan slider interaktif maupun input waktu (`HH:MM:SS`) hingga durasi maksimal 60 menit per klip.
- **Pemrosesan Cepat (`-c copy`):** Menggunakan FFmpeg untuk memotong segmen video ke format `.mp4` (`+faststart`) dengan cepat dan menjaga kualitas asli.
- **Antarmuka Ringan:** Dibangun dengan Tauri 2.0 (Rust backend + HTML/CSS/JavaScript frontend) sehingga konsumsi memori jauh lebih rendah dibandingkan aplikasi berbasis Electron.

## Tumpukan Teknologi (Tech Stack)

- **Framework:** Tauri 2.0
- **Backend:** Rust (`tokio`, `serde`, `reqwest`)
- **Frontend:** HTML5, CSS3, Vanilla JavaScript
- **Media Engine:** `yt-dlp` & `FFmpeg` (Sidecar biner eksternal untuk Desktop)

## Unduh & Instalasi (Windows)

Untuk pengguna akhir yang hanya ingin menjalankan aplikasi tanpa kompilasi kode sumber:

1. Buka halaman **[Releases](https://github.com/IlmanZuhry/ClipYT/releases)** di repository ini.
2. Unduh file installer (`.exe` / `.msi`) atau versi portabel terbaru untuk Windows.
3. Jalankan aplikasi (pastikan koneksi internet aktif saat memuat tautan video).

## Pengembangan Lokal (Build from Source)

### Prasyarat
- [Node.js](https://nodejs.org/) (v18 atau lebih baru)
- [Rust & Cargo](https://www.rust-lang.org/tools/install)
- [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/) sesuai sistem operasi (Visual Studio Build Tools & WebView2 untuk Windows)

### Langkah Menjalankan

1. **Clone repository:**
   ```bash
   git clone [https://github.com/IlmanZuhry/ClipYT.git](https://github.com/IlmanZuhry/ClipYT.git)
   cd ClipYT