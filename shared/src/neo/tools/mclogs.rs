//! plugged.host: neo's `read_mclogs`: a log shared on mclo.gs, with the problems mclo.gs's
//! analysis found and the log's error entries (docs/features/neo.md › Tools). It reads a public
//! paste only, so it needs no server permission.

use super::{Tool, ToolContext, ToolError, ToolOutcome, ToolResult, ToolScope, support};
use crate::minecraft::log_check::{self, Entry, PasteError};
use serde_json::{Value, json};
use std::{borrow::Cow, collections::HashSet};

const MAX_PROBLEMS: usize = 10;
const MAX_ENTRIES: usize = 8;
const MAX_ENTRY_LINES: usize = 14;
const MAX_LINE_CHARS: usize = 400;
const MAX_INFORMATION: usize = 8;

/// One line of a log for the model: tabs as spaces, no control characters, secrets masked (on the
/// line itself: `key=value` patterns only match at a line's start, before it is quoted), trimmed
/// and capped.
fn line(text: &str) -> String {
    let text = crate::redact::strip_ansi(&text.replace('\t', "  "));
    let text: String = text.chars().filter(|c| !c.is_control()).collect();

    support::short(&support::redact(text.trim()), MAX_LINE_CHARS)
}

/// A problem's or an entry's lines, quoted with their line numbers.
fn quote(lines: &[(u64, String)], indent: &str) -> Vec<String> {
    let mut quoted: Vec<String> = lines
        .iter()
        .take(MAX_ENTRY_LINES)
        .map(|(number, content)| format!("{indent}line {number}: {}", line(content)))
        .collect();
    if lines.len() > MAX_ENTRY_LINES {
        quoted.push(format!(
            "{indent}… {} more lines",
            lines.len() - MAX_ENTRY_LINES
        ));
    }

    quoted
}

/// The lines a problem of mclo.gs's analysis points at.
fn problem_lines(problem: &Value) -> Vec<(u64, String)> {
    problem["entry"]["lines"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter(|line| line.is_object())
        .map(|line| {
            (
                line["number"].as_u64().unwrap_or(0),
                line["content"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// The text the model reads: the paste's info, its problems with fixes, then the entries no
/// problem already quotes (the last [`MAX_ENTRIES`]).
fn report(paste: &log_check::Paste, entries: &[Entry], level: &str) -> String {
    let mut out = vec![format!(
        "mclo.gs log {}{}: {} lines, {} error entries. Link: {}",
        paste.id,
        paste
            .title
            .as_deref()
            .filter(|title| !title.is_empty())
            .map(|title| format!(" ({title})"))
            .unwrap_or_default(),
        paste.lines,
        paste.errors,
        paste.url
    )];

    let information: Vec<String> = paste
        .information
        .iter()
        .filter_map(|item| item["message"].as_str())
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .take(MAX_INFORMATION)
        .map(str::to_string)
        .collect();
    if !information.is_empty() {
        out.push(format!("Detected: {}", information.join("; ")));
    }
    out.push(String::new());

    if paste.problems.is_empty() {
        out.push("mclo.gs's analysis found no known problem.".into());
    } else {
        out.push(format!(
            "Problems found by mclo.gs's analysis ({}):",
            paste.problems.len()
        ));
        for (i, problem) in paste.problems.iter().take(MAX_PROBLEMS).enumerate() {
            out.push(format!(
                "{}. {} (seen {}×)",
                i + 1,
                line(problem["message"].as_str().unwrap_or_default()),
                problem["counter"].as_u64().unwrap_or(1).max(1)
            ));
            out.extend(quote(&problem_lines(problem), "   "));

            let fixes: Vec<String> = problem["solutions"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .filter_map(|fix| fix["message"].as_str())
                .map(line)
                .filter(|fix| !fix.is_empty())
                .collect();
            if !fixes.is_empty() {
                out.push(format!("   Suggested fixes: {}", fixes.join(" | ")));
            }
        }
    }

    // entries a problem above already quotes are not repeated
    let quoted: HashSet<u64> = paste
        .problems
        .iter()
        .flat_map(|problem| problem_lines(problem).into_iter().map(|(number, _)| number))
        .collect();
    let entries: Vec<&Entry> = entries
        .iter()
        .filter(|entry| {
            entry
                .lines
                .first()
                .is_none_or(|(number, _)| !quoted.contains(number))
        })
        .collect();

    if !entries.is_empty() {
        let shown = &entries[entries.len().saturating_sub(MAX_ENTRIES)..];
        let mut level = level.to_string();
        if paste.problems.is_empty() {
            if let Some(first) = level.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
        } else {
            level = format!("Other {level}");
        }

        out.push(String::new());
        out.push(format!(
            "{level} entries in the log ({}):",
            if shown.len() < entries.len() {
                format!("last {} of {}", shown.len(), entries.len())
            } else {
                entries.len().to_string()
            }
        ));
        for entry in shown {
            out.extend(quote(&entry.lines, "  "));
            out.push(String::new());
        }
    } else if paste.errors > 0 && paste.problems.is_empty() {
        out.push(
            "The error entries could not be listed (the log is too large). Read the console or logs/latest.log instead."
                .into(),
        );
    }

    support::limit_output(&support::redact(out.join("\n").trim_end()))
}

pub struct ReadMclogsTool;

#[async_trait::async_trait]
impl Tool for ReadMclogsTool {
    fn name(&self) -> &'static str {
        "read_mclogs"
    }

    fn description(&self) -> Cow<'static, str> {
        "Read a log shared on mclo.gs (by link or id): the problems mclo.gs detected with suggested fixes, detected versions, and the log's error entries. The log is a snapshot from when it was uploaded; check the current console and files before changing anything.".into()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "log": {"type": "string", "description": "The mclo.gs link (https://mclo.gs/abc123) or just its id."},
            },
            "required": ["log"],
        })
    }

    fn scope(&self) -> ToolScope {
        ToolScope::Any
    }

    fn summarize(&self, arguments: &Value) -> String {
        match arguments
            .get("log")
            .and_then(Value::as_str)
            .and_then(log_check::id_from)
        {
            Some(id) => format!("Read the mclo.gs log {id}"),
            None => "Read a mclo.gs log".into(),
        }
    }

    async fn execute(&self, context: &ToolContext<'_>, arguments: &Value) -> ToolOutcome {
        let Some(id) = arguments
            .get("log")
            .and_then(Value::as_str)
            .and_then(log_check::id_from)
        else {
            return Err(ToolError::invalid(
                "\"log\" must be a mclo.gs link like https://mclo.gs/abc123 or its id.",
            ));
        };

        let read = async {
            let paste = log_check::paste(context.state, &id).await?;
            let mut level = "error";
            let mut entries =
                log_check::entries(context.state, &id, paste.size, log_check::LEVEL_ERROR).await?;
            if entries.is_empty() && paste.problems.is_empty() {
                entries =
                    log_check::entries(context.state, &id, paste.size, log_check::LEVEL_WARNING)
                        .await?;
                level = "warning";
            }

            Ok::<_, PasteError>((paste, entries, level))
        };

        let (paste, entries, level) = match read.await {
            Ok(read) => read,
            Err(PasteError::NotFound) => {
                return Ok(ToolResult::error(
                    "That log does not exist on mclo.gs (or it has expired).",
                ));
            }
            Err(PasteError::Failed(err)) => {
                tracing::debug!("neo could not read mclo.gs log {id}: {err:#}");
                return Ok(ToolResult::error(
                    "mclo.gs could not be reached. Read the console instead.",
                ));
            }
        };

        let display = if paste.problems.is_empty() {
            format!("{} error entries, no known problem", paste.errors)
        } else {
            format!("{} problem(s) found", paste.problems.len())
        };

        Ok(ToolResult::ok(report(&paste, &entries, level)).with_display(display))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paste(problems: Vec<Value>, errors: u64) -> log_check::Paste {
        log_check::Paste {
            id: "abc123".into(),
            url: "https://mclo.gs/abc123".into(),
            title: Some("Paper 1.21 Server Log".into()),
            size: 100,
            lines: 40,
            errors,
            problems,
            information: vec![json!({"message": "Minecraft version: 1.21"}), json!({})],
        }
    }

    #[test]
    fn reports_problems_and_other_entries() {
        let problem = json!({
            "message": "The plugin 'Foo' needs 'Bar'.",
            "counter": 2,
            "entry": {"lines": [{"number": 7, "content": "[ERROR]\tCould not load Foo"}]},
            "solutions": [{"message": "Install Bar."}, {"message": ""}],
        });
        let entries = vec![
            Entry {
                level: 3,
                lines: vec![(7, "Could not load Foo".into())],
            },
            Entry {
                level: 3,
                lines: vec![(12, "\u{1b}[31mpassword=hunter2\u{1b}[0m".into())],
            },
        ];

        let text = report(&paste(vec![problem], 2), &entries, "error");
        assert!(
            text.starts_with(
                "mclo.gs log abc123 (Paper 1.21 Server Log): 40 lines, 2 error entries."
            )
        );
        assert!(text.contains("Detected: Minecraft version: 1.21\n"));
        assert!(text.contains("1. The plugin 'Foo' needs 'Bar'. (seen 2×)"));
        assert!(text.contains("   line 7: [ERROR]  Could not load Foo"));
        assert!(text.contains("   Suggested fixes: Install Bar.\n"));
        assert!(text.contains("Other error entries in the log (1):"));
        assert!(!text.contains("hunter2"));
        assert_eq!(text.matches("line 7:").count(), 1);
    }

    #[test]
    fn reports_logs_without_problems() {
        let text = report(&paste(Vec::new(), 0), &[], "warning");
        assert!(text.contains("mclo.gs's analysis found no known problem."));

        let entries: Vec<Entry> = (1..=10)
            .map(|n| Entry {
                level: 4,
                lines: vec![(n, format!("warning {n}"))],
            })
            .collect();
        let text = report(&paste(Vec::new(), 0), &entries, "warning");
        assert!(text.contains("Warning entries in the log (last 8 of 10):"));
        assert!(!text.contains("warning 2\n"));
        assert!(text.contains("line 10: warning 10"));

        let text = report(&paste(Vec::new(), 5), &[], "error");
        assert!(text.contains("could not be listed"));
    }

    #[test]
    fn quotes_are_capped() {
        let lines: Vec<(u64, String)> = (1..=20).map(|n| (n, "x".repeat(500))).collect();
        let quoted = quote(&lines, "  ");
        assert_eq!(quoted.len(), MAX_ENTRY_LINES + 1);
        assert_eq!(quoted.last().unwrap(), "  … 6 more lines");
        assert!(quoted[0].chars().count() < MAX_LINE_CHARS + 20);
        assert_eq!(
            ReadMclogsTool.summarize(&json!({"log": "https://mclo.gs/abc123"})),
            "Read the mclo.gs log abc123"
        );
        assert_eq!(
            ReadMclogsTool.summarize(&json!({"log": "https://example.com/x"})),
            "Read a mclo.gs log"
        );
    }
}
