use std::path::Path;
use std::process::{Command, Stdio};
use std::fs::{File, create_dir_all};
use std::io::Write;

use tracing::{debug, info, warn};
use tokio::task;

use crate::error::{VideoError, Result};
use crate::video::types::{Frame, VideoParams};
use crate::video::processor::ProcessedSegment;

/// Represents an encoded video output
#[derive(Debug, Clone)]
pub struct EncodedVideo {
    pub path: String,
    pub duration: f64,
    pub frame_count: usize,
    pub file_size: u64,
}

/// Pure Rust video compositor using external FFmpeg commands
pub struct VideoCompositor {
    params: VideoParams,
    temp_dir: Option<String>,
}

impl VideoCompositor {
    pub fn new(params: VideoParams) -> Self {
        Self {
            params,
            temp_dir: None,
        }
    }

    pub fn check_ffmpeg_available() -> bool {
        Command::new("ffmpeg")
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    fn ensure_temp_dir(&mut self) -> Result<String> {
        if let Some(ref temp_dir) = self.temp_dir {
            return Ok(temp_dir.clone());
        }

        let temp_dir = format!("./temp_retro_compositor_{}", std::process::id());
        create_dir_all(&temp_dir)?;
        self.temp_dir = Some(temp_dir.clone());
        Ok(temp_dir)
    }

    pub async fn compose_video<P: AsRef<Path>>(
        &mut self,
        segments: &[ProcessedSegment],
        audio_path: P,
        output_path: P,
    ) -> Result<EncodedVideo> {
        info!("Composing video with {} segments", segments.len());

        if !Self::check_ffmpeg_available() {
            return Err(VideoError::EncodingFailed {
                reason: "FFmpeg not found. Please install FFmpeg.".to_string(),
            }.into());
        }

        let temp_dir = self.ensure_temp_dir()?;

        let frame_paths = self.save_frames_as_images(segments, &temp_dir).await?;
        let frame_list_path = self.create_frame_list(&frame_paths, &temp_dir)?;

        let video_only_path = format!("{}/video_only.mp4", temp_dir);
        self.encode_video_from_frames(&frame_list_path, &video_only_path).await?;

        // Get output path as string before moving
        let output_path_str = output_path.as_ref().display().to_string();

        self.combine_video_and_audio(&video_only_path, audio_path, &output_path_str).await?;

        let encoded_video = self.get_output_info(&output_path_str, segments).await?;

        info!("Video composition complete: {}MB", 
              encoded_video.file_size / 1024 / 1024);

        Ok(encoded_video)
    }

    async fn save_frames_as_images(
        &self,
        segments: &[ProcessedSegment],
        temp_dir: &str,
    ) -> Result<Vec<String>> {
        let mut frame_paths = Vec::new();
        let mut frame_counter = 0;

        debug!("Saving frames to directory: {}", temp_dir);

        for segment in segments {
            for frame in &segment.frames {
                let frame_path = format!("{}/frame_{:06}.png", temp_dir, frame_counter);

                debug!("Saving frame to: {}", frame_path);

                frame.save_png(&frame_path).map_err(|e| VideoError::EncodingFailed {
                    reason: format!("Failed to save frame: {}", e),
                })?;

                // Verify the file was created
                if !std::path::Path::new(&frame_path).exists() {
                    return Err(VideoError::EncodingFailed {
                        reason: format!("Frame file not created: {}", frame_path),
                    }.into());
                }

                frame_paths.push(frame_path);
                frame_counter += 1;
            }
        }

        info!("Saved {} frames as images", frame_counter);
        Ok(frame_paths)
    }

    fn create_frame_list(&self, frame_paths: &[String], temp_dir: &str) -> Result<String> {
        let list_path = format!("{}/frame_list.txt", temp_dir);
        let mut file = File::create(&list_path)?;

        let frame_duration = 1.0 / self.params.fps;

        for frame_path in frame_paths {
            // Use absolute path to avoid path resolution issues
            let absolute_path = std::path::Path::new(frame_path)
                .canonicalize()
                .unwrap_or_else(|_| std::path::PathBuf::from(frame_path));

            writeln!(file, "file '{}'", absolute_path.display())?;
            writeln!(file, "duration {:.6}", frame_duration)?;
        }

        if let Some(last_frame) = frame_paths.last() {
            let absolute_path = std::path::Path::new(last_frame)
                .canonicalize()
                .unwrap_or_else(|_| std::path::PathBuf::from(last_frame));
            writeln!(file, "file '{}'", absolute_path.display())?;
        }

        Ok(list_path)
    }

    async fn encode_video_from_frames(&self, frame_list_path: &str, output_path: &str) -> Result<()> {
        let mut cmd = Command::new("ffmpeg");
        cmd.args(&[
            "-f", "concat",
            "-safe", "0",
            "-i", frame_list_path,
            "-c:v", &self.params.codec,
            "-r", &self.params.fps.to_string(),
            "-pix_fmt", "yuv420p",
            "-crf", &self.quality_to_crf(self.params.quality).to_string(),
            "-y",
            output_path,
        ]);

        let output = task::spawn_blocking(move || cmd.output()).await
            .map_err(|e| VideoError::EncodingFailed {
                reason: format!("Failed to spawn FFmpeg process: {}", e),
            })?
            .map_err(|e| VideoError::EncodingFailed {
                reason: format!("FFmpeg execution failed: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VideoError::EncodingFailed {
                reason: format!("FFmpeg failed: {}", stderr),
            }.into());
        }

        Ok(())
    }

    async fn combine_video_and_audio<P: AsRef<Path>>(
        &self,
        video_path: &str,
        audio_path: P,
        output_path: &str,
    ) -> Result<()> {
        let mut cmd = Command::new("ffmpeg");
        cmd.args(&[
            "-i", video_path,
            "-i", &audio_path.as_ref().display().to_string(),
            "-c:v", "copy",
            "-c:a", "aac",
            "-shortest",
            "-y",
            output_path,
        ]);

        let output = task::spawn_blocking(move || cmd.output()).await
            .map_err(|e| VideoError::EncodingFailed {
                reason: format!("Failed to spawn FFmpeg process: {}", e),
            })?
            .map_err(|e| VideoError::EncodingFailed {
                reason: format!("FFmpeg execution failed: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VideoError::EncodingFailed {
                reason: format!("FFmpeg failed: {}", stderr),
            }.into());
        }

        Ok(())
    }

    fn quality_to_crf(&self, quality: u8) -> u8 {
        (51 - ((quality as f32 / 100.0) * 51.0) as u8).clamp(0, 51)
    }

    async fn get_output_info<P: AsRef<Path>>(
        &self,
        output_path: P,
        segments: &[ProcessedSegment],
    ) -> Result<EncodedVideo> {
        let metadata = std::fs::metadata(output_path.as_ref())?;
        let duration = segments.last().map(|s| s.end_time).unwrap_or(0.0);
        let frame_count = segments.iter().map(|s| s.frames.len()).sum();

        Ok(EncodedVideo {
            path: output_path.as_ref().display().to_string(),
            duration,
            frame_count,
            file_size: metadata.len(),
        })
    }

    pub async fn create_test_video<P: AsRef<Path>>(
        &mut self,
        output_path: P,
        duration_seconds: f64,
    ) -> Result<EncodedVideo> {
        if !Self::check_ffmpeg_available() {
            return Err(VideoError::EncodingFailed {
                reason: "FFmpeg not found".to_string(),
            }.into());
        }

        let temp_dir = self.ensure_temp_dir()?;
        let frame_count = (duration_seconds * self.params.fps) as usize;
        let mut frame_paths = Vec::new();

        for i in 0..frame_count {
            let hue = (i as f32 / frame_count as f32) * 360.0;
            let color = Self::hsv_to_rgb(hue, 0.7, 0.9);

            let frame = Frame::new_filled(
                self.params.resolution.0,
                self.params.resolution.1,
                color,
            );

            let frame_path = format!("{}/test_frame_{:06}.png", temp_dir, i);
            frame.save_png(&frame_path).map_err(|e| VideoError::EncodingFailed {
                reason: format!("Failed to save test frame: {}", e),
            })?;
            frame_paths.push(frame_path);
        }

        let frame_list_path = self.create_frame_list(&frame_paths, &temp_dir)?;
        self.encode_video_from_frames(&frame_list_path, &output_path.as_ref().display().to_string()).await?;

        let metadata = std::fs::metadata(output_path.as_ref())?;
        Ok(EncodedVideo {
            path: output_path.as_ref().display().to_string(),
            duration: duration_seconds,
            frame_count,
            file_size: metadata.len(),
        })
    }

    fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r, g, b) = if h < 60.0 {
            (c, x, 0.0)
        } else if h < 120.0 {
            (x, c, 0.0)
        } else if h < 180.0 {
            (0.0, c, x)
        } else if h < 240.0 {
            (0.0, x, c)
        } else if h < 300.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        [
            ((r + m) * 255.0) as u8,
            ((g + m) * 255.0) as u8,
            ((b + m) * 255.0) as u8,
        ]
    }

    pub fn cleanup(&mut self) -> Result<()> {
        if let Some(temp_dir) = &self.temp_dir {
            if let Err(e) = std::fs::remove_dir_all(temp_dir) {
                warn!("Failed to remove temporary directory: {}", e);
            }
            self.temp_dir = None;
        }
        Ok(())
    }
}

impl Drop for VideoCompositor {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}