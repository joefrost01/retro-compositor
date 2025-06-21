use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::audio::types::{AudioData, AudioFormat};
use crate::error::{AudioError, Result};

/// Audio file loader supporting multiple formats
pub struct AudioLoader;

impl AudioLoader {
    /// Load an audio file and return raw audio data
    pub async fn load<P: AsRef<Path>>(path: P) -> Result<AudioData> {
        let path = path.as_ref();
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            "wav" => Self::load_wav(path).await,
            "mp3" | "flac" | "ogg" | "m4a" | "aac" => Self::load_with_symphonia(path).await,
            _ => Err(AudioError::UnsupportedFormat {
                format: extension
            }.into()),
        }
    }

    /// Load WAV files using the hound crate (most reliable for WAV)
    async fn load_wav<P: AsRef<Path>>(path: P) -> Result<AudioData> {
        let path = path.as_ref();

        let reader = hound::WavReader::open(path)
            .map_err(|_| AudioError::LoadFailed {
                path: path.display().to_string()
            })?;

        let spec = reader.spec();
        let sample_rate = spec.sample_rate;
        let channels = spec.channels;

        // Convert samples to f32
        let samples: Result<Vec<f32>> = match spec.sample_format {
            hound::SampleFormat::Float => {
                reader.into_samples::<f32>()
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|_| AudioError::LoadFailed {
                        path: path.display().to_string()
                    }.into())
            }
            hound::SampleFormat::Int => {
                let bit_depth = spec.bits_per_sample;
                let samples: std::result::Result<Vec<i32>, _> = reader.into_samples().collect();

                Ok(samples
                    .map_err(|_| AudioError::LoadFailed {
                        path: path.display().to_string()
                    })?
                    .into_iter()
                    .map(|sample| Self::int_to_float(sample, bit_depth))
                    .collect::<Vec<f32>>())
            }
        };

        let samples = samples?;
        let duration = samples.len() as f64 / (sample_rate * channels as u32) as f64;

        Ok(AudioData {
            samples,
            sample_rate,
            channels,
            duration,
            file_path: path.to_path_buf(),
            format: AudioFormat {
                extension: "wav".to_string(),
                bit_depth: Some(spec.bits_per_sample),
                compression: None,
                bitrate: None,
            },
        })
    }

    /// Load various formats using Symphonia
    async fn load_with_symphonia<P: AsRef<Path>>(path: P) -> Result<AudioData> {
        let path = path.as_ref();

        // Open the file
        let file = File::open(path)
            .map_err(|_| AudioError::LoadFailed {
                path: path.display().to_string()
            })?;

        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        // Create a probe hint using the file extension
        let mut hint = Hint::new();
        if let Some(extension) = path.extension() {
            if let Some(extension_str) = extension.to_str() {
                hint.with_extension(extension_str);
            }
        }

        // Use the default options for metadata and format readers
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();

        // Probe the media source
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .map_err(|_| AudioError::LoadFailed {
                path: path.display().to_string()
            })?;

        // Get the instantiated format reader
        let mut format = probed.format;

        // Find the first audio track with a known (decodable) codec
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| AudioError::LoadFailed {
                path: path.display().to_string()
            })?;

        let track_id = track.id;

        // Get codec parameters and store needed values before mutably borrowing format
        let codec_params = &track.codec_params;
        let sample_rate = codec_params.sample_rate
            .ok_or_else(|| AudioError::InvalidParameters {
                details: "No sample rate found".to_string()
            })?;

        let channels = codec_params.channels
            .ok_or_else(|| AudioError::InvalidParameters {
                details: "No channel information found".to_string()
            })?
            .count() as u16;

        // Store codec info for later use
        let bits_per_sample = codec_params.bits_per_sample;
        let codec_type = codec_params.codec;

        // Create a decoder for the track
        let dec_opts: DecoderOptions = Default::default();
        let mut decoder = symphonia::default::get_codecs()
            .make(codec_params, &dec_opts)
            .map_err(|_| AudioError::LoadFailed {
                path: path.display().to_string()
            })?;

        // Decode all packets and collect samples
        let mut samples = Vec::new();

        loop {
            // Get the next packet from the media format
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::ResetRequired) => {
                    // Reset the decoder and try again
                    decoder.reset();
                    continue;
                }
                Err(SymphoniaError::IoError(_)) => break, // End of stream
                Err(_) => break,
            };

            // Consume any new metadata
            while !format.metadata().is_latest() {
                format.metadata().pop();
            }

            // If the packet does not belong to the selected track, skip over it
            if packet.track_id() != track_id {
                continue;
            }

            // Decode the packet into an audio buffer
            match decoder.decode(&packet) {
                Ok(decoded) => {
                    // Convert the audio buffer to f32 samples
                    Self::convert_audio_buffer_to_f32(&decoded, &mut samples);
                }
                Err(SymphoniaError::IoError(_)) => break,
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(_) => break,
            }
        }

        let duration = samples.len() as f64 / (sample_rate * channels as u32) as f64;

        let format_info = AudioFormat {
            extension: path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("unknown")
                .to_string(),
            bit_depth: bits_per_sample.map(|b| b as u16),
            compression: Some(format!("{:?}", codec_type)),
            bitrate: None, // Symphonia doesn't expose max_bitrate easily
        };

        Ok(AudioData {
            samples,
            sample_rate,
            channels,
            duration,
            file_path: path.to_path_buf(),
            format: format_info,
        })
    }

    /// Convert integer sample to float (-1.0 to 1.0)
    fn int_to_float(sample: i32, bit_depth: u16) -> f32 {
        match bit_depth {
            8 => (sample as f32 - 128.0) / 128.0,
            16 => sample as f32 / 32768.0,
            24 => sample as f32 / 8388608.0,
            32 => sample as f32 / 2147483648.0,
            _ => sample as f32 / 32768.0, // Default to 16-bit
        }
    }

    /// Convert Symphonia audio buffer to f32 samples
    fn convert_audio_buffer_to_f32(buffer: &AudioBufferRef, output: &mut Vec<f32>) {
        match buffer {
            AudioBufferRef::F32(buf) => {
                // For planar f32, interleave the channels
                let channels = buf.spec().channels.count();
                let frames = buf.capacity();

                for frame_idx in 0..frames {
                    for ch in 0..channels {
                        let channel_buf = buf.chan(ch);
                        if frame_idx < channel_buf.len() {
                            output.push(channel_buf[frame_idx]);
                        }
                    }
                }
            }
            AudioBufferRef::F64(buf) => {
                // Convert f64 to f32 and interleave
                let channels = buf.spec().channels.count();
                let frames = buf.capacity();

                for frame_idx in 0..frames {
                    for ch in 0..channels {
                        let channel_buf = buf.chan(ch);
                        if frame_idx < channel_buf.len() {
                            output.push(channel_buf[frame_idx] as f32);
                        }
                    }
                }
            }
            AudioBufferRef::S32(buf) => {
                // Convert i32 to f32 and interleave
                let channels = buf.spec().channels.count();
                let frames = buf.capacity();

                for frame_idx in 0..frames {
                    for ch in 0..channels {
                        let channel_buf = buf.chan(ch);
                        if frame_idx < channel_buf.len() {
                            let sample = channel_buf[frame_idx] as f32 / 2147483648.0;
                            output.push(sample);
                        }
                    }
                }
            }
            AudioBufferRef::S16(buf) => {
                // Convert i16 to f32 and interleave
                let channels = buf.spec().channels.count();
                let frames = buf.capacity();

                for frame_idx in 0..frames {
                    for ch in 0..channels {
                        let channel_buf = buf.chan(ch);
                        if frame_idx < channel_buf.len() {
                            let sample = channel_buf[frame_idx] as f32 / 32768.0;
                            output.push(sample);
                        }
                    }
                }
            }
            _ => {
                // Handle other formats by attempting basic conversion
                tracing::warn!("Unsupported audio buffer format, attempting basic conversion");
            }
        }
    }

    /// Detect audio format from file extension
    pub fn detect_format<P: AsRef<Path>>(path: P) -> Option<String> {
        path.as_ref()
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase())
    }

    /// Check if a file format is supported
    pub fn is_format_supported(extension: &str) -> bool {
        matches!(
            extension.to_lowercase().as_str(),
            "wav" | "mp3" | "flac" | "ogg" | "m4a" | "aac"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::io::Write;

    #[test]
    fn test_format_detection() {
        assert_eq!(AudioLoader::detect_format("test.wav"), Some("wav".to_string()));
        assert_eq!(AudioLoader::detect_format("test.MP3"), Some("mp3".to_string()));
        assert_eq!(AudioLoader::detect_format("test"), None);
    }

    #[test]
    fn test_format_support() {
        assert!(AudioLoader::is_format_supported("wav"));
        assert!(AudioLoader::is_format_supported("mp3"));
        assert!(AudioLoader::is_format_supported("FLAC"));
        assert!(!AudioLoader::is_format_supported("xyz"));
    }

    #[test]
    fn test_int_to_float_conversion() {
        // Test 16-bit conversion
        assert_eq!(AudioLoader::int_to_float(0, 16), 0.0);
        assert_eq!(AudioLoader::int_to_float(32767, 16), 32767.0 / 32768.0);
        assert_eq!(AudioLoader::int_to_float(-32768, 16), -1.0);

        // Test 8-bit conversion  
        assert_eq!(AudioLoader::int_to_float(128, 8), 0.0);
        assert_eq!(AudioLoader::int_to_float(255, 8), 127.0 / 128.0);
        assert_eq!(AudioLoader::int_to_float(0, 8), -1.0);
    }

    #[tokio::test]
    async fn test_unsupported_format() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.xyz");

        // Create a dummy file
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"dummy content").unwrap();

        let result = AudioLoader::load(&file_path).await;
        assert!(result.is_err());

        if let Err(crate::error::CompositorError::Audio(AudioError::UnsupportedFormat { format })) = result {
            assert_eq!(format, "xyz");
        } else {
            panic!("Expected UnsupportedFormat error");
        }
    }
}