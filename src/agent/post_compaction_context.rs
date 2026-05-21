use std::path::Path;
use std::sync::Arc;

const MAX_CONTEXT_CHARS: usize = 3000;

pub struct PostCompactionContext {
    sections: Vec<String>,
    workspace_dir: Arc<Path>,
}

impl PostCompactionContext {
    pub fn new(sections: Vec<String>, workspace_dir: Arc<Path>) -> Self {
        Self {
            sections,
            workspace_dir,
        }
    }

    pub async fn generate(&self) -> Option<String> {
        if self.sections.is_empty() {
            return None;
        }

        let agents_path = self.workspace_dir.join("AGENTS.md");
        let content = match tokio::fs::read_to_string(&agents_path).await {
            Ok(c) => c,
            Err(_) => return None,
        };

        let sections = extract_sections(&content, &self.sections);
        if sections.is_empty() {
            return None;
        }

        let combined = sections.join("\n\n");
        let safe_content = if combined.len() > MAX_CONTEXT_CHARS {
            format!("{}...[truncated]...", &combined[..MAX_CONTEXT_CHARS])
        } else {
            combined
        };

        let prose = if self.sections == vec!["Session Startup".to_string(), "Red Lines".to_string()]
        {
            "Session was just compacted. The conversation summary above is a hint, NOT a substitute for your startup sequence. Run your Session Startup sequence — read the required files before responding to the user."
        } else {
            "Session was just compacted. The conversation summary above is a hint, NOT a substitute for your full startup sequence. Re-read the sections injected below and follow your configured startup procedure before responding to the user."
        };

        let section_label = format!(
            "Critical rules from AGENTS.md ({}):",
            self.sections.join(", ")
        );

        Some(format!(
            "[Post-compaction context refresh]\n\n{}\n\n{}\n\n{}",
            prose, section_label, safe_content
        ))
    }
}

fn extract_sections(content: &str, section_names: &[String]) -> Vec<String> {
    let mut results = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    let mut in_section = false;
    let mut current_section_name: Option<String> = None;
    let mut current_section_content: Vec<String> = Vec::new();

    for &line in &lines {
        if line.starts_with("## ") || line.starts_with("### ") {
            let heading = line.trim_start_matches('#').trim();

            if in_section {
                if let Some(ref name) = current_section_name {
                    if section_names.iter().any(|s| s.eq_ignore_ascii_case(name)) {
                        let content_str = current_section_content.join("\n");
                        if !content_str.is_empty() {
                            results.push(format!("### {}\n{}", name, content_str));
                        }
                    }
                }
                current_section_content.clear();
            }

            if section_names
                .iter()
                .any(|s| s.eq_ignore_ascii_case(heading))
            {
                in_section = true;
                current_section_name = Some(heading.to_string());
            } else {
                in_section = false;
                current_section_name = None;
            }
        } else if in_section {
            current_section_content.push(line.to_string());
        }
    }

    if in_section {
        if let Some(ref name) = current_section_name {
            if section_names.iter().any(|s| s.eq_ignore_ascii_case(name)) {
                let content_str = current_section_content.join("\n");
                if !content_str.is_empty() {
                    results.push(format!("### {}\n{}", name, content_str));
                }
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_sections() {
        let content = r#"# AGENTS.md

Some intro text.

## Session Startup

Read the config file first.

## Red Lines

Never delete files.

## Other Section

Some other content.
"#;

        let sections = extract_sections(
            content,
            &["Session Startup".to_string(), "Red Lines".to_string()],
        );

        assert!(sections.len() == 2);
        assert!(sections[0].contains("Session Startup"));
        assert!(sections[0].contains("Read the config file"));
        assert!(sections[1].contains("Red Lines"));
        assert!(sections[1].contains("Never delete"));
    }

    #[test]
    fn test_extract_sections_case_insensitive() {
        let content = r#"## session startup

Read the config.
"#;

        let sections = extract_sections(content, &["Session Startup".to_string()]);

        assert!(!sections.is_empty());
        assert!(sections[0].contains("session startup"));
    }

    #[test]
    fn test_empty_sections_returns_none() {
        let content = r#"## Other

Some content.
"#;

        let sections = extract_sections(
            content,
            &["Session Startup".to_string(), "Red Lines".to_string()],
        );

        assert!(sections.is_empty());
    }
}
