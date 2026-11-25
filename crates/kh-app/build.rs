fn main() {
    // The window macro needs the folder to exist, also for checks that never build the front end.
    if let Err(error) = std::fs::create_dir_all("../../ui/dist") {
        panic!("cannot create ui/dist: {error}");
    }
    tauri_build::build();
}
