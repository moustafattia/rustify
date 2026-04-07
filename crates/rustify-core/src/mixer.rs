use std::sync::atomic::{AtomicU8, Ordering};

/// Lock-free volume control using atomic operations.
/// Volume ranges from 0 (silent) to 100 (full).
pub struct Mixer {
    volume: AtomicU8,
}

impl Mixer {
    /// Create a new mixer with the given initial volume (clamped to 0-100).
    pub fn new(initial_volume: u8) -> Self {
        Self {
            volume: AtomicU8::new(initial_volume.min(100)),
        }
    }

    /// Set the volume (clamped to 0-100).
    pub fn set_volume(&self, volume: u8) {
        self.volume.store(volume.min(100), Ordering::Relaxed);
    }

    /// Get the current volume (0-100).
    pub fn get_volume(&self) -> u8 {
        self.volume.load(Ordering::Relaxed)
    }

    /// Get the gain multiplier (0.0 - 1.0) for applying to audio samples.
    pub fn gain(&self) -> f32 {
        self.get_volume() as f32 / 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_volume() {
        let mixer = Mixer::new(75);
        assert_eq!(mixer.get_volume(), 75);
    }

    #[test]
    fn clamps_initial_volume_to_100() {
        let mixer = Mixer::new(150);
        assert_eq!(mixer.get_volume(), 100);
    }

    #[test]
    fn set_and_get_volume() {
        let mixer = Mixer::new(50);
        mixer.set_volume(80);
        assert_eq!(mixer.get_volume(), 80);
    }

    #[test]
    fn clamps_set_volume_to_100() {
        let mixer = Mixer::new(50);
        mixer.set_volume(200);
        assert_eq!(mixer.get_volume(), 100);
    }

    #[test]
    fn volume_zero() {
        let mixer = Mixer::new(0);
        assert_eq!(mixer.get_volume(), 0);
        assert_eq!(mixer.gain(), 0.0);
    }

    #[test]
    fn gain_at_full_volume() {
        let mixer = Mixer::new(100);
        assert!((mixer.gain() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn gain_at_half_volume() {
        let mixer = Mixer::new(50);
        assert!((mixer.gain() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn gain_at_zero_volume() {
        let mixer = Mixer::new(0);
        assert!((mixer.gain() - 0.0).abs() < f32::EPSILON);
    }
}
