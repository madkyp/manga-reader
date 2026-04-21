// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
  // xdg-desktop-portal hangs on Wayland/Hyprland and freezes WebKit
  unsafe {
    std::env::set_var("GTK_USE_PORTAL", "0");
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
  }
  app_lib::run();
}
