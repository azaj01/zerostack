use std::path::PathBuf;

use crate::ui::slash::{SlashCtx, write_error, write_ok, write_result};

pub(crate) fn resolve_path(s: &str) -> PathBuf {
    let p = PathBuf::from(s);
    if p.is_absolute() {
        p
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

pub async fn handle(parts: &[&str], ctx: &mut SlashCtx<'_>) -> anyhow::Result<()> {
    match parts[0] {
        "/add" => handle_add(parts, ctx).await,
        "/drop" => handle_drop(parts, ctx).await,
        "/drop-all" => handle_drop_all(ctx).await,
        _ => Ok(()),
    }
}

async fn handle_add(parts: &[&str], ctx: &mut SlashCtx<'_>) -> anyhow::Result<()> {
    if parts.len() < 2 {
        if ctx.context.extra_files.is_empty() {
            write_ok(ctx.renderer, "no files added (use /add <path>)");
        } else {
            write_ok(ctx.renderer, "added files:");
            for f in &ctx.context.extra_files {
                let size = std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
                write_result(ctx.renderer, format!("  {} ({size}B)", f.display()));
            }
        }
        return Ok(());
    }

    let path = resolve_path(parts[1]);

    if !path.exists() {
        write_error(ctx.renderer, format!("file not found: {}", path.display()));
        return Ok(());
    }
    if !path.is_file() {
        write_error(ctx.renderer, format!("not a file: {}", path.display()));
        return Ok(());
    }

    let canonical = path.canonicalize().unwrap_or(path);
    if ctx.context.extra_files.contains(&canonical) {
        write_ok(
            ctx.renderer,
            format!("already added: {}", canonical.display()),
        );
        return Ok(());
    }

    let size = std::fs::metadata(&canonical).map(|m| m.len()).unwrap_or(0);
    ctx.context.extra_files.push(canonical.clone());
    ctx.rebuild_agent().await;
    write_ok(
        ctx.renderer,
        format!("added: {} ({size}B)", canonical.display()),
    );
    Ok(())
}

async fn handle_drop(parts: &[&str], ctx: &mut SlashCtx<'_>) -> anyhow::Result<()> {
    if parts.len() < 2 {
        write_error(ctx.renderer, "usage: /drop <path>");
        return Ok(());
    }

    let path = resolve_path(parts[1]);
    let canonical = path.canonicalize().unwrap_or(path);

    let idx = ctx.context.extra_files.iter().position(|f| f == &canonical);

    match idx {
        Some(i) => {
            ctx.context.extra_files.remove(i);
            ctx.rebuild_agent().await;
            write_ok(ctx.renderer, format!("dropped: {}", canonical.display()));
        }
        None => {
            write_error(
                ctx.renderer,
                format!("not in context: {} (use /add to see)", canonical.display()),
            );
        }
    }
    Ok(())
}

async fn handle_drop_all(ctx: &mut SlashCtx<'_>) -> anyhow::Result<()> {
    let count = ctx.context.extra_files.len();
    if count == 0 {
        write_ok(ctx.renderer, "no files to drop");
    } else {
        ctx.context.extra_files.clear();
        ctx.rebuild_agent().await;
        write_ok(ctx.renderer, format!("dropped {count} file(s)"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::resolve_path;
    use std::path::PathBuf;

    #[test]
    fn test_resolve_path_absolute() {
        let result = resolve_path("/tmp/foo.txt");
        assert_eq!(result, PathBuf::from("/tmp/foo.txt"));
    }

    #[test]
    fn test_resolve_path_relative_root() {
        let result = resolve_path("/");
        assert_eq!(result, PathBuf::from("/"));
    }

    #[test]
    fn test_resolve_path_relative_is_under_cwd() {
        let result = resolve_path("bar.txt");
        let expected = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("bar.txt");
        assert_eq!(result, expected);
    }

    #[test]
    fn test_resolve_path_empty_joins_cwd() {
        let result = resolve_path("");
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        assert_eq!(result, cwd);
    }
}
