use crate::ui::slash::{SlashCtx, write_error, write_ok};

pub(crate) const AGENTS_CREATION_PROMPT: &str = "\
Create an AGENTS.md file for this project. Read existing AGENTS.md or CLAUDE.md files \
in parent directories, README.md, and any config files to understand the project first. \
Then write a comprehensive AGENTS.md that documents: \
1) the overall purpose and architecture \
2) build/test/lint commands \
3) coding style and conventions \
4) directory layout \
Keep it focused and actionable for a coding agent.";

pub async fn handle(parts: &[&str], ctx: &mut SlashCtx<'_>) -> anyhow::Result<()> {
    let force = parts.len() >= 2 && parts[1] == "force";

    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let agents_path = cwd.join("AGENTS.md");

    if !force && agents_path.exists() {
        write_error(
            ctx.renderer,
            "AGENTS.md already exists. Use /init force to overwrite.",
        );
        return Ok(());
    }

    // Check if "code" prompt exists
    if !ctx.context.prompts.contains_key("code") {
        write_error(
            ctx.renderer,
            "no 'code' prompt found. Run /regen-prompts first.",
        );
        return Ok(());
    }

    write_ok(ctx.renderer, "delegating AGENTS.md creation to agent...");
    Err(anyhow::anyhow!("DEFER_INIT:{}", AGENTS_CREATION_PROMPT))
}

#[cfg(test)]
mod tests {
    use super::AGENTS_CREATION_PROMPT;

    #[test]
    fn test_prompt_is_non_empty() {
        assert!(!AGENTS_CREATION_PROMPT.is_empty());
    }

    #[test]
    fn test_prompt_contains_key_phrases() {
        assert!(AGENTS_CREATION_PROMPT.contains("AGENTS.md"));
        assert!(AGENTS_CREATION_PROMPT.contains("build"));
        assert!(AGENTS_CREATION_PROMPT.contains("test"));
        assert!(AGENTS_CREATION_PROMPT.contains("coding agent"));
    }

    #[test]
    fn test_prompt_starts_with_create() {
        assert!(AGENTS_CREATION_PROMPT.starts_with("Create an AGENTS.md"));
    }
}
