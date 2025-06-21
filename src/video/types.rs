use image::{ImageBuffer, Rgb, RgbImage};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a single video frame
///
/// This is a simple wrapper around an RGB image buffer that provides
/// convenient methods for pixel manipulation used by effects.
#[derive(Clone, Debug)]
pub struct Frame {
    buffer: RgbImage,
}

impl Frame {
    /// Create a new frame from an RGB image buffer
    pub fn new(buffer: RgbImage) -> Self {
        Self { buffer }
    }

    /// Create a new frame with the given dimensions filled with black
    pub fn new_black(width: u32, height: u32) -> Self {
        let buffer = ImageBuffer::new(width, height);
        Self { buffer }
    }

    /// Create a new frame with the given dimensions filled with the specified color
    pub fn new_filled(width: u32, height: u32, color: [u8; 3]) -> Self {
        let buffer = ImageBuffer::from_fn(width, height, |_, _| {
            Rgb(color)
        });
        Self { buffer }
    }

    /// Get the width of the frame
    pub fn width(&self) -> u32 {
        self.buffer.width()
    }

    /// Get the height of the frame
    pub fn height(&self) -> u32 {
        self.buffer.height()
    }

    /// Get a pixel at the given coordinates (returns RGB array)
    pub fn get_pixel(&self, x: u32, y: u32) -> [u8; 3] {
        let pixel = self.buffer.get_pixel(x, y);
        [pixel[0], pixel[1], pixel[2]]
    }

    /// Get a mutable reference to a pixel at the given coordinates
    pub fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut [u8] {
        let pixel = self.buffer.get_pixel_mut(x, y);
        &mut pixel.0
    }

    /// Set a pixel at the given coordinates
    pub fn set_pixel(&mut self, x: u32, y: u32, color: [u8; 3]) {
        self.buffer.put_pixel(x, y, Rgb(color));
    }

    /// Get the underlying image buffer
    pub fn as_image(&self) -> &RgbImage {
        &self.buffer
    }

    /// Get a mutable reference to the underlying image buffer
    pub fn as_image_mut(&mut self) -> &mut RgbImage {
        &mut self.buffer
    }

    /// Convert the frame to raw RGB bytes
    pub fn to_rgb_bytes(&self) -> Vec<u8> {
        self.buffer.as_raw().clone()
    }

    /// Create a frame from raw RGB bytes
    pub fn from_rgb_bytes(width: u32, height: u32, data: Vec<u8>) -> Option<Self> {
        ImageBuffer::from_raw(width, height, data)
            .map(|buffer| Self { buffer })
    }

    /// Save the frame as a PNG file
    pub fn save_png<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), image::ImageError> {
        self.buffer.save(path)
    }
}

/// Represents a video clip with metadata
#[derive(Debug, Clone)]
pub struct VideoClip {
    /// Path to the video file
    pub path: PathBuf,

    /// Sequence number (from filename like "01_intro.mp4")
    pub sequence_number: u32,

    /// Name/identifier for the clip
    pub name: String,

    /// Duration in seconds (if known)
    pub duration: Option<f64>,

    /// Frame rate (if known)
    pub fps: Option<f64>,

    /// Resolution (width, height)
    pub resolution: Option<(u32, u32)>,
}

impl VideoClip {
    /// Create a new video clip
    pub fn new<P: Into<PathBuf>>(path: P, sequence_number: u32, name: String) -> Self {
        Self {
            path: path.into(),
            sequence_number,
            name,
            duration: None,
            fps: None,
            resolution: None,
        }
    }

    /// Parse sequence number and name from a filename like "01_intro.mp4"
    pub fn from_path<P: Into<PathBuf>>(path: P) -> Option<Self> {
        let path = path.into();
        let filename = path.file_stem()?.to_str()?;

        // Split on first underscore to get sequence number and name
        let parts: Vec<&str> = filename.splitn(2, '_').collect();
        if parts.len() != 2 {
            return None;
        }

        let sequence_number = parts[0].parse().ok()?;
        let name = parts[1].to_string();

        Some(Self::new(path, sequence_number, name))
    }

    /// Get the file extension
    pub fn extension(&self) -> Option<&str> {
        self.path.extension()?.to_str()
    }

    /// Check if this is a supported video format
    pub fn is_supported(&self) -> bool {
        match self.extension() {
            Some("mp4") | Some("avi") | Some("mov") | Some("mkv") => true,
            Some("jpg") | Some("jpeg") | Some("png") | Some("bmp") => true, // Static images
            _ => false,
        }
    }
}

/// Video processing parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoParams {
    /// Target frame rate for output
    pub fps: f64,

    /// Target resolution (width, height)
    pub resolution: (u32, u32),

    /// Video codec to use for output
    pub codec: String,

    /// Quality setting (0-100, higher is better)
    pub quality: u8,
}

impl Default for VideoParams {
    fn default() -> Self {
        Self {
            fps: 30.0,
            resolution: (1920, 1080),
            codec: "h264".to_string(),
            quality: 85,
        }
    }
}

/// Represents a sequence of video clips in order
#[derive(Debug, Clone)]
pub struct VideoSequence {
    clips: Vec<VideoClip>,
}

impl VideoSequence {
    /// Create a new empty sequence
    pub fn new() -> Self {
        Self { clips: Vec::new() }
    }

    /// Add a clip to the sequence
    pub fn add_clip(&mut self, clip: VideoClip) {
        self.clips.push(clip);
        // Keep clips sorted by sequence number
        self.clips.sort_by_key(|clip| clip.sequence_number);
    }

    /// Get all clips in sequence order
    pub fn clips(&self) -> &[VideoClip] {
        &self.clips
    }

    /// Get the total number of clips
    pub fn len(&self) -> usize {
        self.clips.len()
    }

    /// Check if the sequence is empty
    pub fn is_empty(&self) -> bool {
        self.clips.is_empty()
    }

    /// Get a clip by its sequence number
    pub fn get_clip(&self, sequence_number: u32) -> Option<&VideoClip> {
        self.clips.iter().find(|clip| clip.sequence_number == sequence_number)
    }

    /// Get clips as an iterator
    pub fn iter(&self) -> impl Iterator<Item = &VideoClip> {
        self.clips.iter()
    }
}

impl Default for VideoSequence {
    fn default() -> Self {
        Self::new()
    }
}

impl FromIterator<VideoClip> for VideoSequence {
    fn from_iter<I: IntoIterator<Item = VideoClip>>(iter: I) -> Self {
        let mut sequence = Self::new();
        for clip in iter {
            sequence.add_clip(clip);
        }
        sequence
    }
}