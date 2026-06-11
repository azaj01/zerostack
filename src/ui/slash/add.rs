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

#[cfg(feature = "multimodal")]
fn has_pending_media(ctx: &SlashCtx<'_>) -> bool {
    !ctx.session.pending_media.is_empty()
}

#[cfg(not(feature = "multimodal"))]
fn has_pending_media(_ctx: &SlashCtx<'_>) -> bool {
    false
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
        if ctx.context.extra_files.is_empty() && !has_pending_media(ctx) {
            write_ok(ctx.renderer, "no files added (use /add <path>)");
        } else {
            write_ok(ctx.renderer, "added files:");
            for f in &ctx.context.extra_files {
                let size = std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
                write_result(ctx.renderer, format!("  {} ({size}B)", f.display()));
            }
            #[cfg(feature = "multimodal")]
            for m in &ctx.session.pending_media {
                let (kind, size, mime) = match m {
                    crate::extras::multimodal::MediaAttachment::Image { data, mime, .. } => {
                        ("image", data.len(), mime.as_str())
                    }
                    crate::extras::multimodal::MediaAttachment::Audio { data, mime, .. } => {
                        ("audio", data.len(), mime.as_str())
                    }
                    crate::extras::multimodal::MediaAttachment::Document { data, mime, .. } => {
                        ("document", data.len(), mime.as_str())
                    }
                };
                write_result(ctx.renderer, format!("  [{kind}] {mime} ({size}B)"));
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

    #[cfg(feature = "multimodal")]
    if crate::extras::multimodal::detect_media(&path).is_some() {
        match crate::extras::multimodal::load_attachment(&path) {
            Ok(attachment) => {
                let size = attachment.size();
                let mime = match &attachment {
                    crate::extras::multimodal::MediaAttachment::Image { mime, .. }
                    | crate::extras::multimodal::MediaAttachment::Audio { mime, .. }
                    | crate::extras::multimodal::MediaAttachment::Document { mime, .. } => {
                        mime.clone()
                    }
                };
                ctx.session.pending_media.push(attachment);
                write_ok(
                    ctx.renderer,
                    format!("attached: {} ({mime}, {size}B)", path.display()),
                );
            }
            Err(e) => {
                write_error(ctx.renderer, format!("failed to load media: {e}"));
            }
        }
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
