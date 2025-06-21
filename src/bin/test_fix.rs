// Quick test for video creation with the fixes

use retro_compositor::{
    video::{VideoCompositor, VideoParams},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎬 Testing Fixed Video Creation");

    // Set up logging to see debug info
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Create compositor with smaller resolution for testing
    let params = VideoParams {
        fps: 30.0,
        resolution: (640, 480),
        codec: "h264".to_string(),
        quality: 75, // Lower quality for faster encoding
    };

    let mut compositor = VideoCompositor::new(params);

    // Check FFmpeg
    if !VideoCompositor::check_ffmpeg_available() {
        println!("❌ FFmpeg not available. Install with:");
        println!("   macOS: brew install ffmpeg");
        println!("   Ubuntu: sudo apt install ffmpeg");
        return Ok(());
    }

    println!("✅ FFmpeg available");

    // Create a short test video (1 second = 30 frames)
    println!("Creating test video...");

    match compositor.create_test_video("test_fixed.mp4", 1.0).await {
        Ok(video) => {
            println!("🎉 SUCCESS!");
            println!("   File: {}", video.path);
            println!("   Duration: {:.1}s", video.duration);
            println!("   Frames: {}", video.frame_count);
            println!("   Size: {:.1} KB", video.file_size as f64 / 1024.0);

            // Clean up
            compositor.cleanup()?;

            println!("✅ Video creation fixed! Ready for full composition.");
        }
        Err(e) => {
            println!("❌ Still having issues: {}", e);
            compositor.cleanup().ok();
        }
    }

    Ok(())
}