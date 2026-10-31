use kde_assistant_lib::backend::{
    audio_capture::{convert_to_mono, resample_to_16k},
    voice_coordinator::{pcm_rms, validate_model},
};
fn signal(rate: u32, hz: f32) -> Vec<f32> {
    (0..rate)
        .map(|i| (2.0 * std::f32::consts::PI * hz * i as f32 / rate as f32).sin())
        .collect()
}
fn rms(audio: &[f32]) -> f32 {
    (audio.iter().map(|x| x * x).sum::<f32>() / audio.len() as f32).sqrt()
}
#[test]
fn resampling_preserves_duration_and_speech_band_at_16_44_and_48k() {
    for rate in [16000, 44100, 48000] {
        let result = resample_to_16k(&signal(rate, 1000.0), rate);
        assert_eq!(result.len(), 16000);
        assert!((rms(&result) - 0.707).abs() < 0.02);
        let reference = signal(16000, 1000.0);
        let error: Vec<f32> = result
            .iter()
            .zip(reference)
            .skip(100)
            .take(15800)
            .map(|(a, b)| a - b)
            .collect();
        assert!(rms(&error) < 0.02);
    }
}
#[test]
fn anti_alias_filter_rejects_12khz_before_decimation() {
    for rate in [44100, 48000] {
        let result = resample_to_16k(&signal(rate, 12000.0), rate);
        assert!(rms(&result[100..15900]) < 0.01);
    }
}
#[test]
fn invalid_input_silence_and_channel_conversion() {
    assert!(resample_to_16k(&[1.0], 0).is_empty());
    assert!(resample_to_16k(&[f32::NAN], 48000).is_empty());
    assert!(resample_to_16k(&[], 48000).is_empty());
    assert_eq!(convert_to_mono(&[32767i16, -32767], 2), vec![0.0]);
    assert!(convert_to_mono(&[1i16], 0).is_empty());
    assert_eq!(pcm_rms(&[0; 3200]), 0.0);
    assert_eq!(pcm_rms(&[]), 0.0);
}
#[test]
fn missing_and_corrupt_models_fail_before_loading() {
    assert!(validate_model(std::path::Path::new("/nonexistent/f4-model")).is_err());
    let path = std::env::temp_dir().join(format!("f4-model-{}", uuid::Uuid::new_v4()));
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&path)
        .unwrap();
    file.set_len(1024 * 1024).unwrap();
    assert!(validate_model(&path)
        .unwrap_err()
        .to_string()
        .contains("Formato"));
    std::fs::remove_file(path).unwrap();
}
