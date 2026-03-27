use std::fs;
use std::path::{Path, PathBuf};

const STARTER_CODINEER_JSON: &str = concat!(
    "{\n",
    "  \"permissions\": {\n",
    "    \"defaultMode\": \"dontAsk\"\n",
    "  }\n",
    "}\n",
);
const GITIGNORE_COMMENT: &str = "# Codineer local artifacts";
const GITIGNORE_ENTRIES: [&str; 2] = [".codineer/settings.local.json", ".codineer/sessions/"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InitStatus {
    Created,
    Updated,
    Skipped,
}

impl InitStatus {
    #[must_use]
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Updated => "updated",
            Self::Skipped => "skipped (already exists)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InitArtifact {
    pub(crate) name: &'static str,
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
            lines.push(format!(
                "  {:<16} {}",
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
    let mut artifacts = Vec::new();

    let config_dir = cwd.join(".codineer");
    artifacts.push(InitArtifact {
        name: ".codineer/",
        status: ensure_dir(&config_dir)?,
    });

    let config_json = cwd.join(".codineer.json");
    artifacts.push(InitArtifact {
        name: ".codineer.json",
        status: write_file_if_missing(&config_json, STARTER_CODINEER_JSON)?,
    });

    let gitignore = cwd.join(".gitignore");
    artifacts.push(InitArtifact {
        name: ".gitignore",
        status: ensure_gitignore_entries(&gitignore)?,
    });

    let codineer_md = cwd.join("CODINEER.md");
    let content = render_init_codineer_md(cwd);
    artifacts.push(InitArtifact {
        name: "CODINEER.md",
        status: write_file_if_missing(&codineer_md, &content)?,
    });

    Ok(InitReport {
        project_root: cwd.to_path_buf(),
        artifacts,
    })
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

fn ensure_gitignore_entries(path: &Path) -> Result<InitStatus, std::io::Error> {
    if !path.exists() {
        let mut lines = vec![GITIGNORE_COMMENT.to_string()];
        lines.extend(GITIGNORE_ENTRIES.iter().map(|entry| (*entry).to_string()));
        fs::write(path, format!("{}\n", lines.join("\n")))?;
        return Ok(InitStatus::Created);
    }

    let existing = fs::read_to_string(path)?;
    let mut lines = existing.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    let mut changed = false;

    if !lines.iter().any(|line| line == GITIGNORE_COMMENT) {
        lines.push(GITIGNORE_COMMENT.to_string());
        changed = true;
    }

    for entry in GITIGNORE_ENTRIES {
        if !lines.iter().any(|line| line == entry) {
            lines.push(entry.to_string());
            changed = true;
        }
    }

    if !changed {
        return Ok(InitStatus::Skipped);
    }

    fs::write(path, format!("{}\n", lines.join("\n")))?;
    Ok(InitStatus::Updated)
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
    lines.push("- Keep shared defaults in `.codineer.json`; reserve `.codineer/settings.local.json` for machine-local overrides.".to_string());
    lines.push("- Do not overwrite existing `CODINEER.md` content automatically; update it intentionally when repo workflows change.".to_string());
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

