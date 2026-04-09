use crossbeam::channel::Sender;

/// Start the MPRIS2 D-Bus service (Linux only).
/// On non-Linux platforms, this is a no-op.
#[cfg(target_os = "linux")]
pub fn start(_player: &rustify_core::Player, _event_tx: Sender<rustify_core::PlayerEvent>) {
    // Full MPRIS2 implementation requires Linux D-Bus
    // This will be implemented when testing on Linux/Pi
    eprintln!("rustify: MPRIS support not yet implemented");
}

#[cfg(not(target_os = "linux"))]
pub fn start(_player: &rustify_core::Player, _event_tx: Sender<rustify_core::PlayerEvent>) {
    // No-op on non-Linux platforms
}
