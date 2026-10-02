#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(
    non_snake_case,
    reason = "The executable uses the NimblePost brand name."
)]

fn main() {
    nimblepost_desktop::run();
}
