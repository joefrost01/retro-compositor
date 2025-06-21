// Test binary for video processing functionality

use std::path::PathBuf;
use retro_compositor::{
    video::{VideoLoader, VideoProcessor, VideoCompositor, VideoParams, Frame},
    styles::{VhsStyle, StyleConfig},
    config::Config,
    Style, // Import Style trait from the main lib re-export
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging with debug level to see processing details
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    println!("🎬 Testing Retro-Compositor Video Processing");

    // Test 1: Video Loader Initialization
    println!("\n1. Testing Video Loader...");
    match VideoLoader::new() {
        Ok(mut loader) => {
            println!("   ✅ Video loader initialized successfully");

            // Test format support detection
            println!("   Testing format support:");
            let test_formats = vec!["test.mp4", "test.avi", "test.mov", "test.mkv", "test.txt"];
            for format in test_formats {
                let supported = VideoLoader::is_supported(format);
                println!("     {} - {}", format, if supported { "✅ Supported" } else { "❌ Not supported" });
            }
        }
        Err(e) => {
            println!("   ❌ Failed to initialize video loader: {}", e);
            println!("   Note: This might happen if FFmpeg is not installed or properly configured");
        }
    }

    // Test 2: Video Processor
    println!("\n2. Testing Video Processor...");
    let video_params = VideoParams {
        fps: 30.0,
        resolution: (640, 480),
        codec: "h264".to_string(),
        quality: 85,
    };

    match VideoProcessor::new(video_params.clone()) {
        Ok(processor) => {
            println!("   ✅ Video processor initialized");

            let stats = processor.get_stats();
            println!("   Processor stats:");
            println!("     Target FPS: {:.1}", stats.target_fps);
            println!("     Target resolution: {}x{}", stats.target_resolution.0, stats.target_resolution.1);
            println!("     Cached clips: {}", stats.cached_clips);
        }
        Err(e) => {
            println!("   ❌ Failed to initialize video processor: {}", e);
        }
    }

    // Test 3: Frame Processing with Effects
    println!("\n3. Testing Frame Processing...");
    let style = VhsStyle::new();
    let style_config = StyleConfig::default()
        .set("scanline_intensity", 0.8)
        .set("color_bleeding", 0.6)
        .set("noise_level", 0.4);

    // Create test frames
    let mut test_frames = vec![
        Frame::new_filled(320, 240, [255, 100, 100]), // Red-ish
        Frame::new_filled(320, 240, [100, 255, 100]), // Green-ish
        Frame::new_filled(320, 240, [100, 100, 255]), // Blue-ish
    ];

    println!("   Created {} test frames (320x240)", test_frames.len());

    // Apply VHS effects
    for (i, frame) in test_frames.iter_mut().enumerate() {
        match style.apply_effect(frame, &style_config) {
            Ok(()) => println!("   ✅ Applied VHS effect to frame {}", i + 1),
            Err(e) => println!("   ❌ Failed to apply effect to frame {}: {}", i + 1, e),
        }
    }

    // Save processed frames for inspection
    for (i, frame) in test_frames.iter().enumerate() {
        let filename = format!("test_frame_processed_{}.png", i + 1);
        match frame.save_png(&filename) {
            Ok(()) => println!("   📁 Saved processed frame: {}", filename),
            Err(e) => println!("   ⚠️  Could not save frame {}: {}", filename, e),
        }
    }

    // Test 4: Video Compositor
    println!("\n4. Testing Video Compositor...");
    let mut compositor = VideoCompositor::new(video_params);

    // Check if FFmpeg is available
    if VideoCompositor::check_ffmpeg_available() {
        println!("   ✅ FFmpeg is available");

        // Create a simple test video with fewer frames to avoid path issues
        let test_output = "simple_test_video.mp4";
        println!("   Creating simple test video: {}", test_output);

        match compositor.create_test_video(test_output, 1.0).await {
            Ok(encoded_video) => {
                println!("   ✅ Test video created successfully!");
                println!("     Path: {}", encoded_video.path);
                println!("     Duration: {:.1}s", encoded_video.duration);
                println!("     Frames: {}", encoded_video.frame_count);
                println!("     Size: {:.1} KB", encoded_video.file_size as f64 / 1024.0);
            }
            Err(e) => {
                println!("   ❌ Failed to create test video: {}", e);
                println!("   This might be due to FFmpeg path configuration");
                println!("   FFmpeg is available but may have path resolution issues");
            }
        }
    } else {
        println!("   ⚠️  FFmpeg not found in PATH");
        println!("   Install FFmpeg: brew install ffmpeg (macOS) or sudo apt install ffmpeg (Ubuntu)");
        println!("   This is expected if FFmpeg is not installed");
    }

    // Test 5: Integration Test (if we have test files)
    println!("\n5. Testing Integration (mock scenario)...");

    // Simulate a composition timeline
    use retro_compositor::composition::engine::CompositionTimeline;
    let mut timeline = CompositionTimeline::new();
    timeline.add_cut(0.0, 1);
    timeline.add_cut(1.5, 2);
    timeline.add_cut(3.0, 1);

    println!("   Created mock timeline with {} cuts", timeline.cuts.len());
    println!("   Cut points: {:?}", timeline.cuts);
    println!("   Clip assignments: {:?}", timeline.clip_assignments);

    // Test timeline operations
    let unique_clips = timeline.unique_clips();
    println!("   Unique clips used: {:?}", unique_clips);

    let segment_duration = timeline.segment_duration(0, 5.0);
    println!("   First segment duration: {:.1}s", segment_duration);

    // Test 6: Configuration
    println!("\n6. Testing Configuration Integration...");
    let app_config = Config::default();
    println!("   Video configuration:");
    println!("     Target FPS: {:.1}", app_config.video.params.fps);
    println!("     Resolution: {}x{}", app_config.video.params.resolution.0, app_config.video.params.resolution.1);
    println!("     Codec: {}", app_config.video.params.codec);
    println!("     Quality: {}", app_config.video.params.quality);
    println!("     Processing threads: {}", app_config.video.processing_threads);
    println!("     GPU acceleration: {}", app_config.video.gpu_acceleration);

    println!("   Composition configuration:");
    println!("     Beat sync strength: {:.1}", app_config.composition.beat_sync_strength);
    println!("     Min cut interval: {:.1}s", app_config.composition.min_cut_interval);
    println!("     Max cut interval: {:.1}s", app_config.composition.max_cut_interval);
    println!("     Crossfade duration: {:.1}s", app_config.composition.crossfade_duration);

    // Test 7: Memory and Performance
    println!("\n7. Testing Memory and Performance...");

    // Create a larger batch of frames to test performance
    let large_frame_count = 30; // 1 second at 30fps
    let mut large_frame_batch = Vec::new();

    for i in 0..large_frame_count {
        let hue = (i as f32 / large_frame_count as f32) * 360.0;
        let color = hsv_to_rgb(hue, 0.8, 0.9);
        large_frame_batch.push(Frame::new_filled(640, 480, color));
    }

    println!("   Created {} frames for performance test", large_frame_batch.len());

    let start_time = std::time::Instant::now();
    for frame in large_frame_batch.iter_mut() {
        let _ = style.apply_effect(frame, &style_config);
    }
    let processing_time = start_time.elapsed();

    println!("   Processed {} frames in {:.2}ms",
             large_frame_count, processing_time.as_millis());
    println!("   Average: {:.2}ms per frame",
             processing_time.as_millis() as f64 / large_frame_count as f64);

    println!("\n🎉 All video processing tests completed!");
    println!("📝 Ready for full integration with audio analysis!");

    // Cleanup
    compositor.cleanup().ok();

    Ok(())
}

/// Convert HSV to RGB color
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