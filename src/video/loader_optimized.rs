// src/video/loader_optimized.rs - Actually fast video loader

use std::path::Path;
use std::process::Command;
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

use rayon::prelude::*;
use image::{ImageBuffer, Rgb, RgbImage, GenericImageView};
use tracing::{debug, info, warn, error};

use crate::error::{VideoError, Result};
use crate::video::types::{Frame, VideoClip};

#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub duration: f64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub codec: String,
    pub frame_count: i64,
}

pub struct VideoLoader {
    metadata_cache: HashMap<String, VideoMetadata>,
    temp_counter: u32,
}

impl VideoLoader {
    pub fn new() -> Result<Self> {
        let cpu_count = num_cpus::get();
        info!("Detected {} CPU cores for parallel processing", cpu_count);

        if cfg!(target_os = "macos") {
            info!("macOS detected - will use VideoToolbox hardware acceleration");
        }

        let output = Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map_err(|_| VideoError::LoadFailed {
                path: "FFmpeg command not found".to_string(),
            })?;

        if output.status.success() {
            info!("Initialized video loader with external FFmpeg command");
            Ok(Self {
                metadata_cache: HashMap::new(),
                temp_counter: 0,
            })
        } else {
            Err(VideoError::LoadFailed {
                path: "FFmpeg command failed".to_string(),
            }.into())
        }
    }

    pub fn load_metadata<P: AsRef<Path>>(&mut self, path: P) -> Result<VideoMetadata> {
        let path_str = path.as_ref().display().to_string();

        if let Some(metadata) = self.metadata_cache.get(&path_str) {
            return Ok(metadata.clone());
        }

        let metadata = if Self::is_image_file(path.as_ref()) {
            self.load_image_metadata(path.as_ref())?
        } else {
            self.load_video_metadata_ffprobe(path.as_ref())?
        };

        self.metadata_cache.insert(path_str, metadata.clone());
        Ok(metadata)
    }

    fn load_image_metadata(&self, path: &Path) -> Result<VideoMetadata> {
        let image = image::open(path).map_err(|_| VideoError::LoadFailed {
            path: path.display().to_string(),
        })?;

        let (width, height) = image.dimensions();
        Ok(VideoMetadata {
            duration: 1.0 / 30.0,
            fps: 30.0,
            width,
            height,
            codec: "image".to_string(),
            frame_count: 1,
        })
    }

    fn load_video_metadata_ffprobe(&self, path: &Path) -> Result<VideoMetadata> {
        let output = Command::new("ffprobe")
            .args(&[
                "-v", "quiet",
                "-print_format", "json",
                "-show_streams",
                "-select_streams", "v:0",
                &path.display().to_string()
            ])
            .output()
            .map_err(|_| VideoError::LoadFailed {
                path: format!("{}: ffprobe failed", path.display()),
            })?;

        if !output.status.success() {
            warn!("ffprobe failed for {}, using estimated metadata", path.display());
            return Ok(VideoMetadata {
                duration: 30.0,
                fps: 30.0,
                width: 1920,
                height: 1080,
                codec: "unknown".to_string(),
                frame_count: 900,
            });
        }

        let json_output = String::from_utf8(output.stdout).map_err(|_| VideoError::LoadFailed {
            path: format!("{}: invalid ffprobe output", path.display()),
        })?;

        let width = self.extract_json_number(&json_output, "width").unwrap_or(1920.0) as u32;
        let height = self.extract_json_number(&json_output, "height").unwrap_or(1080.0) as u32;
        let duration = self.extract_json_number(&json_output, "duration").unwrap_or(30.0);
        let fps = self.extract_fps_from_json(&json_output).unwrap_or(30.0);

        info!("Video metadata: {}x{} @ {:.1}fps, {:.1}s", width, height, fps, duration);

        Ok(VideoMetadata {
            duration,
            fps,
            width,
            height,
            codec: "h264".to_string(),
            frame_count: (duration * fps) as i64,
        })
    }

    pub fn extract_frame_at_time<P: AsRef<Path>>(&mut self, path: P, timestamp: f64) -> Result<Frame> {
        if Self::is_image_file(path.as_ref()) {
            return self.load_image_as_frame(path.as_ref());
        }

        // For single frame, just use the batch method with one timestamp
        let frames = self.extract_frames_at_times(path, &[timestamp])?;
        frames.into_iter().next().ok_or_else(|| VideoError::FrameProcessingFailed {
            reason: "No frame extracted".to_string(),
        }.into())
    }

    /// **THIS IS THE KEY OPTIMIZATION** - Extract all frames in parallel using multiple FFmpeg processes
    pub fn extract_frames_at_times<P: AsRef<Path>>(&mut self, path: P, timestamps: &[f64]) -> Result<Vec<Frame>> {
        if Self::is_image_file(path.as_ref()) {
            let base_frame = self.load_image_as_frame(path.as_ref())?;
            return Ok(vec![base_frame; timestamps.len()]);
        }

        if timestamps.is_empty() {
            return Ok(Vec::new());
        }

        info!("🚀 Extracting {} frames in parallel from {}", timestamps.len(), path.as_ref().display());

        // Create temp directory
        let temp_dir = format!("/tmp/retro_compositor_parallel_{}", std::process::id());
        std::fs::create_dir_all(&temp_dir).map_err(|_| VideoError::FrameProcessingFailed {
            reason: "Cannot create temp directory".to_string(),
        })?;

        let path_str = path.as_ref().display().to_string();

        // **PARALLEL EXTRACTION** - Use rayon to spawn multiple FFmpeg processes simultaneously
        let frame_results: Vec<(usize, Result<Frame>)> = timestamps
            .par_iter()
            .enumerate()
            .map(|(i, &timestamp)| {
                let temp_frame = format!("{}/frame_{:06}.png", temp_dir, i);

                // Create FFmpeg command with hardware acceleration
                let mut cmd = Command::new("ffmpeg");
                cmd.args(&[
                    "-ss", &timestamp.to_string(),
                    "-i", &path_str,
                    "-vframes", "1",
                    "-f", "image2",
                    "-q:v", "2"
                ]);

                // Add hardware acceleration for M2 MacBook Pro
                if cfg!(target_os = "macos") {
                    cmd.args(&["-hwaccel", "videotoolbox"]);
                }

                cmd.args(&["-y", &temp_frame]);

                // Execute FFmpeg
                let result = match cmd.output() {
                    Ok(output) if output.status.success() => {
                        // Check if file was created and has reasonable size
                        if Path::new(&temp_frame).exists() {
                            if let Ok(metadata) = std::fs::metadata(&temp_frame) {
                                if metadata.len() > 1000 {
                                    // Load the frame
                                    match image::open(&temp_frame) {
                                        Ok(img) => {
                                            let rgb_image = img.to_rgb8();
                                            let frame = Frame::new(rgb_image);
                                            // Clean up immediately
                                            let _ = std::fs::remove_file(&temp_frame);
                                            Ok(frame)
                                        }
                                        Err(e) => Err(VideoError::FrameProcessingFailed {
                                            reason: format!("Image load failed: {}", e),
                                        }.into())
                                    }
                                } else {
                                    Err(VideoError::FrameProcessingFailed {
                                        reason: "Frame file too small".to_string(),
                                    }.into())
                                }
                            } else {
                                Err(VideoError::FrameProcessingFailed {
                                    reason: "Cannot read frame metadata".to_string(),
                                }.into())
                            }
                        } else {
                            Err(VideoError::FrameProcessingFailed {
                                reason: "Frame file not created".to_string(),
                            }.into())
                        }
                    }
                    Ok(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        Err(VideoError::FrameProcessingFailed {
                            reason: format!("FFmpeg failed: {}", stderr),
                        }.into())
                    }
                    Err(e) => Err(VideoError::FrameProcessingFailed {
                        reason: format!("FFmpeg execution failed: {}", e),
                    }.into())
                };

                (i, result)
            })
            .collect();

        // Sort results by index and convert to final frame vector
        let mut sorted_results = frame_results;
        sorted_results.sort_by_key(|(i, _)| *i);

        let mut frames = Vec::with_capacity(timestamps.len());
        let mut success_count = 0;
        let mut error_count = 0;

        for (_, result) in sorted_results {
            match result {
                Ok(frame) => {
                    frames.push(frame);
                    success_count += 1;
                }
                Err(e) => {
                    error_count += 1;
                    // Create a simple placeholder frame for failed extractions
                    warn!("Frame extraction failed: {}", e);
                    frames.push(Frame::new_filled(1920, 1080, [64, 64, 64]));
                }
            }
        }

        // Clean up temp directory
        let _ = std::fs::remove_dir_all(&temp_dir);

        info!("✅ Parallel extraction complete: {}/{} frames successful", success_count, timestamps.len());
        if error_count > 0 {
            warn!("⚠️  {} frames failed, using placeholders", error_count);
        }

        Ok(frames)
    }

    fn load_image_as_frame(&self, path: &Path) -> Result<Frame> {
        let image = image::open(path).map_err(|e| VideoError::LoadFailed {
            path: format!("{}: {}", path.display(), e),
        })?;
        let rgb_image = image.to_rgb8();
        Ok(Frame::new(rgb_image))
    }

    // Helper methods for JSON parsing
    fn extract_json_number(&self, json: &str, key: &str) -> Option<f64> {
        let pattern = format!("\"{}\":", key);
        if let Some(start) = json.find(&pattern) {
            let start = start + pattern.len();
            let remaining = &json[start..];
            let remaining = remaining.trim_start().trim_start_matches('"');
            let end = remaining.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
                .unwrap_or(remaining.len());
            remaining[..end].trim_end_matches('"').parse().ok()
        } else {
            None
        }
    }

    fn extract_fps_from_json(&self, json: &str) -> Option<f64> {
        if let Some(start) = json.find("\"avg_frame_rate\":") {
            let start = start + 17;
            let remaining = &json[start..];
            let remaining = remaining.trim_start().trim_start_matches('"');

            if let Some(end) = remaining.find('"') {
                let fps_str = &remaining[..end];
                if let Some(slash_pos) = fps_str.find('/') {
                    let num: f64 = fps_str[..slash_pos].parse().unwrap_or(30.0);
                    let den: f64 = fps_str[slash_pos + 1..].parse().unwrap_or(1.0);
                    if den != 0.0 {
                        return Some(num / den);
                    }
                }
            }
        }
        None
    }

    fn is_image_file(path: &Path) -> bool {
        matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some(ext) if matches!(
                ext.to_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "bmp" | "gif" | "tiff" | "webp"
            )
        )
    }

    pub fn is_supported<P: AsRef<Path>>(path: P) -> bool {
        let path = path.as_ref();
        Self::is_image_file(path) || matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some(ext) if matches!(
                ext.to_lowercase().as_str(),
                "mp4" | "avi" | "mov" | "mkv" | "webm" | "m4v" | "flv"
            )
        )
    }

    pub fn create_video_clip<P: AsRef<Path>>(&mut self, path: P) -> Result<VideoClip> {
        let path = path.as_ref();

        if let Some(mut clip) = VideoClip::from_path(path) {
            if Self::is_supported(path) {
                if let Ok(metadata) = self.load_metadata(path) {
                    clip.duration = Some(metadata.duration);
                    clip.fps = Some(metadata.fps);
                    clip.resolution = Some((metadata.width, metadata.height));
                }
            }
            return Ok(clip);
        }

        if Self::is_supported(path) {
            let filename = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("video");

            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            filename.hash(&mut hasher);
            let sequence_number = (hasher.finish() % 1000) as u32 + 1;

            let mut clip = VideoClip::new(path, sequence_number, filename.to_string());

            if let Ok(metadata) = self.load_metadata(path) {
                clip.duration = Some(metadata.duration);
                clip.fps = Some(metadata.fps);
                clip.resolution = Some((metadata.width, metadata.height));
            }

            Ok(clip)
        } else {
            Err(VideoError::LoadFailed {
                path: path.display().to_string(),
            }.into())
        }
    }

    pub fn load_clips_from_directory<P: AsRef<Path>>(&mut self, directory: P) -> Result<Vec<VideoClip>> {
        let directory = directory.as_ref();
        let mut clips = Vec::new();

        if !directory.exists() || !directory.is_dir() {
            return Err(VideoError::LoadFailed {
                path: directory.display().to_string(),
            }.into());
        }

        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && !self.is_hidden_file(&path) && Self::is_supported(&path) {
                match self.create_video_clip(&path) {
                    Ok(clip) => {
                        info!("Loaded clip: {} (sequence: {}, {:.1}s, {}x{})", 
                              clip.name, 
                              clip.sequence_number, 
                              clip.duration.unwrap_or(0.0),
                              clip.resolution.map(|r| r.0).unwrap_or(0),
                              clip.resolution.map(|r| r.1).unwrap_or(0));
                        clips.push(clip);
                    }
                    Err(e) => {
                        warn!("Could not load clip {:?}: {}", path, e);
                    }
                }
            }
        }

        if clips.is_empty() {
            return Err(VideoError::LoadFailed {
                path: format!("No supported videos in {}", directory.display()),
            }.into());
        }

        clips.sort_by_key(|clip| clip.sequence_number);
        info!("Successfully loaded {} video clips with external FFmpeg", clips.len());
        Ok(clips)
    }

    fn is_hidden_file(&self, path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with('.'))
            .unwrap_or(false)
    }

    pub fn clear_cache(&mut self) {
        self.metadata_cache.clear();
    }
}

impl Default for VideoLoader {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            metadata_cache: HashMap::new(),
            temp_counter: 0,
        })
    }
}