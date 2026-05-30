use rig::completion::ToolDefinition;
use rig::tool::Tool;

use crate::agent::tools::crc::crc32_hex;
use crate::agent::tools::{
    AskSender, PermCheck, ReadArgs, ToolError, check_perm_path, edit_system,
};
use crate::config::types::EditSystem;

const DEFAULT_MAX_TEXT_SIZE: u64 = 1024 * 1024;

pub struct ReadTool {
    pub permission: Option<PermCheck>,
    pub ask_tx: Option<AskSender>,
    pub max_text_file_size: u64,
}

impl ReadTool {
    pub fn new(
        permission: Option<PermCheck>,
        ask_tx: Option<AskSender>,
        max_text_file_size: Option<u64>,
    ) -> Self {
        ReadTool {
            permission,
            ask_tx,
            max_text_file_size: max_text_file_size.unwrap_or(DEFAULT_MAX_TEXT_SIZE),
        }
    }
}

impl Tool for ReadTool {
    const NAME: &'static str = "read";

    type Error = ToolError;
    type Args = ReadArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let (desc, params) = match edit_system() {
            EditSystem::Similarity => (
                "Read the contents of a file. Supports text files. Defaults to first 2000 lines. Use offset/limit for large files.".to_string(),
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Path to the file (relative or absolute)" },
                        "offset": { "type": "integer", "description": "Line number to start from (1-indexed)" },
                        "limit": { "type": "integer", "description": "Maximum number of lines to read" }
                    },
                    "required": ["path"]
                }),
            ),
            EditSystem::Hashedit => (
                "Read file contents with CRC-32 tagged lines for tag-based editing. Each line is prefixed with 'N|TAG' where TAG is an 8-char hex CRC-32 of the line content. Use these tags with the edit tool for CAS-guarded edits. Defaults to first 2000 lines.".to_string(),
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Path to the file (relative or absolute)" },
                        "offset": { "type": "integer", "description": "Line number to start from (1-indexed)" },
                        "limit": { "type": "integer", "description": "Maximum number of lines to read" }
                    },
                    "required": ["path"]
                }),
            ),
        };

        ToolDefinition {
            name: "read".to_string(),
            description: desc,
            parameters: params,
        }
    }

    async fn call(&self, args: ReadArgs) -> Result<String, ToolError> {
        let path = crate::fs::expand_tilde(&args.path);
        check_perm_path(&self.permission, &self.ask_tx, "read", &path).await?;

        let metadata = tokio::fs::metadata(&path).await?;
        let file_size = metadata.len();
        if file_size > self.max_text_file_size {
            return Err(ToolError::Msg(format!(
                "File too large ({} bytes). Maximum allowed file size is {} bytes.",
                file_size, self.max_text_file_size
            )));
        }
        let content = tokio::fs::read_to_string(&path).await?;
        let total_lines = content.lines().count();

        let offset = args.offset.unwrap_or(1).saturating_sub(1);
        let limit = args.limit.unwrap_or(2000);
        let end = (offset + limit).min(total_lines);

        let es = edit_system();

        let excerpt: String = match es {
            EditSystem::Hashedit => {
                // Annotate each line with CRC-32 tag
                content
                    .lines()
                    .skip(offset)
                    .take(end - offset)
                    .enumerate()
                    .map(|(i, line)| {
                        let line_num = offset + i + 1;
                        let tag = crc32_hex(line.as_bytes());
                        let line_num_width = if total_lines >= 1000 { 4 } else { 3 };
                        format!(
                            "{:>width$}|{} {}",
                            line_num,
                            tag,
                            line,
                            width = line_num_width
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            EditSystem::Similarity => {
                // Plain text (original behavior)
                content
                    .lines()
                    .skip(offset)
                    .take(end - offset)
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        };

        let info = match es {
            EditSystem::Hashedit => {
                let file_crc = crc32_hex(content.replace("\r\n", "\n").as_bytes());
                format!(
                    "File: {} ({} lines total, lines {}-{}) [CRC: {}]\n\n{}",
                    path,
                    total_lines,
                    offset + 1,
                    end,
                    file_crc,
                    excerpt
                )
            }
            EditSystem::Similarity => {
                format!(
                    "File: {} ({} lines total, showing lines {}-{})\n\n{}",
                    path,
                    total_lines,
                    offset + 1,
                    end,
                    excerpt
                )
            }
        };

        Ok(info)
    }
}
