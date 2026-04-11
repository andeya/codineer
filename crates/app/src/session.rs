//! Session persistence — save/restore workspace state across restarts.

use aineer_ui::blocks::Block;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionData {
    pub version: u32,
    pub tabs: Vec<TabSession>,
    pub active_tab_index: usize,
    pub sidebar_visible: bool,
    pub sidebar_width: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TabSession {
    pub id: u64,
    pub title: String,
    pub working_dir: PathBuf,
    pub blocks: Vec<Block>,
    pub scroll_position: f64,
}

impl SessionData {
    pub fn new() -> Self {
        Self {
            version: 1,
            tabs: Vec::new(),
            active_tab_index: 0,
            sidebar_visible: true,
            sidebar_width: 280.0,
        }
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)
    }

    pub fn load(path: &std::path::Path) -> Result<Self, std::io::Error> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}

impl Default for SessionData {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");

        let session = SessionData {
            version: 1,
            tabs: vec![TabSession {
                id: 1,
                title: "Test Tab".into(),
                working_dir: PathBuf::from("/tmp"),
                blocks: vec![],
                scroll_position: 42.0,
            }],
            active_tab_index: 0,
            sidebar_visible: true,
            sidebar_width: 300.0,
        };

        session.save(&path).unwrap();
        let loaded = SessionData::load(&path).unwrap();

        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.tabs.len(), 1);
        assert_eq!(loaded.tabs[0].title, "Test Tab");
        assert_eq!(loaded.tabs[0].scroll_position, 42.0);
        assert_eq!(loaded.active_tab_index, 0);
        assert_eq!(loaded.sidebar_width, 300.0);
    }

    #[test]
    fn session_with_blocks_round_trip() {
        use aineer_ui::blocks::*;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session_blocks.json");

        let session = SessionData {
            version: 1,
            tabs: vec![TabSession {
                id: 1,
                title: "Block Tab".into(),
                working_dir: PathBuf::from("/home"),
                blocks: vec![
                    Block::System(SystemBlock {
                        meta: BlockMeta {
                            id: 1,
                            created_at: chrono::Utc::now(),
                            collapsed: false,
                            parent_id: None,
                            tags: vec![],
                        },
                        kind: SystemKind::Info,
                        message: "Hello world".into(),
                    }),
                    Block::Command(CommandBlock {
                        meta: BlockMeta {
                            id: 2,
                            created_at: chrono::Utc::now(),
                            collapsed: false,
                            parent_id: None,
                            tags: vec![],
                        },
                        command: "ls -la".into(),
                        cwd: PathBuf::from("/home"),
                        output_text: "total 0".into(),
                        exit_code: Some(0),
                        duration: Some(std::time::Duration::from_millis(15)),
                        ai_diagnosis: None,
                    }),
                ],
                scroll_position: 0.0,
            }],
            active_tab_index: 0,
            sidebar_visible: false,
            sidebar_width: 260.0,
        };

        session.save(&path).unwrap();
        let loaded = SessionData::load(&path).unwrap();

        assert_eq!(loaded.tabs[0].blocks.len(), 2);
        assert!(matches!(loaded.tabs[0].blocks[0], Block::System(_)));
        assert!(matches!(loaded.tabs[0].blocks[1], Block::Command(_)));
    }
}
