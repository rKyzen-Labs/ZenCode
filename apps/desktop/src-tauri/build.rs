fn main() {
    let win_attrs = tauri_build::WindowsAttributes::new();
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(win_attrs))
        .expect("failed to build tauri");
}
