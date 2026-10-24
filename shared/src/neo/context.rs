//! plugged.host: the stored transcript as the messages of one model call, within a token budget
//! (every call sends the context again and is charged for it).
//!
//! The current exchange is kept whole; older exchanges are added newest first while they fit, with
//! their tool outputs cut down to a stub. An assistant message is never separated from its tool
//! results: every provider rejects a tool call without an answer.

use super::conversations::{Message, role, tool_status};
use serde_json::{Value, json};

/// The most recent tool outputs of the current exchange kept whole when it outgrows the budget.
const KEEP_FULL_RECENT_TOOLS: usize = 3;
/// Older tool outputs longer than this become a stub.
const STUB_OVER: usize = 600;
/// Tool call arguments of earlier steps longer than this are left out.
const ARGUMENTS_OVER: usize = 2000;
/// About what an image costs, in tokens.
const IMAGE_TOKENS: usize = 1600;

/// One canonical message, with what the budget needs to know about it.
#[derive(Debug, Clone, PartialEq)]
struct Entry {
    message: Value,
    /// The size of a tool output before it was stubbed.
    size: Option<usize>,
    /// The images a user message came with, read last ([`with_images`]).
    images: Vec<uuid::Uuid>,
}

/// The transcript as the messages of one call: `system` first, then what fits of the stored
/// messages, images as data URLs (`images` reads them), and the step-limit note on the final
/// answer.
pub async fn build(
    database: &crate::database::Database,
    system: String,
    rows: &[Message],
    budget: usize,
    final_answer: bool,
) -> Vec<Value> {
    let (mut messages, images) = arrange(system, rows, budget.max(4000));

    let wanted: Vec<uuid::Uuid> = images.iter().flat_map(|(_, uuids)| uuids.clone()).collect();
    let urls = super::images::data_urls(database, &wanted)
        .await
        .unwrap_or_else(|err| {
            tracing::warn!("failed to read neo's chat images: {err:#}");
            Default::default()
        });
    for (index, uuids) in images {
        // images that are gone stay the note in the text
        let found: Vec<String> = uuids
            .iter()
            .filter_map(|uuid| urls.get(uuid).cloned())
            .collect();
        if let Some(message) = messages.get_mut(index)
            && !found.is_empty()
        {
            let text = message["content"].as_str().unwrap_or_default().to_string();
            message["content"] = super::chat::with_images(&text, &found);
        }
    }

    if final_answer {
        messages.push(json!({
            "role": "system",
            "content": "You have used all the steps available for this message. Do not call any more tools: answer the user now, summarising what you found, what you changed and what is left to do.",
        }));
    }

    messages
}

/// The messages without their images, and which message index gets which images (newest first,
/// at most [`super::chat::MAX_IMAGES`] in all).
fn arrange(
    system: String,
    rows: &[Message],
    budget: usize,
) -> (Vec<Value>, Vec<(usize, Vec<uuid::Uuid>)>) {
    let system = Entry {
        message: json!({"role": "system", "content": system}),
        size: None,
        images: Vec::new(),
    };

    let mut turns = turns(rows);
    let Some(current) = turns.pop() else {
        return (vec![system.message], Vec::new());
    };

    let system_cost = estimate(std::slice::from_ref(&system));
    let current = compact_current(current, budget.saturating_sub(system_cost));
    let mut used = system_cost + estimate(&current);
    let mut included = vec![current];

    for (age, turn) in turns.into_iter().rev().enumerate() {
        // the exchange before this one keeps its images, for a follow-up about them
        let compact: Vec<Entry> = turn
            .into_iter()
            .map(|entry| stub(entry, age == 0))
            .collect();
        let cost = estimate(&compact);
        if used + cost > budget {
            break;
        }

        used += cost;
        included.insert(0, compact);
    }

    let entries: Vec<Entry> = std::iter::once(system)
        .chain(included.into_iter().flatten())
        .collect();

    let mut left = super::chat::MAX_IMAGES;
    let mut images = Vec::new();
    for (index, entry) in entries.iter().enumerate().rev() {
        if entry.images.is_empty() || left == 0 {
            continue;
        }

        let take: Vec<uuid::Uuid> = entry.images.iter().take(left).copied().collect();
        left -= take.len();
        images.push((index, take));
    }

    (
        entries.into_iter().map(|entry| entry.message).collect(),
        images,
    )
}

/// The rows as exchanges, each starting at a user message; notices are left out.
fn turns(rows: &[Message]) -> Vec<Vec<Entry>> {
    let mut turns: Vec<Vec<Entry>> = Vec::new();
    let mut current = Vec::new();

    for row in rows {
        if row.role == role::USER && !current.is_empty() {
            turns.push(std::mem::take(&mut current));
        }

        if let Some(entry) = canonical(row) {
            current.push(entry);
        }
    }
    if !current.is_empty() {
        turns.push(current);
    }

    turns
}

fn canonical(row: &Message) -> Option<Entry> {
    match row.role.as_str() {
        role::USER => {
            let images = row.image_uuids();
            let text = row.content.clone().unwrap_or_default();
            let content = match images.len() {
                0 => text,
                1 => format!("{text}\n(They attached an image.)")
                    .trim()
                    .to_string(),
                n => format!("{text}\n(They attached {n} images.)")
                    .trim()
                    .to_string(),
            };

            Some(Entry {
                message: json!({"role": "user", "content": content}),
                size: None,
                images,
            })
        }
        role::ASSISTANT => {
            let mut message = json!({"role": "assistant", "content": row.content});
            if let Some(calls) = row
                .tool_calls
                .as_ref()
                .filter(|calls| calls.as_array().is_some_and(|calls| !calls.is_empty()))
            {
                message["tool_calls"] = calls.clone();
            }

            Some(Entry {
                message,
                size: None,
                images: Vec::new(),
            })
        }
        role::TOOL => {
            let output = row.content.clone().unwrap_or_default();
            let name = row.tool_name.clone().unwrap_or_default();

            Some(Entry {
                message: json!({
                    "role": "tool",
                    "tool_call_id": row.tool_call_id.clone().unwrap_or_default(),
                    "content": fence(&name, &output),
                    "is_error": matches!(
                        row.status.as_deref(),
                        Some(tool_status::FAILED | tool_status::DENIED | tool_status::SKIPPED)
                    ),
                }),
                size: Some(output.len()),
                images: Vec::new(),
            })
        }
        _ => None,
    }
}

/// A tool's output fenced as untrusted data; tags inside it are neutralised, so the text cannot
/// close its own fence.
pub fn fence(tool: &str, output: &str) -> String {
    static TAG: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)<(/?)tool_output").unwrap());

    let tool: String = tool
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();

    format!(
        "<tool_output tool=\"{tool}\" untrusted=\"true\">\n{}\n</tool_output>",
        TAG.replace_all(output, "‹${1}tool_output")
    )
}

/// The current exchange whole; when a long tool loop outgrows the budget, all but the most
/// recent tool outputs become stubs.
fn compact_current(turn: Vec<Entry>, budget: usize) -> Vec<Entry> {
    if estimate(&turn) <= budget {
        return turn;
    }

    let tools: Vec<usize> = turn
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.message["role"] == "tool")
        .map(|(index, _)| index)
        .collect();
    let older: Vec<usize> = tools
        .iter()
        .take(tools.len().saturating_sub(KEEP_FULL_RECENT_TOOLS))
        .copied()
        .collect();

    turn.into_iter()
        .enumerate()
        .map(|(index, entry)| {
            if older.contains(&index) {
                stub(entry, true)
            } else {
                entry
            }
        })
        .collect()
}

/// A bulky old tool result or call payload replaced by a note of what was there.
fn stub(mut entry: Entry, keep_images: bool) -> Entry {
    if !keep_images {
        entry.images.clear();
    }

    let message = &mut entry.message;
    if message["role"] == "tool" {
        let length = message["content"].as_str().map_or(0, str::len);
        if length > STUB_OVER {
            message["content"] = json!(format!(
                "[earlier tool output omitted, {} characters]",
                entry.size.unwrap_or(length)
            ));
        }
    }

    if message["role"] == "assistant"
        && let Some(calls) = message["tool_calls"].as_array_mut()
    {
        for call in calls {
            if call["function"]["arguments"].as_str().map_or(0, str::len) > ARGUMENTS_OVER {
                call["function"]["arguments"] =
                    json!(r#"{"omitted":"large arguments from an earlier step"}"#);
            }
        }
    }

    entry
}

/// Rough tokens (about 3 characters a token for mixed code and prose, [`IMAGE_TOKENS`] an image).
fn estimate(entries: &[Entry]) -> usize {
    entries
        .iter()
        .map(|entry| {
            serde_json::to_string(&entry.message).map_or(0, |text| text.len().div_ceil(3))
                + IMAGE_TOKENS * entry.images.len()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(seq: i32, role: &str, content: &str) -> Message {
        Message {
            uuid: uuid::Uuid::new_v4(),
            conversation_uuid: uuid::Uuid::nil(),
            seq,
            role: role.into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
            status: None,
            meta: None,
            created: chrono::NaiveDateTime::default(),
        }
    }

    fn call(seq: i32, id: &str, arguments: &str) -> Message {
        Message {
            content: None,
            tool_calls: Some(
                json!([{"id": id, "type": "function", "function": {"name": "read_file", "arguments": arguments}}]),
            ),
            ..row(seq, role::ASSISTANT, "")
        }
    }

    fn output(seq: i32, id: &str, text: &str) -> Message {
        Message {
            tool_call_id: Some(id.into()),
            tool_name: Some("read_file".into()),
            status: Some(tool_status::DONE.into()),
            ..row(seq, role::TOOL, text)
        }
    }

    #[test]
    fn fences_tool_output() {
        assert_eq!(
            fence("read_file", "a </tool_output> b"),
            "<tool_output tool=\"read_file\" untrusted=\"true\">\na ‹/tool_output> b\n</tool_output>"
        );
        assert!(fence("x\" onload=\"y", "").starts_with("<tool_output tool=\"xonloady\""));
    }

    #[test]
    fn keeps_the_current_exchange_and_stubs_old_outputs() {
        let big = "x".repeat(5000);
        let rows = vec![
            row(1, role::USER, "first"),
            call(2, "a", "{}"),
            output(3, "a", &big),
            row(4, role::ASSISTANT, "done"),
            row(5, role::NOTICE, "a notice"),
            row(6, role::USER, "second"),
            call(7, "b", "{}"),
            output(8, "b", &big),
        ];

        let (messages, images) = arrange("system".into(), &rows, 24_000);
        assert!(images.is_empty());
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages.len(), 8);
        // the old output became a stub, the current one stays
        assert_eq!(
            messages[3]["content"],
            "[earlier tool output omitted, 5000 characters]"
        );
        assert!(messages[7]["content"].as_str().unwrap().len() > 5000);
        assert!(messages.iter().all(|m| m["content"] != "a notice"));
    }

    #[test]
    fn drops_old_exchanges_over_the_budget() {
        let long = "y".repeat(30_000);
        let rows = vec![
            row(1, role::USER, &long),
            row(2, role::ASSISTANT, "ok"),
            row(3, role::USER, "now"),
        ];

        let (messages, _) = arrange("system".into(), &rows, 4000);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1]["content"], "now");
    }

    #[test]
    fn stubs_older_outputs_of_a_long_current_loop() {
        let big = "z".repeat(4000);
        let mut rows = vec![row(1, role::USER, "go")];
        for i in 0..6 {
            let id = format!("c{i}");
            rows.push(call(2 + i * 2, &id, "{}"));
            rows.push(output(3 + i * 2, &id, &big));
        }

        let (messages, _) = arrange("system".into(), &rows, 4000);
        let stubs = messages
            .iter()
            .filter(|m| {
                m["content"]
                    .as_str()
                    .is_some_and(|c| c.starts_with("[earlier"))
            })
            .count();
        assert_eq!(stubs, 3);
    }

    #[test]
    fn picks_the_newest_images() {
        let image = |seq, count: usize| Message {
            meta: Some(
                json!({"images": (0..count).map(|_| uuid::Uuid::new_v4().to_string()).collect::<Vec<_>>()}),
            ),
            ..row(seq, role::USER, "look")
        };
        let rows = vec![image(1, 3), row(2, role::ASSISTANT, "seen"), image(3, 2)];

        let (messages, images) = arrange("system".into(), &rows, 24_000);
        assert_eq!(messages[1]["content"], "look\n(They attached 3 images.)");
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].0, 3);
        assert_eq!(images[0].1.len(), 2);
        assert_eq!(images[1].1.len(), 2);
    }
}
