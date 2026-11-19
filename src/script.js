const { invoke } = window.__TAURI__.core;

const videoUrl = document.querySelector("#video-url");
const loadButton = document.querySelector("#load-video");
const downloadButton = document.querySelector("#download-clip");
const startTime = document.querySelector("#start-time");
const endTime = document.querySelector("#end-time");
const startRange = document.querySelector("#start-range");
const endRange = document.querySelector("#end-range");
const statusText = document.querySelector("#status");
const selectedDuration = document.querySelector("#selected-duration");
const timelineEnd = document.querySelector("#timeline-end");

const isAndroid = /Android/i.test(navigator.userAgent);

let videoDuration = null;
let currentVideoTitle = "youtube-clip";

function sanitizeFilename(name) {
  return name
    .replace(/[^a-zA-Z0-9\s\-_()]/g, "")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 80) || "youtube-clip";
}

function toSeconds(value) {
  const parts = value.trim().split(":");
  if (!parts.length || parts.some((part) => !/^\d+(\.\d+)?$/.test(part))) return NaN;
  return parts.reverse().reduce((total, part, index) => total + Number(part) * (60 ** index), 0);
}

function formatTime(seconds, includeHours = false) {
  const value = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(value / 3600);
  const minutes = Math.floor((value % 3600) / 60);
  const secs = value % 60;
  const two = (number) => String(number).padStart(2, "0");
  return includeHours || hours ? `${two(hours)}:${two(minutes)}:${two(secs)}` : `${two(minutes)}:${two(secs)}`;
}

function updateDuration() {
  const start = toSeconds(startTime.value);
  const end = toSeconds(endTime.value);
  selectedDuration.textContent = Number.isFinite(start) && Number.isFinite(end) && end > start ? formatTime(end - start) : "--:--";
}

function syncRangesFromTimes() {
  const start = toSeconds(startTime.value);
  const end = toSeconds(endTime.value);
  if (videoDuration && Number.isFinite(start) && Number.isFinite(end)) {
    startRange.value = Math.min(start, videoDuration);
    endRange.value = Math.min(Math.max(end, start + 1), videoDuration);
  }
  updateDuration();
}

async function showPopup(title, body, kind = "info") {
  try {
    if (window.__TAURI__?.dialog?.message) {
      await window.__TAURI__.dialog.message(body, { title, kind });
    }
  } catch (_) {
    // Abaikan jika dialog tidak didukung di platform tertentu
  }
}

function setStatus(message, isError = false) {
  statusText.textContent = message;
  statusText.style.color = isError ? "#bf2522" : "";
}

async function loadVideo() {
  const url = videoUrl.value.trim();
  if (!url) {
    const msg = "Masukkan tautan video terlebih dahulu.";
    setStatus(msg, true);
    return showPopup("Tautan Kosong", msg, "warning");
  }
  loadButton.disabled = true;
  setStatus("Membaca informasi video...");
  try {
    const data = await invoke("get_video_info", { url });
    if (!data.duration) throw new Error("Durasi video tidak tersedia.");
    videoDuration = Math.floor(data.duration);
    currentVideoTitle = sanitizeFilename(data.title || "youtube-clip");
    document.querySelector("#thumbnail").src = data.thumbnail || "";
    document.querySelector("#video-title").textContent = data.title;
    document.querySelector("#video-uploader").textContent = data.uploader || "";
    document.querySelector("#video-duration").textContent = `Durasi video: ${formatTime(videoDuration, true)}`;
    document.querySelector("#video-panel").classList.remove("is-hidden");
    [startRange, endRange].forEach((range) => { range.max = videoDuration; });
    startRange.value = 0;
    endRange.value = Math.min(30, videoDuration);
    startTime.value = formatTime(0, true);
    endTime.value = formatTime(Number(endRange.value), true);
    timelineEnd.textContent = formatTime(videoDuration, true);
    downloadButton.disabled = false;
    updateDuration();
    setStatus("Video siap. Atur rentang clip Anda.");
  } catch (error) {
    downloadButton.disabled = true;
    const errMsg = typeof error === "string" ? error : (error.message || "Video tidak dapat dimuat.");
    setStatus(errMsg, true);
    await showPopup("Gagal Memuat Video", errMsg, "error");
  } finally {
    loadButton.disabled = false;
  }
}

async function downloadClip() {
  const start = toSeconds(startTime.value);
  const end = toSeconds(endTime.value);
  if (!videoDuration || !Number.isFinite(start) || !Number.isFinite(end) || end <= start || end > videoDuration) {
    const msg = "Periksa kembali waktu mulai dan berakhir clip.";
    setStatus(msg, true);
    return showPopup("Waktu Tidak Valid", msg, "warning");
  }

  downloadButton.disabled = true;
  try {
    let outputPath = "";

    if (!isAndroid) {
      const startLabel = formatTime(start, true).replace(/:/g, "-");
      const endLabel = formatTime(end, true).replace(/:/g, "-");
      const defaultFileName = `${currentVideoTitle} (${startLabel} - ${endLabel}).mp4`;

      setStatus("Membuka dialog penyimpanan...");
      outputPath = await window.__TAURI__.dialog.save({
        defaultPath: defaultFileName,
        filters: [{ name: "Video MP4", extensions: ["mp4"] }],
      });

      if (!outputPath) {
        setStatus("Penyimpanan dibatalkan.");
        return;
      }
    }

    setStatus("Membuat clip. Proses ini bergantung pada panjang dan format video...");
    const savedPath = await invoke("download_clip", {
      url: videoUrl.value.trim(),
      startSeconds: Math.floor(start),
      endSeconds: Math.floor(end),
      outputPath,
    });

    const finalLocation = savedPath || outputPath;
    const successMsg = `Clip berhasil disimpan ke:\n${finalLocation}`;
    setStatus(`Clip selesai disimpan ke: ${finalLocation}`);
    await showPopup("Berhasil", successMsg, "info");
  } catch (error) {
    const errMsg = typeof error === "string" ? error : (error.message || "Clip gagal dibuat.");
    setStatus(errMsg, true);
    await showPopup("Gagal Membuat Clip", errMsg, "error");
  } finally {
    downloadButton.disabled = false;
  }
}

loadButton.addEventListener("click", loadVideo);
videoUrl.addEventListener("keydown", (event) => { if (event.key === "Enter") loadVideo(); });
[startTime, endTime].forEach((input) => input.addEventListener("input", syncRangesFromTimes));
startRange.addEventListener("input", () => {
  const start = Number(startRange.value);
  if (start >= Number(endRange.value)) endRange.value = Math.min(videoDuration, start + 1);
  startTime.value = formatTime(start, true);
  endTime.value = formatTime(Number(endRange.value), true);
  updateDuration();
});
endRange.addEventListener("input", () => {
  const end = Number(endRange.value);
  if (end <= Number(startRange.value)) startRange.value = Math.max(0, end - 1);
  startTime.value = formatTime(Number(startRange.value), true);
  endTime.value = formatTime(end, true);
  updateDuration();
});
downloadButton.addEventListener("click", downloadClip);
updateDuration();