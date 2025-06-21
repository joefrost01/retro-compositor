// Minimal test to verify the core functionality works

use retro_compositor::{
    styles::{StyleRegistry, StyleConfig},
    video::Frame,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎬 Testing Retro-Compositor Core Functionality");

    // Test 1: Style Registry
    println!("\n1. Testing Style Registry...");
    let registry = StyleRegistry::new();
    let available = registry.available_styles();
    println!("   Available styles: {:?}", available);
    assert_eq!(available.len(), 4);

    // Test 2: VHS Style Creation
    println!("\n2. Testing VHS Style...");
    let vhs_style = registry.get_style("vhs")
        .ok_or("VHS style not found")?;

    println!("   Style name: {}", vhs_style.name());
    println!("   Description: {}", vhs_style.description());

    // Test 3: Frame Creation
    println!("\n3. Testing Frame Creation...");
    let mut frame = Frame::new_filled(200, 150, [100, 150, 200]);
    println!("   Created frame: {}x{}", frame.width(), frame.height());

    // Test 4: Configuration
    println!("\n4. Testing Configuration...");
    let config = StyleConfig::default()
        .set("scanline_intensity", 0.9)
        .set("color_bleeding", 0.7)
        .set("noise_level", 0.4);

    println!("   Config intensity: {}", config.intensity);
    println!("   Scanline intensity: {:?}", config.get_f32("scanline_intensity"));

    // Test 5: Apply VHS Effect
    println!("\n5. Testing VHS Effect Application...");
    let result = vhs_style.apply_effect(&mut frame, &config);

    match result {
        Ok(()) => {
            println!("   ✅ VHS effect applied successfully!");

            // Save the result
            match frame.save_png("minimal_test_output.png") {
                Ok(()) => println!("   📁 Output saved to: minimal_test_output.png"),
                Err(e) => println!("   ⚠️  Could not save file: {}", e),
            }
        },
        Err(e) => {
            println!("   ❌ VHS effect failed: {}", e);
            return Err(e.into());
        }
    }

    // Test 6: Style Metadata
    println!("\n6. Testing Style Metadata...");
    let metadata = vhs_style.metadata();
    println!("   Performance impact: {}", metadata.performance_impact);
    println!("   GPU accelerated: {}", metadata.gpu_accelerated);
    println!("   Parameters: {}", metadata.optional_parameters.len());

    println!("\n🎉 All tests passed! Retro-Compositor core is working.");
    println!("📝 Next steps: Implement audio analysis and video file loading.");

    Ok(())
}