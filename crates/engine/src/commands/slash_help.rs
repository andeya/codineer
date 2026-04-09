use super::slash_spec::{slash_command_specs, SlashCommandCategory, SlashCommandSpec};

#[must_use]
pub fn resume_supported_slash_commands() -> Vec<&'static SlashCommandSpec> {
    slash_command_specs()
        .iter()
        .filter(|spec| spec.resume_supported)
        .collect()
}

#[must_use]
pub fn render_slash_command_help() -> String {
    let mut lines = vec![
        "Slash commands".to_string(),
        "  Tab completes commands inside the REPL.".to_string(),
        "  [resume] = also available via aineer --resume SESSION.json".to_string(),
    ];

    for category in [
        SlashCommandCategory::Core,
        SlashCommandCategory::Workspace,
        SlashCommandCategory::Session,
        SlashCommandCategory::Git,
        SlashCommandCategory::Automation,
    ] {
        lines.push(String::new());
        lines.push(category.title().to_string());
        lines.extend(
            slash_command_specs()
                .iter()
                .filter(|spec| spec.category == category)
                .map(render_slash_command_entry),
        );
    }

    lines.join("\n")
}

fn render_slash_command_entry(spec: &SlashCommandSpec) -> String {
    let alias_suffix = if spec.aliases.is_empty() {
        String::new()
    } else {
        format!(
            " (aliases: {})",
            spec.aliases
                .iter()
                .map(|alias| format!("/{alias}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let resume = if spec.resume_supported {
        " [resume]"
    } else {
        ""
    };
    format!(
        "  {name:<46} {}{alias_suffix}{resume}",
        spec.summary,
        name = render_slash_command_name(spec),
    )
}

fn render_slash_command_name(spec: &SlashCommandSpec) -> String {
    match spec.argument_hint {
        Some(argument_hint) => format!("/{} {}", spec.name, argument_hint),
        None => format!("/{}", spec.name),
    }
}

fn levenshtein_distance(left: &str, right: &str) -> usize {
    if left == right {
        return 0;
    }
    if left.is_empty() {
        return right.chars().count();
    }
    if right.is_empty() {
        return left.chars().count();
    }

    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    let mut current = vec![0; right_chars.len() + 1];

    for (left_index, left_char) in left.chars().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_char) in right_chars.iter().enumerate() {
            let cost = usize::from(left_char != *right_char);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right_chars.len()]
}

#[must_use]
pub fn suggest_slash_commands(input: &str, limit: usize) -> Vec<String> {
    let normalized = input.trim().trim_start_matches('/').to_ascii_lowercase();
    if normalized.is_empty() || limit == 0 {
        return Vec::new();
    }

    let mut ranked = slash_command_specs()
        .iter()
        .filter_map(|spec| {
            let score = std::iter::once(spec.name)
                .chain(spec.aliases.iter().copied())
                .map(str::to_ascii_lowercase)
                .filter_map(|alias| {
                    if alias == normalized {
                        Some((0_usize, alias.len()))
                    } else if alias.starts_with(&normalized) {
                        Some((1, alias.len()))
                    } else if alias.contains(&normalized) {
                        Some((2, alias.len()))
                    } else {
                        let distance = levenshtein_distance(&alias, &normalized);
                        (distance <= 2).then_some((3 + distance, alias.len()))
                    }
                })
                .min();

            score.map(|(bucket, len)| (bucket, len, render_slash_command_name(spec)))
        })
        .collect::<Vec<_>>();

    ranked.sort();
    ranked.dedup_by(|left, right| left.2 == right.2);
    ranked
        .into_iter()
        .take(limit)
        .map(|(_, _, display)| display)
        .collect()
}
