use std::path::Path;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use image::{ImageBuffer, Rgb, RgbImage, GenericImageView};
use tracing::{debug, info, warn};

use crate::error::{VideoError, Result};
use crate::video::types::{Frame, VideoClip};

/// Video file metadata (simplified for pure Rust implementation)
#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub duration: f64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub codec: String,
    pub frame_count: i64,
}

/// Pure Rust video loader without FFmpeg dependency
pub struct VideoLoader {
    metadata_cache: HashMap<String, VideoMetadata>,
}

impl VideoLoader {
    pub fn new() -> Result<Self> {
        info!("Initialized pure Rust video loader (FFmpeg-free)");
        Ok(Self {
            metadata_cache: HashMap::new(),
        })
    }

    pub fn load_metadata<P: AsRef<Path>>(&mut self, path: P) -> Result<VideoMetadata> {
        let path = path.as_ref();
        let path_str = path.display().to_string();

        if let Some(metadata) = self.metadata_cache.get(&path_str) {
            return Ok(metadata.clone());
        }

        let metadata = if Self::is_image_file(path) {
            self.load_image_metadata(path)?
        } else {
            self.estimate_video_metadata(path)?
        };

        self.metadata_cache.insert(path_str, metadata.clone());
        Ok(metadata)
    }

    fn load_image_metadata<P: AsRef<Path>>(&self, path: P) -> Result<VideoMetadata> {
        let image = image::open(path.as_ref()).map_err(|_| VideoError::LoadFailed {
            path: path.as_ref().display().to_string(),
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

    fn estimate_video_metadata<P: AsRef<Path>>(&self, path: P) -> Result<VideoMetadata> {
        let file_size = std::fs::metadata(path.as_ref())
            .map_err(|_| VideoError::LoadFailed {
                path: path.as_ref().display().to_string(),
            })?
            .len();

        let estimated_duration = (file_size as f64 / 1_000_000.0).clamp(1.0, 300.0);

        warn!("Using estimated metadata for: {:?}", path.as_ref());

        Ok(VideoMetadata {
            duration: estimated_duration,
            fps: 30.0,
            width: 1920,
            height: 1080,
            codec: "unknown".to_string(),
            frame_count: (estimated_duration * 30.0) as i64,
        })
    }

    pub fn extract_frame_at_time<P: AsRef<Path>>(
        &mut self,
        path: P,
        timestamp: f64
    ) -> Result<Frame> {
        if Self::is_image_file(path.as_ref()) {
            self.load_image_as_frame(path)
        } else {
            self.create_placeholder_frame(timestamp)
        }
    }

    fn load_image_as_frame<P: AsRef<Path>>(&self, path: P) -> Result<Frame> {
        let image = image::open(path.as_ref()).map_err(|_| VideoError::LoadFailed {
            path: path.as_ref().display().to_string(),
        })?;

        let rgb_image = match image {
            image::DynamicImage::ImageRgb8(img) => img,
            _ => image.to_rgb8(),
        };

        Ok(Frame::new(rgb_image))
    }

    fn create_placeholder_frame(&self, timestamp: f64) -> Result<Frame> {
        let hue = ((timestamp * 60.0) % 360.0) as f32;
        let color = self.hsv_to_rgb(hue, 0.6, 0.8);

        let mut frame = Frame::new_filled(640, 480, color);
        self.add_placeholder_pattern(&mut frame, timestamp);

        Ok(frame)
    }

    fn add_placeholder_pattern(&self, frame: &mut Frame, timestamp: f64) {
        let width = frame.width();
        let height = frame.height();

        for y in 0..height {
            for x in 0..width {
                if (x + y + (timestamp * 30.0) as u32) % 20 < 2 {
                    frame.set_pixel(x, y, [255, 255, 255]);
                }
            }
        }
    }

    fn hsv_to_rgb(&self, h: f32, s: f32, v: f32) -> [u8; 3] {
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

    pub fn extract_frames_at_times<P: AsRef<Path>>(
        &mut self,
        path: P,
        timestamps: &[f64],
    ) -> Result<Vec<Frame>> {
        let mut frames = Vec::with_capacity(timestamps.len());

        if Self::is_image_file(path.as_ref()) {
            let base_frame = self.load_image_as_frame(path)?;
            frames.resize(timestamps.len(), base_frame);
        } else {
            for &timestamp in timestamps {
                frames.push(self.create_placeholder_frame(timestamp)?);
            }
        }

        Ok(frames)
    }

    fn is_image_file<P: AsRef<Path>>(path: P) -> bool {
        match path.as_ref().extension().and_then(|ext| ext.to_str()) {
            Some(ext) => matches!(
                ext.to_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "bmp" | "gif" | "tiff" | "webp"
            ),
            None => false,
        }
    }

    pub fn is_supported<P: AsRef<Path>>(path: P) -> bool {
        let path = path.as_ref();
        Self::is_image_file(path) || matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some(ext) if matches!(
                ext.to_lowercase().as_str(),
                "mp4" | "avi" | "mov" | "mkv" | "webm"
            )
        )
    }

    pub fn create_video_clip<P: AsRef<Path>>(&mut self, path: P) -> Result<VideoClip> {
        let path = path.as_ref();

        // First try to parse as numbered clip (01_name.mp4)
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

        // If numbered parsing fails, create a clip with auto-assigned number
        if Self::is_supported(path) {
            let filename = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("video");

            // Use hash of filename to get consistent sequence number
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            use std::hash::{Hash, Hasher};
            filename.hash(&mut hasher);
            let sequence_number = (hasher.finish() % 1000) as u32 + 1; // 1-1000

            let mut clip = VideoClip::new(path, sequence_number, filename.to_string());

            if let Ok(metadata) = self.load_metadata(path) {
                clip.duration = Some(metadata.duration);
                clip.fps = Some(metadata.fps);
                clip.resolution = Some((metadata.width, metadata.height));
            }

            debug!("Auto-assigned sequence number {} to file: {}", sequence_number, filename);
            Ok(clip)
        } else {
            Err(VideoError::LoadFailed {
                path: path.display().to_string(),
            }.into())
        }
    }

    pub fn load_clips_from_directory<P: AsRef<Path>>(
        &mut self,
        directory: P,
    ) -> Result<Vec<VideoClip>> {
        let directory = directory.as_ref();
        let mut clips = Vec::new();

        if !directory.exists() || !directory.is_dir() {
            return Err(VideoError::LoadFailed {
                path: directory.display().to_string(),
            }.into());
        }

        for entry in std::fs::read_dir(directory)? {
            let path = entry?.path();

            if path.is_file() && !self.is_hidden_file(&path) && Self::is_supported(&path) {
                match self.create_video_clip(&path) {
                    Ok(clip) => {
                        info!("Loaded clip: {} (sequence: {}, duration: {:.1}s)", 
                              clip.name, clip.sequence_number, clip.duration.unwrap_or(0.0));
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
                path: format!("No supported video files found in {}", directory.display()),
            }.into());
        }

        // Sort clips by sequence number
        clips.sort_by_key(|clip| clip.sequence_number);

        info!("Loaded {} clips from directory", clips.len());
        if clips.iter().any(|c| Self::is_image_file(&c.path)) {
            info!("Image files detected - using static frame extraction");
        }
        if clips.iter().any(|c| !Self::is_image_file(&c.path)) {
            warn!("Video files detected but FFmpeg not available - using placeholder frames");
            warn!("For full video support, enable the 'ffmpeg' feature and install FFmpeg");
        }

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
        })
    }
}