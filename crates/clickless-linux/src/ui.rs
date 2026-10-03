//! Linux GTK4 UI prototyping for Home, Settings and safe Practice.

use clickless_config::Config;
use clickless_config::settings_model::SettingsPage;

// Prototype Settings Window using GTK4
pub fn show_settings(_config: &Config) -> Result<(), String> {
    // We would initialize gtk4::Application here and build the UI.
    // For now, this is a placeholder scaffold because we are cross-compiling.
    println!("Prototype: GTK4 initialization would happen here.");

    for page in SettingsPage::ALL {
        println!(
            "Prototype: would render GTK4 sidebar item for {}",
            page.title()
        );
    }

    Ok(())
}
