/// Detect whether the text is primarily Chinese.
pub fn detect_language(text: &str) -> &'static str {
    let total = text.len();
    if total == 0 {
        return "en";
    }
    let cjk_count = text
        .chars()
        .filter(|c| ('\u{4e00}'..='\u{9fff}').contains(c))
        .count();
    if cjk_count as f64 > 0.1 * text.chars().count() as f64 {
        "cn"
    } else {
        "en"
    }
}

pub fn build_tool_prompt(tools_json: &str, lang: &str, force_use: bool) -> String {
    let example = if lang == "cn" {
        TOOL_EXAMPLE_CN
    } else {
        TOOL_EXAMPLE_EN
    };
    let force_hint = if force_use {
        if lang == "cn" {
            "\n\n重要：你必须使用上述工具之一来回答，不要直接用纯文本回复。"
        } else {
            "\n\nIMPORTANT: You MUST use one of the available tools to respond. Do NOT reply with plain text."
        }
    } else {
        ""
    };

    if lang == "cn" {
        format!(
            "你有以下工具可用。当需要使用工具时，请仅回复一个 `tool_json` 代码块，不要包含其他文字。\n\n\
             可用工具：\n{tools_json}\n\n\
             示例：\n{example}\n\n\
             如果需要使用工具，请只输出一个 ```tool_json 代码块。否则直接回答。{force_hint}"
        )
    } else {
        format!(
            "You have the following tools available. When you need to use a tool, reply ONLY with a `tool_json` code block, no other text.\n\n\
             Available tools:\n{tools_json}\n\n\
             Example:\n{example}\n\n\
             If you need to use a tool, output exactly ONE ```tool_json code block. Otherwise answer directly.{force_hint}"
        )
    }
}

const TOOL_EXAMPLE_EN: &str = r#"```tool_json
{"tool":"plus_one","parameters":{"number":"5"}}
```"#;

const TOOL_EXAMPLE_CN: &str = r#"```tool_json
{"tool":"plus_one","parameters":{"number":"5"}}
```"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("Hello world"), "en");
        assert_eq!(detect_language("你好世界"), "cn");
        assert_eq!(detect_language(""), "en");
        assert_eq!(detect_language("abc你"), "cn");
        assert_eq!(
            detect_language("This is a long English sentence with one 字"),
            "en"
        );
    }
}
