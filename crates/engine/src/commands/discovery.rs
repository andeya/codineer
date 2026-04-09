use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DefinitionSource {
    Project,
    User,
}

impl DefinitionSource {
    fn label(self) -> &'static str {
        match self {
            Self::Project => "Project (.aineer)",
            Self::User => "User (~/.aineer)",
        }
    }
}

pub(crate) trait ShadowableItem {
    fn name(&self) -> &str;
    fn source(&self) -> DefinitionSource;
    fn shadowed_by(&self) -> Option<DefinitionSource>;
    fn set_shadowed_by(&mut self, winner: DefinitionSource);
    fn detail_line(&self) -> String;
}

fn apply_shadowing<T: ShadowableItem>(items: &mut [T]) {
    let mut active_sources = BTreeMap::<String, DefinitionSource>::new();
    for item in items.iter_mut() {
        let key = item.name().to_ascii_lowercase();
        if let Some(existing) = active_sources.get(&key) {
            item.set_shadowed_by(*existing);
        } else {
            active_sources.insert(key, item.source());
        }
    }
}

fn render_shadowed_report<T: ShadowableItem>(
    title: &str,
    count_label: &str,
    items: &[T],
) -> String {
    if items.is_empty() {
        return format!("No {title} found.");
    }

    let total_active = items.iter().filter(|i| i.shadowed_by().is_none()).count();
    let mut lines = vec![
        title.to_string(),
        format!("  {total_active} {count_label}"),
        String::new(),
    ];

    for source in [DefinitionSource::Project, DefinitionSource::User] {
        let group = items
            .iter()
            .filter(|i| i.source() == source)
            .collect::<Vec<_>>();
        if group.is_empty() {
            continue;
        }

        lines.push(format!("{}:", source.label()));
        for item in group {
            let detail = item.detail_line();
            match item.shadowed_by() {
                Some(winner) => lines.push(format!("  (shadowed by {}) {detail}", winner.label())),
                None => lines.push(format!("  {detail}")),
            }
        }
        lines.push(String::new());
    }

    lines.join("\n").trim_end().to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentSummary {
    name: String,
    description: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<String>,
    source: DefinitionSource,
    shadowed_by: Option<DefinitionSource>,
}

impl ShadowableItem for AgentSummary {
    fn name(&self) -> &str {
        &self.name
    }
    fn source(&self) -> DefinitionSource {
        self.source
    }
    fn shadowed_by(&self) -> Option<DefinitionSource> {
        self.shadowed_by
    }
    fn set_shadowed_by(&mut self, winner: DefinitionSource) {
        self.shadowed_by = Some(winner);
    }
    fn detail_line(&self) -> String {
        let mut parts = vec![self.name.clone()];
        if let Some(description) = &self.description {
            parts.push(description.clone());
        }
        if let Some(model) = &self.model {
            parts.push(model.clone());
        }
        if let Some(reasoning) = &self.reasoning_effort {
            parts.push(reasoning.clone());
        }
        parts.join(" · ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkillSummary {
    name: String,
    description: Option<String>,
    root_path: PathBuf,
    source: DefinitionSource,
    shadowed_by: Option<DefinitionSource>,
}

impl ShadowableItem for SkillSummary {
    fn name(&self) -> &str {
        &self.name
    }
    fn source(&self) -> DefinitionSource {
        self.source
    }
    fn shadowed_by(&self) -> Option<DefinitionSource> {
        self.shadowed_by
    }
    fn set_shadowed_by(&mut self, winner: DefinitionSource) {
        self.shadowed_by = Some(winner);
    }
    fn detail_line(&self) -> String {
        let mut parts = vec![self.name.clone()];
        if let Some(description) = &self.description {
            parts.push(description.clone());
        }
        parts.push(format!("({})", self.root_path.display()));
        parts.join(" · ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkillRoot {
    pub(crate) source: DefinitionSource,
    pub(crate) path: PathBuf,
}

fn discover_definition_roots(cwd: &Path, leaf: &str) -> Vec<(DefinitionSource, PathBuf)> {
    let mut roots = Vec::new();

    for ancestor in cwd.ancestors() {
        push_unique_root(
            &mut roots,
            DefinitionSource::Project,
            ancestor.join(".aineer").join(leaf),
        );
    }

    if let Some(home) = crate::home_dir() {
        push_unique_root(
            &mut roots,
            DefinitionSource::User,
            home.join(".aineer").join(leaf),
        );
    }

    roots
}

fn discover_skill_roots(cwd: &Path) -> Vec<SkillRoot> {
    let mut roots = Vec::new();

    for ancestor in cwd.ancestors() {
        push_unique_skill_root(
            &mut roots,
            DefinitionSource::Project,
            ancestor.join(".aineer").join("skills"),
        );
    }

    if let Some(home) = crate::home_dir() {
        push_unique_skill_root(
            &mut roots,
            DefinitionSource::User,
            home.join(".aineer").join("skills"),
        );
    }

    roots
}

fn push_unique_root(
    roots: &mut Vec<(DefinitionSource, PathBuf)>,
    source: DefinitionSource,
    path: PathBuf,
) {
    if path.is_dir() && !roots.iter().any(|(_, existing)| existing == &path) {
        roots.push((source, path));
    }
}

fn push_unique_skill_root(roots: &mut Vec<SkillRoot>, source: DefinitionSource, path: PathBuf) {
    if path.is_dir() && !roots.iter().any(|existing| existing.path == path) {
        roots.push(SkillRoot { source, path });
    }
}

pub(crate) fn load_agents_from_roots(
    roots: &[(DefinitionSource, PathBuf)],
) -> std::io::Result<Vec<AgentSummary>> {
    let mut agents = Vec::new();

    for (source, root) in roots {
        let mut root_agents = Vec::new();
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            if entry.path().extension().is_none_or(|ext| ext != "toml") {
                continue;
            }
            let contents = fs::read_to_string(entry.path())?;
            let fallback_name = entry.path().file_stem().map_or_else(
                || entry.file_name().to_string_lossy().to_string(),
                |stem| stem.to_string_lossy().to_string(),
            );
            root_agents.push(AgentSummary {
                name: parse_toml_string(&contents, "name").unwrap_or(fallback_name),
                description: parse_toml_string(&contents, "description"),
                model: parse_toml_string(&contents, "model"),
                reasoning_effort: parse_toml_string(&contents, "model_reasoning_effort"),
                source: *source,
                shadowed_by: None,
            });
        }
        root_agents.sort_by(|left, right| left.name.cmp(&right.name));
        agents.extend(root_agents);
    }

    apply_shadowing(&mut agents);
    Ok(agents)
}

pub(crate) fn load_skills_from_roots(roots: &[SkillRoot]) -> std::io::Result<Vec<SkillSummary>> {
    let mut skills = Vec::new();

    for root in roots {
        let mut root_skills = Vec::new();
        for entry in fs::read_dir(&root.path)? {
            let entry = entry?;
            if !entry.path().is_dir() {
                continue;
            }
            let skill_path = entry.path().join("SKILL.md");
            if !skill_path.is_file() {
                continue;
            }
            let contents = fs::read_to_string(skill_path)?;
            let (name, description) = parse_skill_frontmatter(&contents);
            root_skills.push(SkillSummary {
                name: name.unwrap_or_else(|| entry.file_name().to_string_lossy().to_string()),
                description,
                root_path: root.path.clone(),
                source: root.source,
                shadowed_by: None,
            });
        }
        root_skills.sort_by(|left, right| left.name.cmp(&right.name));
        skills.extend(root_skills);
    }

    apply_shadowing(&mut skills);
    Ok(skills)
}

fn parse_toml_string(contents: &str, key: &str) -> Option<String> {
    let prefix = format!("{key} =");
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        let Some(value) = trimmed.strip_prefix(&prefix) else {
            continue;
        };
        let value = value.trim();
        let Some(value) = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
        else {
            continue;
        };
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

pub(crate) fn parse_skill_frontmatter(contents: &str) -> (Option<String>, Option<String>) {
    let mut lines = contents.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, None);
    }

    let mut name = None;
    let mut description = None;
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("name:") {
            let value = unquote_frontmatter_value(value.trim());
            if !value.is_empty() {
                name = Some(value);
            }
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("description:") {
            let value = unquote_frontmatter_value(value.trim());
            if !value.is_empty() {
                description = Some(value);
            }
        }
    }

    (name, description)
}

fn unquote_frontmatter_value(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|trimmed| trimmed.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|trimmed| trimmed.strip_suffix('\''))
        })
        .unwrap_or(value)
        .trim()
        .to_string()
}

pub(crate) fn render_agents_report(agents: &[AgentSummary]) -> String {
    render_shadowed_report("Agents", "active agents", agents)
}

pub(crate) fn render_skills_report(skills: &[SkillSummary]) -> String {
    render_shadowed_report("Skills", "available skills", skills)
}

pub(crate) fn normalize_optional_args(args: Option<&str>) -> Option<&str> {
    args.map(str::trim).filter(|value| !value.is_empty())
}

fn render_agents_usage(unexpected: Option<&str>) -> String {
    let mut lines = vec![
        "Agents".to_string(),
        "  Usage            /agents".to_string(),
        "  Direct CLI       aineer agents".to_string(),
        "  Sources          .aineer/agents, ~/.aineer/agents".to_string(),
    ];
    if let Some(args) = unexpected {
        lines.push(format!("  Unexpected       {args}"));
    }
    lines.join("\n")
}

fn render_skills_usage(unexpected: Option<&str>) -> String {
    let mut lines = vec![
        "Skills".to_string(),
        "  Usage            /skills".to_string(),
        "  Direct CLI       aineer skills".to_string(),
        "  Sources          .aineer/skills, ~/.aineer/skills".to_string(),
    ];
    if let Some(args) = unexpected {
        lines.push(format!("  Unexpected       {args}"));
    }
    lines.join("\n")
}

pub fn handle_agents_slash_command(args: Option<&str>, cwd: &Path) -> std::io::Result<String> {
    match normalize_optional_args(args) {
        None | Some("list") => {
            let roots = discover_definition_roots(cwd, "agents");
            let agents = load_agents_from_roots(&roots)?;
            Ok(render_agents_report(&agents))
        }
        Some("-h" | "--help" | "help") => Ok(render_agents_usage(None)),
        Some(args) => Ok(render_agents_usage(Some(args))),
    }
}

pub fn handle_skills_slash_command(args: Option<&str>, cwd: &Path) -> std::io::Result<String> {
    match normalize_optional_args(args) {
        None | Some("list") => {
            let roots = discover_skill_roots(cwd);
            let skills = load_skills_from_roots(&roots)?;
            Ok(render_skills_report(&skills))
        }
        Some("-h" | "--help" | "help") => Ok(render_skills_usage(None)),
        Some(args) => Ok(render_skills_usage(Some(args))),
    }
}
