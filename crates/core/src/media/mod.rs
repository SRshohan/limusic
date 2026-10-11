//! Stream resolution + proxying (openspec/changes/http-audio-streaming). Everything needed to go
//! from a videoId to playable bytes over HTTP, Tauri-free: [`orchestrator`] ties the cipher/
//! PoToken/client-fallback pipeline together, [`audio_proxy`]/[`video_proxy`] serve the resolved
//! stream through a throttling-safe range proxy.

pub mod audio_proxy;
pub mod orchestrator;
pub mod video_proxy;
