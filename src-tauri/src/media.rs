use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::Value;

use crate::models::MediaProbe;

// ---------------------------------------------------------------------------
// Python command detection
// ---------------------------------------------------------------------------

pub fn python_cmd() -> &'static str {
    if Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
        "python3"
    } else {
        "python"
    }
}

// ---------------------------------------------------------------------------
// Media probing
// ---------------------------------------------------------------------------

pub fn command_exists(name: &str) -> bool {
    Command::new(name).arg("-version").output().is_ok()
}

pub fn probe_media(path: &str) -> Result<MediaProbe> {
    if !command_exists("ffprobe") {
        return Err(anyhow!("ffprobe is not installed or not available on PATH"));
    }

    let output = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            path,
        ])
        .output()
        .context("running ffprobe")?;

    if !output.status.success() {
        return Err(anyhow!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let json: Value = serde_json::from_slice(&output.stdout).context("parsing ffprobe JSON")?;
    let streams = json
        .get("streams")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let video = streams.iter().find(|s| s.get("codec_type").and_then(Value::as_str) == Some("video"));
    let audio = streams.iter().find(|s| s.get("codec_type").and_then(Value::as_str) == Some("audio"));

    let duration_sec = json
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(Value::as_str)
        .and_then(|d| d.parse::<f64>().ok());

    let fps = video
        .and_then(|s| s.get("r_frame_rate").or_else(|| s.get("avg_frame_rate")))
        .and_then(Value::as_str)
        .and_then(parse_fps_str);

    Ok(MediaProbe {
        duration_sec,
        has_video: video.is_some(),
        width: video.and_then(|s| s.get("width")).and_then(Value::as_i64),
        height: video.and_then(|s| s.get("height")).and_then(Value::as_i64),
        fps,
        video_codec: video.and_then(|s| s.get("codec_name")).and_then(Value::as_str).map(ToOwned::to_owned),
        audio_codec: audio.and_then(|s| s.get("codec_name")).and_then(Value::as_str).map(ToOwned::to_owned),
    })
}

fn parse_fps_str(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.split('/').collect();
    match parts.as_slice() {
        [num, den] => {
            let n = num.parse::<f64>().ok()?;
            let d = den.parse::<f64>().ok()?;
            if d == 0.0 { None } else { Some(n / d) }
        }
        _ => s.parse::<f64>().ok(),
    }
}

// ---------------------------------------------------------------------------
// Audio extraction
// ---------------------------------------------------------------------------

pub fn extract_audio(source_path: &str, project_dir: &Path) -> Result<PathBuf> {
    if !command_exists("ffmpeg") {
        return Err(anyhow!("ffmpeg is not installed or not available on PATH"));
    }

    std::fs::create_dir_all(project_dir)?;
    let output_path = project_dir.join("transcription_audio.wav");

    let output = Command::new("ffmpeg")
        .args(["-y", "-i", source_path, "-vn", "-ac", "1", "-ar", "16000"])
        .arg(&output_path)
        .output()
        .context("running ffmpeg audio extraction")?;

    if !output.status.success() {
        return Err(anyhow!(
            "ffmpeg audio extraction failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(output_path)
}

// ---------------------------------------------------------------------------
// Face tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Clone)]
pub struct FaceKeyframe {
    pub frame: u64,
    pub x: f64,
}

const TRACK_FACES_PY: &str = r#"
import cv2
import json
import sys
import argparse

def smooth(values, window=21):
    result = []
    half = window // 2
    for i in range(len(values)):
        start = max(0, i - half)
        end = min(len(values), i + half + 1)
        result.append(sum(values[start:end]) / (end - start))
    return result

def clamp(x, lo=0.1, hi=0.9):
    return max(lo, min(hi, x))

def norm_x(face, full_width):
    x, y, w, h = face
    return ((x + w / 2) * 2) / full_width

def dedup_faces(faces):
    kept = []
    for f in sorted(faces, key=lambda f: f[2] * f[3], reverse=True):
        fx_c = f[0] + f[2] / 2
        fy_c = f[1] + f[3] / 2
        too_close = any(
            ((fx_c - (k[0] + k[2]/2))**2 + (fy_c - (k[1] + k[3]/2))**2) ** 0.5
            < (f[2] + k[2]) * 0.5
            for k in kept
        )
        if not too_close:
            kept.append(f)
    return kept

def pick_target_x(faces, full_width, last_x):
    if not faces:
        return last_x
    by_area = sorted(faces, key=lambda f: f[2] * f[3], reverse=True)
    top = by_area[:2]
    if len(top) == 1:
        return clamp(norm_x(top[0], full_width))
    a0 = top[0][2] * top[0][3]
    a1 = top[1][2] * top[1][3]
    if a0 > a1 * 1.4:
        return clamp(norm_x(top[0], full_width))
    avg = (norm_x(top[0], full_width) + norm_x(top[1], full_width)) / 2
    return clamp(avg)

def detect_all_faces(small, profile_cascade, frontal_cascade):
    frontal = list(frontal_cascade.detectMultiScale(small, 1.1, 5, minSize=(35, 35)))
    prof_l = list(profile_cascade.detectMultiScale(small, 1.1, 5, minSize=(35, 35)))
    flipped = cv2.flip(small, 1)
    sw = small.shape[1]
    prof_r_raw = list(profile_cascade.detectMultiScale(flipped, 1.1, 5, minSize=(35, 35)))
    prof_r = [(sw - x - w, y, w, h) for x, y, w, h in prof_r_raw]
    return dedup_faces(frontal + prof_l + prof_r)

def track_faces(video_path, output_path, sample_every=5):
    cap = cv2.VideoCapture(video_path)
    if not cap.isOpened():
        print(f"Cannot open: {video_path}", file=sys.stderr)
        sys.exit(1)
    frontal_cascade = cv2.CascadeClassifier(cv2.data.haarcascades + 'haarcascade_frontalface_default.xml')
    profile_cascade = cv2.CascadeClassifier(cv2.data.haarcascades + 'haarcascade_profileface.xml')
    width = int(cap.get(cv2.CAP_PROP_FRAME_WIDTH))
    raw = []
    last_x = 0.5
    frame_num = 0
    while True:
        ret, frame = cap.read()
        if not ret:
            break
        if frame_num % sample_every == 0:
            gray = cv2.cvtColor(frame, cv2.COLOR_BGR2GRAY)
            small = cv2.resize(gray, (0, 0), fx=0.5, fy=0.5)
            faces = detect_all_faces(small, profile_cascade, frontal_cascade)
            last_x = pick_target_x(faces, width, last_x)
            raw.append((frame_num, last_x))
        frame_num += 1
    cap.release()
    if not raw:
        output = [{"frame": 0, "x": 0.5}]
    else:
        frames = [t[0] for t in raw]
        x_vals = [t[1] for t in raw]
        x_smooth = smooth(smooth(x_vals, window=21), window=11)
        output = [{"frame": f, "x": round(x, 4)} for f, x in zip(frames, x_smooth)]
    with open(output_path, 'w') as f:
        json.dump(output, f)

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('video', help='Input video path')
    parser.add_argument('output', help='Output JSON path')
    parser.add_argument('--sample', type=int, default=5)
    args = parser.parse_args()
    track_faces(args.video, args.output, args.sample)
"#;

/// Run face tracking on a clip segment. Returns normalized (time_sec_absolute, x_norm) pairs.
/// `start_sec`/`end_sec` are positions in the source video.
/// Falls back gracefully — callers should use `.ok()` so a missing opencv install doesn't block render.
pub fn run_face_tracker(
    source_path: &str,
    start_sec: f64,
    end_sec: f64,
    work_dir: &Path,
) -> Result<Vec<FaceKeyframe>> {
    let python = python_cmd();

    // Check opencv is available
    let has_cv2 = Command::new(python)
        .args(["-c", "import cv2"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !has_cv2 {
        return Err(anyhow!(
            "opencv-python not installed. Run: pip install opencv-python"
        ));
    }

    std::fs::create_dir_all(work_dir)?;

    // Write the embedded Python script
    let script_path = work_dir.join("track_faces.py");
    if !script_path.exists() {
        std::fs::write(&script_path, TRACK_FACES_PY.trim_start())
            .context("writing track_faces.py")?;
    }

    // Extract a downscaled clip for fast face detection (input seek for speed)
    let temp_clip = work_dir.join("_face_track_temp.mp4");
    let extract_out = Command::new("ffmpeg")
        .args([
            "-y",
            "-ss", &format!("{start_sec:.3}"),
            "-to", &format!("{end_sec:.3}"),
            "-i", source_path,
            "-vf", "scale=640:-2",
            "-an",
            "-c:v", "libx264",
            "-preset", "ultrafast",
            "-crf", "28",
        ])
        .arg(&temp_clip)
        .output()
        .context("extracting temp clip for face tracking")?;

    if !extract_out.status.success() {
        return Err(anyhow!(
            "FFmpeg temp clip extraction failed: {}",
            String::from_utf8_lossy(&extract_out.stderr)
        ));
    }

    // Run face tracker
    let keyframes_path = work_dir.join("_face_keyframes.json");
    let tracker_out = Command::new(python)
        .arg(script_path.to_string_lossy().as_ref())
        .arg(temp_clip.to_string_lossy().as_ref())
        .arg(keyframes_path.to_string_lossy().as_ref())
        .args(["--sample", "5"])
        .output()
        .context("running track_faces.py")?;

    let _ = std::fs::remove_file(&temp_clip);

    if !tracker_out.status.success() {
        return Err(anyhow!(
            "Face tracker failed: {}",
            String::from_utf8_lossy(&tracker_out.stderr)
        ));
    }

    let json_bytes = std::fs::read(&keyframes_path).context("reading face keyframes")?;
    let _ = std::fs::remove_file(&keyframes_path);
    let keyframes: Vec<FaceKeyframe> =
        serde_json::from_slice(&json_bytes).context("parsing face keyframes")?;

    Ok(keyframes)
}

/// Build the FFmpeg crop x= expression from face keyframes.
/// Uses piecewise linear interpolation (smooth pan) and a deadzone to suppress jitter.
fn build_face_crop_x_expr(keyframes: &[FaceKeyframe], fps: f64, start_sec: f64) -> String {
    // Ignore movements smaller than this fraction of frame width (prevents jitter from head sway)
    const DEADZONE: f64 = 0.05;

    if keyframes.is_empty() {
        return "(iw-out_w)/2".to_string();
    }

    // Resample to ~1 keyframe per second; apply deadzone to filter micro-movements
    let interval_frames = fps.max(1.0) as u64;
    let mut sampled: Vec<(f64, f64)> = Vec::new();
    let mut next_frame: u64 = 0;
    let mut committed_x: Option<f64> = None;

    for kf in keyframes {
        if kf.frame >= next_frame {
            let abs_t = start_sec + kf.frame as f64 / fps;
            let x = match committed_x {
                None => kf.x,
                Some(prev) if (kf.x - prev).abs() >= DEADZONE => kf.x,
                Some(prev) => prev,
            };
            committed_x = Some(x);
            sampled.push((abs_t, x));
            next_frame = kf.frame + interval_frames;
        }
    }

    if sampled.is_empty() {
        return "(iw-out_w)/2".to_string();
    }

    if sampled.len() == 1 {
        let x = sampled[0].1;
        return format!("max(0,min(iw-out_w,{x:.4}*iw-out_w/2))");
    }

    // Piecewise linear interpolation: for segment [T0 → T1]:
    //   pixel_x = x0*iw - out_w/2 + slope*iw*(t - T0)
    // This produces a smooth pan instead of a hard positional jump.

    let (_, last_x) = *sampled.last().unwrap();
    let mut expr = format!("{last_x:.4}*iw-out_w/2");

    for w in sampled.windows(2).rev() {
        let (t0, x0) = w[0];
        let (t1, x1) = w[1];
        let dt = t1 - t0;
        let slope = if dt > 0.001 { (x1 - x0) / dt } else { 0.0 };
        let seg = if slope.abs() < 0.0001 {
            format!("{x0:.4}*iw-out_w/2")
        } else {
            format!("{x0:.4}*iw-out_w/2+{slope:.6}*iw*(t-{t0:.3})")
        };
        expr = format!("if(lt(t,{t1:.3}),{seg},{expr})");
    }

    let (t0, x0) = sampled[0];
    expr = format!("if(lt(t,{t0:.3}),{x0:.4}*iw-out_w/2,{expr})");

    format!("max(0,min(iw-out_w,{expr}))")
}

// ---------------------------------------------------------------------------
// Clip rendering
// ---------------------------------------------------------------------------

pub fn render_flat_clip(
    source_path: &str,
    start_sec: f64,
    end_sec: f64,
    output_path: &Path,
    drawtext_filters: Option<&str>,
    face_track: Option<(&[FaceKeyframe], f64)>, // (keyframes, source_fps)
) -> Result<PathBuf> {
    if !command_exists("ffmpeg") {
        return Err(anyhow!("ffmpeg is not installed or not available on PATH"));
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let start = format!("{start_sec:.3}");
    let end = format!("{end_sec:.3}");

    let probe = probe_media(source_path).ok();
    let has_video = probe.as_ref().map(|p| p.has_video).unwrap_or(false);

    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-y", "-i", source_path, "-ss", &start, "-to", &end]);

    if has_video {
        // Build portrait crop: width = 9/16 of height, height = full
        let crop_w = "2*trunc(min(iw,ih*9/16)/2)";
        let crop_h = "2*trunc(min(ih,iw*16/9)/2)";

        let crop_x = if let Some((keyframes, fps)) = face_track {
            build_face_crop_x_expr(keyframes, fps, start_sec)
        } else {
            "(iw-out_w)/2".to_string()
        };

        let mut filter = format!("crop=w='{crop_w}':h='{crop_h}':x='{crop_x}'");

        if let Some(drawtext) = drawtext_filters {
            if !drawtext.is_empty() {
                filter = format!("{filter},{drawtext}");
            }
        }

        cmd.args(["-vf", &filter]);
        cmd.args(["-c:v", "libx264", "-preset", "fast", "-crf", "18", "-pix_fmt", "yuv420p"]);
    } else {
        cmd.arg("-vn");
    }

    cmd.args(["-c:a", "aac", "-b:a", "192k"]);
    cmd.arg(output_path);

    let output = cmd.output().context("running ffmpeg clip render")?;

    if !output.status.success() {
        return Err(anyhow!(
            "ffmpeg clip render failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(output_path.to_path_buf())
}
