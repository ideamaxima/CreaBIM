//! The web interface's own files (style, script, icons). They are built
//! into the program; with `CREABIM_UI_DIR` set to a folder holding
//! `app.css`, `app.js` and `icons.js`, they are read from there at start
//! instead, so a change to an icon or a style needs no build, only a new
//! start.

use std::borrow::Cow;

const CSS: &str = include_str!("../ui/app.css");
const JS: &str = include_str!("../ui/app.js");
pub const ICONS: &str = include_str!("../ui/icons.js");

/// `name` from `CREABIM_UI_DIR`, if it is set and the file reads; else
/// the built-in copy.
fn file(name: &str, built_in: &'static str) -> Cow<'static, str> {
    std::env::var_os("CREABIM_UI_DIR")
        .and_then(|dir| std::fs::read_to_string(std::path::Path::new(&dir).join(name)).ok())
        .map_or(Cow::Borrowed(built_in), Cow::Owned)
}

pub fn css() -> Cow<'static, str> {
    file("app.css", CSS)
}

pub fn js() -> Cow<'static, str> {
    file("app.js", JS)
}

pub fn icons() -> Cow<'static, str> {
    file("icons.js", ICONS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ui_files_come_from_the_folder_when_it_is_set() {
        let dir = std::env::temp_dir().join(format!("creabim-ui-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("icons.js"), "\"x\": \"<path />\",\n").unwrap();
        // only this test sets the variable
        std::env::set_var("CREABIM_UI_DIR", &dir);
        assert!(icons().contains("\"x\""));
        // a file missing from the folder falls back to the built-in one
        assert_eq!(css(), CSS);
        std::env::remove_var("CREABIM_UI_DIR");
        assert_eq!(icons(), ICONS);
        std::fs::remove_dir_all(&dir).ok();
    }
}
