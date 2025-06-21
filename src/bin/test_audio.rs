// Test binary for audio analysis functionality

use std::path::PathBuf;
use retro_compositor::{
    audio::{AudioLoader, AudioAnalyzer, types::AnalysisConfig},
    config::Config,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging with debug level to see analysis details
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    println!("🎵 Testing Retro-Compositor Audio Analysis");

    // Test 1: Create synthetic audio data
    println!("\n1. Creating synthetic audio data...");
    let test_audio = create_test_audio_file().await?;
    println!("   ✅ Created test audio: {:.1}s duration", test_audio.duration);

    // Test 2: Test audio analysis with different configurations
    println!("\n2. Testing audio analysis configurations...");

    // Fast analysis
    println!("   Testing fast analysis...");
    let mut fast_config = AnalysisConfig::fast();
    fast_config.beat_sensitivity = 0.5; // Lower threshold
    let fast_analyzer = AudioAnalyzer::with_config(fast_config);
    let fast_analysis = fast_analyzer.analyze(&test_audio).await?;
    println!("   ✅ Fast analysis: {:.1} BPM, {} beats",
             fast_analysis.bpm, fast_analysis.beats.len());

    // High quality analysis
    println!("   Testing high-quality analysis...");
    let mut hq_config = AnalysisConfig::high_quality();
    hq_config.beat_sensitivity = 0.5; // Lower threshold for better detection
    let hq_analyzer = AudioAnalyzer::with_config(hq_config);
    let hq_analysis = hq_analyzer.analyze(&test_audio).await?;
    println!("   ✅ High-quality analysis: {:.1} BPM, {} beats",
             hq_analysis.bpm, hq_analysis.beats.len());

    // Test 3: Detailed analysis inspection
    println!("\n3. Inspecting analysis results...");
    let analysis = &hq_analysis;

    println!("   Duration: {:.2}s", analysis.duration);
    println!("   BPM: {:.1} (confidence: {:.2})", analysis.bpm, analysis.bpm_confidence);
    println!("   Beats detected: {}", analysis.beats.len());
    println!("   Energy levels: {}", analysis.energy_levels.len());
    println!("   Phrases detected: {}", analysis.phrases.len());

    // Show first few beats
    println!("   First 5 beats:");
    for (i, beat) in analysis.beats.iter().take(5).enumerate() {
        println!("     Beat {}: {:.2}s (strength: {:.2}, type: {:?})",
                 i + 1, beat.time, beat.strength, beat.beat_type);
    }

    // Show energy distribution
    if !analysis.energy_levels.is_empty() {
        let avg_energy: f32 = analysis.energy_levels.iter().map(|e| e.rms).sum::<f32>()
            / analysis.energy_levels.len() as f32;
        let max_energy = analysis.energy_levels.iter()
            .map(|e| e.rms)
            .fold(0.0f32, f32::max);
        println!("   Energy levels - Average: {:.3}, Peak: {:.3}", avg_energy, max_energy);
    }

    // Show phrases
    if !analysis.phrases.is_empty() {
        println!("   Musical phrases:");
        for (i, phrase) in analysis.phrases.iter().enumerate() {
            println!("     Phrase {}: {:.1}s-{:.1}s ({:?}, confidence: {:.2})",
                     i + 1, phrase.start, phrase.end, phrase.phrase_type, phrase.confidence);
        }
    }

    // Test 4: Beat timing analysis
    println!("\n4. Analyzing beat timing...");
    if analysis.beats.len() >= 2 {
        let intervals: Vec<f64> = analysis.beats
            .windows(2)
            .map(|pair| pair[1].time - pair[0].time)
            .collect();

        let avg_interval = intervals.iter().sum::<f64>() / intervals.len() as f64;
        let calculated_bpm = 60.0 / avg_interval;

        println!("   Average beat interval: {:.3}s", avg_interval);
        println!("   Calculated BPM from intervals: {:.1}", calculated_bpm);
        println!("   Tempo consistency: {:.1}%",
                 (1.0 - intervals.iter()
                     .map(|&i| (i - avg_interval).abs())
                     .sum::<f64>() / (intervals.len() as f64 * avg_interval)) * 100.0);
    }

    // Test 5: Integration with composition engine
    println!("\n5. Testing integration with composition...");
    let config = Config::default();
    println!("   Config audio settings:");
    println!("     Sample rate: {} Hz", config.audio.sample_rate);
    println!("     Window size: {}", config.audio.window_size);
    println!("     BPM range: {:.0}-{:.0}", config.audio.min_bpm, config.audio.max_bpm);
    println!("     Beat sensitivity: {:.1}", config.audio.beat_sensitivity);

    // Test composition timeline generation
    let beats_for_timeline = &analysis.beats[..analysis.beats.len().min(10)];
    println!("   Timeline generation simulation:");
    println!("     Would generate cuts at: {:?}",
             beats_for_timeline.iter().map(|b| b.time).collect::<Vec<_>>());

    println!("\n🎉 All audio analysis tests completed successfully!");
    println!("📝 Ready for integration with video processing pipeline!");

    Ok(())
}

/// Create synthetic audio data for testing
async fn create_test_audio_file() -> Result<retro_compositor::audio::AudioData, Box<dyn std::error::Error>> {
    use retro_compositor::audio::types::{AudioData, AudioFormat};
    use std::f32::consts::PI;

    // Generate a 4-second test track with very clear, prominent beats
    let sample_rate = 44100u32;
    let duration = 4.0;
    let bpm = 120.0;
    let beat_interval = 60.0 / bpm;

    let num_samples = (sample_rate as f64 * duration) as usize;
    let mut samples = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;

        // Create a base tone (fundamental)
        let fundamental = (2.0 * PI * 220.0 * t).sin() * 0.2;

        // Add harmonic content for more interesting spectrum
        let harmonic1 = (2.0 * PI * 440.0 * t).sin() * 0.1;
        let harmonic2 = (2.0 * PI * 880.0 * t).sin() * 0.05;

        // Create very pronounced beats - strong attack every beat_interval
        let beat_phase = (t as f64 % beat_interval) / beat_interval;
        let beat_emphasis = if beat_phase < 0.05 { // 50ms attack
            // Very sharp, loud attack for clear onset detection
            let attack_envelope = 1.0 - (beat_phase as f32 * 20.0); // Sharp decay
            let beat_tone = (2.0 * PI * 80.0 * t).sin() * 0.6; // Low frequency punch
            let click = (2.0 * PI * 2000.0 * t).sin() * 0.3 * attack_envelope; // High freq click
            beat_tone * attack_envelope + click
        } else if beat_phase < 0.1 {
            // Sustain phase with some energy
            let sustain_envelope = 0.3 * (1.0 - ((beat_phase as f32 - 0.05) * 20.0));
            (2.0 * PI * 80.0 * t).sin() * sustain_envelope
        } else {
            0.0
        };

        // Add some controlled noise for realism
        let noise = (rand::random::<f32>() - 0.5) * 0.02;

        // Combine all elements
        let sample = fundamental + harmonic1 + harmonic2 + beat_emphasis + noise;
        samples.push(sample.clamp(-1.0, 1.0));
    }

    println!("   Generated {} samples with clear beats every {:.2}s",
             samples.len(), beat_interval);

    Ok(AudioData {
        samples,
        sample_rate,
        channels: 1,
        duration,
        file_path: PathBuf::from("synthetic_test.wav"),
        format: AudioFormat {
            extension: "wav".to_string(),
            bit_depth: Some(16),
            compression: None,
            bitrate: None,
        },
    })
}