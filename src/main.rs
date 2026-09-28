use std::env::consts::{ARCH, OS};

mod error;
mod command;
mod retry;

#[cfg(target_os = "windows")]
#[path = "platform/windows.rs"]
mod windows;

#[cfg(target_os = "linux")]
#[path = "platform/linux.rs"]
mod linux;

#[cfg(target_os = "macos")]
#[path = "platform/macos.rs"]
mod macos;

fn main() {
    println!();
    println!("================================");
    println!("        PUPPET-SSH");
    println!("================================");
    println!();

    println!("Operating system : {}", OS);
    println!("Architecture     : {}", ARCH);
    println!();

    #[cfg(target_os = "windows")]
    windows::run();

    #[cfg(target_os = "linux")]
    linux::run();

    #[cfg(target_os = "macos")]
    macos::run();

    #[cfg(not(any(
        target_os = "windows",
        target_os = "linux",
        target_os = "macos"
    )))]
    println!("This device is only the controller; no installer runs here.");

    println!();
}
