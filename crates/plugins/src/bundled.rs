use std::fs;
use std::path::Path;

pub(crate) struct BundledPluginFile {
    pub relative_path: &'static str,
    pub content: &'static str,
    #[cfg_attr(not(unix), allow(dead_code))]
    pub executable: bool,
}

pub(crate) struct BundledPlugin {
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
    pub files: &'static [BundledPluginFile],
}

impl BundledPlugin {
    /// Write all embedded files to `target_dir`, creating subdirectories as needed.
    pub fn materialize(&self, target_dir: &Path) -> Result<(), std::io::Error> {
        fs::create_dir_all(target_dir)?;
        for file in self.files {
            let path = target_dir.join(file.relative_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, file.content)?;
            #[cfg(unix)]
            if file.executable {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&path, perms)?;
            }
        }
        Ok(())
    }
}

pub(crate) static BUNDLED_PLUGINS: &[BundledPlugin] = &[
    BundledPlugin {
        name: "example-bundled",
        version: "0.1.0",
        description: "Example bundled plugin scaffold for the Rust plugin system",
        files: &[
            BundledPluginFile {
                relative_path: "plugin.json",
                content: include_str!("../bundled/example-bundled/plugin.json"),
                executable: false,
            },
            BundledPluginFile {
                relative_path: "hooks/pre.sh",
                content: include_str!("../bundled/example-bundled/hooks/pre.sh"),
                executable: true,
            },
            BundledPluginFile {
                relative_path: "hooks/post.sh",
                content: include_str!("../bundled/example-bundled/hooks/post.sh"),
                executable: true,
            },
        ],
    },
    BundledPlugin {
        name: "sample-hooks",
        version: "0.1.0",
        description: "Bundled sample plugin scaffold for hook integration tests.",
        files: &[
            BundledPluginFile {
                relative_path: "plugin.json",
                content: include_str!("../bundled/sample-hooks/plugin.json"),
                executable: false,
            },
            BundledPluginFile {
                relative_path: "hooks/pre.sh",
                content: include_str!("../bundled/sample-hooks/hooks/pre.sh"),
                executable: true,
            },
            BundledPluginFile {
                relative_path: "hooks/post.sh",
                content: include_str!("../bundled/sample-hooks/hooks/post.sh"),
                executable: true,
            },
        ],
    },
];
