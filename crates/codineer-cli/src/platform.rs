use std::process::Command;
use std::sync::OnceLock;

pub(crate) fn vim_installed() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        #[cfg(target_os = "windows")]
        let which = "where";
        #[cfg(not(target_os = "windows"))]
        let which = "which";

        Command::new(which)
            .arg("vim")
            .output()
            .is_ok_and(|output| output.status.success())
    })
}
