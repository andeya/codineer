use std::fs;
use std::path::{Path, PathBuf};

/// Content for `.codineer/.gitignore`.
/// Lines starting with `!` are negated (i.e. explicitly tracked).
const CODINEER_GITIGNORE: &str = "\
# Tracked (committed to the repo)
!CODINEER.md
!settings.json
!plugins/
!skills/

# Ignored (local / runtime artifacts)
settings.local.json
sessions/
agents/
sandbox-home/
sandbox-tmp/
todos.json
cache/
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InitStatus {
    Created,
    Skipped,
}

impl InitStatus {
    #[must_use]
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Skipped => "skipped (already exists)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InitArtifact {
    pub(crate) name: String,
    pub(crate) depth: u8,
    pub(crate) status: InitStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InitReport {
    pub(crate) project_root: PathBuf,
    pub(crate) artifacts: Vec<InitArtifact>,
}

impl InitReport {
    #[must_use]
    pub(crate) fn render(&self) -> String {
        let mut lines = vec![
            "Init".to_string(),
            format!("  Project          {}", self.project_root.display()),
        ];
        for artifact in &self.artifacts {
            let (prefix, width) = match artifact.depth {
                0 => ("  ", 16),
                _ => ("    ", 14),
            };
            lines.push(format!(
                "{prefix}{:<width$} {}",
                artifact.name,
                artifact.status.label()
            ));
        }
        lines.push("  Next step        Review and tailor the generated guidance".to_string());
        lines.join("\n")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RepoFeature {
    RustWorkspace,
    RustRoot,
    Python,
    PackageJson,
    TypeScript,
    NextJs,
    React,
    Vite,
    NestJs,
    SrcDir,
    TestsDir,
    RustDir,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RepoDetection {
    features: std::collections::HashSet<RepoFeature>,
}

impl RepoDetection {
    fn has(&self, feature: RepoFeature) -> bool {
        self.features.contains(&feature)
    }
}

pub(crate) fn initialize_repo(cwd: &Path) -> Result<InitReport, Box<dyn std::error::Error>> {
    let mut artifacts = ensure_codineer_scaffold(cwd)?;

    let content = render_init_codineer_md(cwd);
    artifacts.push(InitArtifact {
        name: "CODINEER.md".into(),
        depth: 1,
        status: write_file_if_missing(&cwd.join(".codineer").join("CODINEER.md"), &content)?,
    });

    Ok(InitReport {
        project_root: cwd.to_path_buf(),
        artifacts,
    })
}

const CODINEER_SUBDIRS: &[&str] = &["plugins", "skills", "agents", "sessions"];

const STARTER_SETTINGS_JSON: &str = concat!(
    "{\n",
    "  \"permissions\": {\n",
    "    \"defaultMode\": \"dontAsk\"\n",
    "  }\n",
    "}\n",
);

fn ensure_codineer_scaffold(root: &Path) -> Result<Vec<InitArtifact>, std::io::Error> {
    let cd = root.join(".codineer");
    let mut artifacts = Vec::new();

    artifacts.push(InitArtifact {
        name: ".codineer/".into(),
        depth: 0,
        status: ensure_dir(&cd)?,
    });
    for dir in CODINEER_SUBDIRS {
        artifacts.push(InitArtifact {
            name: format!("{dir}/"),
            depth: 1,
            status: ensure_dir(&cd.join(dir))?,
        });
    }
    artifacts.push(InitArtifact {
        name: "settings.json".into(),
        depth: 1,
        status: write_file_if_missing(&cd.join("settings.json"), STARTER_SETTINGS_JSON)?,
    });
    artifacts.push(InitArtifact {
        name: ".gitignore".into(),
        depth: 1,
        status: write_file_if_missing(&cd.join(".gitignore"), CODINEER_GITIGNORE)?,
    });

    Ok(artifacts)
}

pub(crate) fn ensure_home_codineer_dirs() {
    let Some(home) = runtime::home_dir() else {
        return;
    };
    let _ = ensure_codineer_scaffold(&home);
}

fn ensure_dir(path: &Path) -> Result<InitStatus, std::io::Error> {
    if path.is_dir() {
        return Ok(InitStatus::Skipped);
    }
    fs::create_dir_all(path)?;
    Ok(InitStatus::Created)
}

fn write_file_if_missing(path: &Path, content: &str) -> Result<InitStatus, std::io::Error> {
    if path.exists() {
        return Ok(InitStatus::Skipped);
    }
    fs::write(path, content)?;
    Ok(InitStatus::Created)
}

pub(crate) fn render_init_codineer_md(cwd: &Path) -> String {
    let detection = detect_repo(cwd);
    let mut lines = vec![
        "# CODINEER.md".to_string(),
        String::new(),
        "This file provides guidance to Codineer (codineer.dev) when working with code in this repository.".to_string(),
        String::new(),
    ];

    let detected_languages = detected_languages(&detection);
    let detected_frameworks = detected_frameworks(&detection);
    lines.push("## Detected stack".to_string());
    if detected_languages.is_empty() {
        lines.push("- No specific language markers were detected yet; document the primary language and verification commands once the project structure settles.".to_string());
    } else {
        lines.push(format!("- Languages: {}.", detected_languages.join(", ")));
    }
    if detected_frameworks.is_empty() {
        lines.push("- Frameworks: none detected from the supported starter markers.".to_string());
    } else {
        lines.push(format!(
            "- Frameworks/tooling markers: {}.",
            detected_frameworks.join(", ")
        ));
    }
    lines.push(String::new());

    let verification_lines = verification_lines(cwd, &detection);
    if !verification_lines.is_empty() {
        lines.push("## Verification".to_string());
        lines.extend(verification_lines);
        lines.push(String::new());
    }

    let structure_lines = repository_shape_lines(&detection);
    if !structure_lines.is_empty() {
        lines.push("## Repository shape".to_string());
        lines.extend(structure_lines);
        lines.push(String::new());
    }

    let framework_lines = framework_notes(&detection);
    if !framework_lines.is_empty() {
        lines.push("## Framework notes".to_string());
        lines.extend(framework_lines);
        lines.push(String::new());
    }

    lines.push("## Working agreement".to_string());
    lines.push("- Prefer small, reviewable changes and keep generated bootstrap files aligned with actual repo workflows.".to_string());
    lines.push("- Keep shared defaults in `.codineer/settings.json`; reserve `.codineer/settings.local.json` for machine-local overrides.".to_string());
    lines.push("- Do not overwrite existing `.codineer/CODINEER.md` content automatically; update it intentionally when repo workflows change.".to_string());
    lines.push(String::new());

    lines.join("\n")
}

fn detect_repo(cwd: &Path) -> RepoDetection {
    let package_json_contents = fs::read_to_string(cwd.join("package.json"))
        .unwrap_or_default()
        .to_ascii_lowercase();

    let mut features = std::collections::HashSet::new();
    let mut add_if = |cond: bool, feat: RepoFeature| {
        if cond {
            features.insert(feat);
        }
    };

    add_if(
        cwd.join("rust").join("Cargo.toml").is_file(),
        RepoFeature::RustWorkspace,
    );
    add_if(cwd.join("Cargo.toml").is_file(), RepoFeature::RustRoot);
    add_if(
        cwd.join("pyproject.toml").is_file()
            || cwd.join("requirements.txt").is_file()
            || cwd.join("setup.py").is_file(),
        RepoFeature::Python,
    );
    add_if(cwd.join("package.json").is_file(), RepoFeature::PackageJson);
    add_if(
        cwd.join("tsconfig.json").is_file() || package_json_contents.contains("typescript"),
        RepoFeature::TypeScript,
    );
    add_if(
        package_json_contents.contains("\"next\""),
        RepoFeature::NextJs,
    );
    add_if(
        package_json_contents.contains("\"react\""),
        RepoFeature::React,
    );
    add_if(
        package_json_contents.contains("\"vite\""),
        RepoFeature::Vite,
    );
    add_if(
        package_json_contents.contains("@nestjs"),
        RepoFeature::NestJs,
    );
    add_if(cwd.join("src").is_dir(), RepoFeature::SrcDir);
    add_if(cwd.join("tests").is_dir(), RepoFeature::TestsDir);
    add_if(cwd.join("rust").is_dir(), RepoFeature::RustDir);

    RepoDetection { features }
}

fn detected_languages(detection: &RepoDetection) -> Vec<&'static str> {
    let mut languages = Vec::new();
    if detection.has(RepoFeature::RustWorkspace) || detection.has(RepoFeature::RustRoot) {
        languages.push("Rust");
    }
    if detection.has(RepoFeature::Python) {
        languages.push("Python");
    }
    if detection.has(RepoFeature::TypeScript) {
        languages.push("TypeScript");
    } else if detection.has(RepoFeature::PackageJson) {
        languages.push("JavaScript/Node.js");
    }
    languages
}

fn detected_frameworks(detection: &RepoDetection) -> Vec<&'static str> {
    let mut frameworks = Vec::new();
    if detection.has(RepoFeature::NextJs) {
        frameworks.push("Next.js");
    }
    if detection.has(RepoFeature::React) {
        frameworks.push("React");
    }
    if detection.has(RepoFeature::Vite) {
        frameworks.push("Vite");
    }
    if detection.has(RepoFeature::NestJs) {
        frameworks.push("NestJS");
    }
    frameworks
}

fn verification_lines(cwd: &Path, detection: &RepoDetection) -> Vec<String> {
    let mut lines = Vec::new();
    if detection.has(RepoFeature::RustWorkspace) {
        lines.push("- Run Rust verification from `rust/`: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`".to_string());
    } else if detection.has(RepoFeature::RustRoot) {
        lines.push("- Run Rust verification from the repo root: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`".to_string());
    }
    if detection.has(RepoFeature::Python) {
        if cwd.join("pyproject.toml").is_file() {
            lines.push("- Run the Python project checks declared in `pyproject.toml` (for example: `pytest`, `ruff check`, and `mypy` when configured).".to_string());
        } else {
            lines.push(
                "- Run the repo's Python test/lint commands before shipping changes.".to_string(),
            );
        }
    }
    if detection.has(RepoFeature::PackageJson) {
        lines.push("- Run the JavaScript/TypeScript checks from `package.json` before shipping changes (`npm test`, `npm run lint`, `npm run build`, or the repo equivalent).".to_string());
    }
    if detection.has(RepoFeature::TestsDir) && detection.has(RepoFeature::SrcDir) {
        lines.push("- `src/` and `tests/` are both present; update both surfaces together when behavior changes.".to_string());
    }
    lines
}

fn repository_shape_lines(detection: &RepoDetection) -> Vec<String> {
    let mut lines = Vec::new();
    if detection.has(RepoFeature::RustDir) {
        lines.push(
            "- `rust/` contains the Rust workspace and active CLI/runtime implementation."
                .to_string(),
        );
    }
    if detection.has(RepoFeature::SrcDir) {
        lines.push("- `src/` contains source files that should stay consistent with generated guidance and tests.".to_string());
    }
    if detection.has(RepoFeature::TestsDir) {
        lines.push("- `tests/` contains validation surfaces that should be reviewed alongside code changes.".to_string());
    }
    lines
}

fn framework_notes(detection: &RepoDetection) -> Vec<String> {
    let mut lines = Vec::new();
    if detection.has(RepoFeature::NextJs) {
        lines.push("- Next.js detected: preserve routing/data-fetching conventions and verify production builds after changing app structure.".to_string());
    }
    if detection.has(RepoFeature::React) && !detection.has(RepoFeature::NextJs) {
        lines.push("- React detected: keep component behavior covered with focused tests and avoid unnecessary prop/API churn.".to_string());
    }
    if detection.has(RepoFeature::Vite) {
        lines.push("- Vite detected: validate the production bundle after changing build-sensitive configuration or imports.".to_string());
    }
    if detection.has(RepoFeature::NestJs) {
        lines.push("- NestJS detected: keep module/provider boundaries explicit and verify controller/service wiring after refactors.".to_string());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{initialize_repo, render_init_codineer_md};
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> std::path::PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!("codineer-init-{nanos}-{id}"))
    }

    #[test]
    fn initialize_repo_creates_expected_files_and_gitignore_entries() {
        let root = temp_dir();
        fs::create_dir_all(root.join("rust")).expect("create rust dir");
        fs::write(root.join("rust").join("Cargo.toml"), "[workspace]\n").expect("write cargo");

        let report = initialize_repo(&root).expect("init should succeed");
        let rendered = report.render();
        assert!(rendered.contains(".codineer/       created"));
        assert!(rendered.contains("  plugins/       created"));
        assert!(rendered.contains("  skills/        created"));
        assert!(rendered.contains("  agents/        created"));
        assert!(rendered.contains("  sessions/      created"));
        assert!(rendered.contains("  settings.json  created"));
        assert!(rendered.contains("  .gitignore     created"));
        assert!(rendered.contains("  CODINEER.md    created"));
        let cd = root.join(".codineer");
        assert!(cd.is_dir());
        assert!(cd.join("plugins").is_dir());
        assert!(cd.join("skills").is_dir());
        assert!(cd.join("agents").is_dir());
        assert!(cd.join("sessions").is_dir());
        assert!(cd.join("settings.json").is_file());
        assert!(cd.join(".gitignore").is_file());
        assert!(cd.join("CODINEER.md").is_file());
        assert_eq!(
            fs::read_to_string(cd.join("settings.json")).expect("read settings json"),
            concat!(
                "{\n",
                "  \"permissions\": {\n",
                "    \"defaultMode\": \"dontAsk\"\n",
                "  }\n",
                "}\n",
            )
        );
        let gitignore = fs::read_to_string(cd.join(".gitignore")).expect("read gitignore");
        assert!(gitignore.contains("!CODINEER.md"));
        assert!(gitignore.contains("!settings.json"));
        assert!(gitignore.contains("!plugins/"));
        assert!(gitignore.contains("!skills/"));
        assert!(gitignore.contains("settings.local.json"));
        assert!(gitignore.contains("sessions/"));
        assert!(gitignore.contains("agents/"));
        assert!(gitignore.contains("sandbox-home/"));
        assert!(gitignore.contains("sandbox-tmp/"));
        assert!(gitignore.contains("todos.json"));
        let codineer_md = fs::read_to_string(cd.join("CODINEER.md")).expect("read codineer md");
        assert!(codineer_md.contains("Languages: Rust."));
        assert!(codineer_md.contains("cargo clippy --workspace --all-targets -- -D warnings"));

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn initialize_repo_is_idempotent_and_preserves_existing_files() {
        let root = temp_dir();
        let cd = root.join(".codineer");
        fs::create_dir_all(&cd).expect("create .codineer dir");
        fs::write(cd.join("CODINEER.md"), "custom guidance\n").expect("write existing codineer md");

        let first = initialize_repo(&root).expect("first init should succeed");
        assert!(first
            .render()
            .contains("CODINEER.md    skipped (already exists)"));
        let second = initialize_repo(&root).expect("second init should succeed");
        let second_rendered = second.render();
        assert!(second_rendered.contains(".codineer/       skipped (already exists)"));
        assert!(second_rendered.contains("  plugins/       skipped (already exists)"));
        assert!(second_rendered.contains("  settings.json  skipped (already exists)"));
        assert!(second_rendered.contains("  .gitignore     skipped (already exists)"));
        assert!(second_rendered.contains("  CODINEER.md    skipped (already exists)"));
        assert_eq!(
            fs::read_to_string(cd.join("CODINEER.md")).expect("read existing codineer md"),
            "custom guidance\n"
        );

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }

    #[test]
    fn render_init_template_mentions_detected_python_and_nextjs_markers() {
        let root = temp_dir();
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("pyproject.toml"), "[project]\nname = \"demo\"\n")
            .expect("write pyproject");
        fs::write(
            root.join("package.json"),
            r#"{"dependencies":{"next":"14.0.0","react":"18.0.0"},"devDependencies":{"typescript":"5.0.0"}}"#,
        )
        .expect("write package json");

        let rendered = render_init_codineer_md(Path::new(&root));
        assert!(rendered.contains("Languages: Python, TypeScript."));
        assert!(rendered.contains("Frameworks/tooling markers: Next.js, React."));
        assert!(rendered.contains("pyproject.toml"));
        assert!(rendered.contains("Next.js detected"));

        fs::remove_dir_all(root).expect("cleanup temp dir");
    }
}
