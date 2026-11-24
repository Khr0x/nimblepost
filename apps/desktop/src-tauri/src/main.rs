#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(
    non_snake_case,
    reason = "The executable uses the NimblePost brand name."
)]

#[cfg(target_os = "linux")]
mod linux_renderer;

fn main() {
    #[cfg(target_os = "linux")]
    linux_renderer::configure_dmabuf();
    nimblepost_desktop::run();
}
