fn main() {
    let result = clickless_config::default_config_path()
        .map_err(|error| error.to_string())
        .and_then(|path| {
            let config = if path.exists() {
                clickless_config::Config::load_from_file(&path)
                    .map_err(|error| error.to_string())?
            } else {
                clickless_config::Config::default()
            };
            clickless_ui::run(config, path, None)
        });
    if let Err(error) = result {
        eprintln!("Settings failed: {error}");
        std::process::exit(1);
    }
}
