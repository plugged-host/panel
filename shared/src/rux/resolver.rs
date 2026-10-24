//! plugged.host: what an admin wrote ("<@123>", "123", "@ann", "Ann", "#giveaways",
//! "🎉│giveaways", "@Moderators", a message link) made into the Discord object it means, or a
//! [`ToolError::Invalid`] saying why it cannot be, for the model to act on. The tools resolve
//! every member, user, channel, message and role through here.

use super::{
    discord::{POSTABLE, Rest, member_name, user_name},
    tools::ToolError,
};
use serde_json::Value;
use std::sync::LazyLock;

/// A member of the server.
#[derive(Debug, Clone)]
pub struct Member {
    pub id: String,
    pub name: String,
    pub member: Value,
}

/// Any Discord user.
#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub name: String,
}

/// A message and the channel it is in.
#[derive(Debug, Clone)]
pub struct Found {
    pub channel: Value,
    pub message: Value,
}

static MESSAGE_LINK: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"^<?https?://(?:(?:ptb|canary)\.)?discord(?:app)?\.com/channels/(\d{17,20}|@me)/(\d{17,20})/(\d{17,20})>?$",
    )
    .unwrap()
});

/// The id in `input` when it is one: bare, or in Discord's markup with this prefix (`@` / `@!`
/// for members, `#` for channels, `@&` for roles).
pub fn snowflake(input: &str, prefixes: &[&str]) -> Option<String> {
    let value = input.trim();
    let digits =
        |text: &str| (17..=20).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_digit());

    if digits(value) {
        return Some(value.to_string());
    }
    let inner = value.strip_prefix('<')?.strip_suffix('>')?;
    prefixes
        .iter()
        .filter_map(|prefix| inner.strip_prefix(prefix))
        .find(|id| digits(id))
        .map(str::to_string)
}

/// A name without case, emoji and separators ("🎉│Give-aways" → "giveaways").
pub fn plain(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// The items (channels, roles) with this name: the exact name first, then the same name
/// without emoji, separators and case, then the ones whose name holds it.
pub fn by_name<'a>(items: &'a [Value], name: &str) -> Vec<&'a Value> {
    let name_of = |item: &Value| item["name"].as_str().unwrap_or_default().to_string();
    let lower = name.to_lowercase();

    let exact: Vec<&Value> = items
        .iter()
        .filter(|item| name_of(item).to_lowercase() == lower)
        .collect();
    if !exact.is_empty() {
        return exact;
    }

    let wanted = plain(name);
    if wanted.is_empty() {
        return Vec::new();
    }
    let loose: Vec<&Value> = items
        .iter()
        .filter(|item| plain(&name_of(item)) == wanted)
        .collect();
    if !loose.is_empty() {
        return loose;
    }

    items
        .iter()
        .filter(|item| plain(&name_of(item)).contains(&wanted))
        .collect()
}

/// The member a name search means: the one whose username, display name or nickname is the
/// name, else the only one found. `Err` lists the candidates (or says there were none).
pub fn pick_member(found: &[Value], query: &str) -> Result<Value, ToolError> {
    let needle = query.to_lowercase();
    let exact: Vec<&Value> = found
        .iter()
        .filter(|member| {
            [
                &member["user"]["username"],
                &member["user"]["global_name"],
                &member["nick"],
            ]
            .into_iter()
            .filter_map(Value::as_str)
            .any(|name| name.to_lowercase() == needle)
        })
        .collect();

    if exact.len() == 1 {
        return Ok(exact[0].clone());
    }
    match found {
        [only] => Ok(only.clone()),
        [] => Err(ToolError::invalid(format!(
            "No member's name starts with \"{query}\". Ask for their @mention or id."
        ))),
        _ => Err(ToolError::invalid(format!(
            "Several members match \"{query}\": {}. Ask which one is meant.",
            found
                .iter()
                .take(8)
                .map(|member| format!(
                    "{} (<@{}>)",
                    member_name(member),
                    member["user"]["id"].as_str().unwrap_or_default()
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// A message link's server, channel and message.
pub fn message_link(input: &str) -> Option<(String, String, String)> {
    let captures = MESSAGE_LINK.captures(input.trim())?;

    Some((
        captures[1].to_string(),
        captures[2].to_string(),
        captures[3].to_string(),
    ))
}

fn id_of(value: &Value) -> &str {
    value["id"].as_str().unwrap_or_default()
}

/// A member of the server, by mention, id or name.
pub async fn member(rest: &Rest, guild_id: &str, input: &str) -> Result<Member, ToolError> {
    if let Some(id) = snowflake(input, &["@!", "@"]) {
        let Some(member) = rest.member(guild_id, &id).await? else {
            return Err(ToolError::invalid(format!("<@{id}> is not in the server.")));
        };

        return Ok(Member {
            name: member_name(&member),
            id,
            member,
        });
    }

    let query = input.trim().trim_start_matches('@').trim();
    if query.is_empty() {
        return Err(ToolError::invalid("No member given."));
    }
    let found = rest.search_members(guild_id, query, 10).await?;
    let member = pick_member(&found, query)?;

    Ok(Member {
        id: member["user"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        name: member_name(&member),
        member,
    })
}

/// Any Discord user, in the server or not (for unbans): only by mention or id.
pub async fn user(rest: &Rest, input: &str) -> Result<User, ToolError> {
    let Some(id) = snowflake(input, &["@!", "@"]) else {
        return Err(ToolError::invalid(
            "Give a user id or <@mention> for someone who is not in the server.",
        ));
    };
    let Some(user) = rest.user(&id).await? else {
        return Err(ToolError::invalid(format!(
            "There is no Discord user {id}."
        )));
    };

    Ok(User {
        name: user_name(&user),
        id,
    })
}

/// A channel of the server, by mention, id or name, of these types (`None`: any; the default
/// for posting is [`POSTABLE`]).
pub async fn channel(
    rest: &Rest,
    guild_id: &str,
    input: &str,
    types: Option<&[i64]>,
) -> Result<Value, ToolError> {
    let channels = rest.channels(guild_id, false).await?;
    let id = snowflake(input, &["#"]);

    let mut matches: Vec<Value> = match &id {
        Some(id) => channels
            .iter()
            .filter(|channel| id_of(channel) == id)
            .cloned()
            .collect(),
        None => by_name(&channels, input.trim().trim_start_matches('#'))
            .into_iter()
            .cloned()
            .collect(),
    };
    // a channel made a moment ago
    if let Some(id) = &id
        && matches.is_empty()
    {
        matches = rest
            .channels(guild_id, true)
            .await?
            .into_iter()
            .filter(|channel| id_of(channel) == id)
            .collect();
    }
    if let Some(types) = types {
        matches.retain(|channel| {
            channel["type"]
                .as_i64()
                .is_some_and(|kind| types.contains(&kind))
        });
    }

    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(ToolError::invalid(format!(
            "There is no text channel \"{input}\" in the server. Use server_overview to see the channels."
        ))),
        _ => Err(ToolError::invalid(format!(
            "Several channels match \"{input}\": {}. Ask which one is meant.",
            matches
                .iter()
                .take(8)
                .map(|channel| format!("<#{}>", id_of(channel)))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// A channel to post in ([`POSTABLE`]).
pub async fn postable_channel(
    rest: &Rest,
    guild_id: &str,
    input: &str,
) -> Result<Value, ToolError> {
    channel(rest, guild_id, input, Some(&POSTABLE)).await
}

/// The channel a message lives in, threads included (the server's channel list leaves threads
/// out, so a thread is fetched and must belong to this server).
pub async fn message_channel(
    rest: &Rest,
    guild_id: &str,
    channel_id: &str,
) -> Result<Value, ToolError> {
    if let Some(channel) = rest
        .channels(guild_id, false)
        .await?
        .into_iter()
        .find(|channel| id_of(channel) == channel_id)
    {
        return Ok(channel);
    }

    let channel = rest.channel(channel_id).await.unwrap_or(Value::Null);
    if channel["guild_id"].as_str() != Some(guild_id) {
        return Err(ToolError::invalid(format!(
            "<#{channel_id}> is not a channel of this server."
        )));
    }

    Ok(channel)
}

/// A message, from its link (`https://discord.com/channels/<server>/<channel>/<message>`) or
/// its id with `channel_input` (a channel as [`channel`] takes it).
pub async fn message(
    rest: &Rest,
    guild_id: &str,
    input: &str,
    channel_input: &str,
) -> Result<Found, ToolError> {
    let value = input.trim();
    let (channel, message_id) = if let Some((server, channel_id, message_id)) = message_link(value)
    {
        if server != guild_id {
            return Err(ToolError::invalid(
                "That message link is not from this server.",
            ));
        }
        (
            message_channel(rest, guild_id, &channel_id).await?,
            message_id,
        )
    } else if snowflake(value, &[]).is_some() {
        if channel_input.trim().is_empty() {
            return Err(ToolError::invalid(
                "A message id alone does not say which channel it is in: give the message link, or the channel too.",
            ));
        }
        let channel = match snowflake(channel_input, &["#"]) {
            Some(id) => message_channel(rest, guild_id, &id).await?,
            None => channel(rest, guild_id, channel_input, None).await?,
        };
        (channel, value.to_string())
    } else {
        return Err(ToolError::invalid(format!(
            "\"{input}\" is not a message link or id. Use read_messages to find the message."
        )));
    };

    let channel_id = id_of(&channel).to_string();
    let Some(message) = rest.message(&channel_id, &message_id).await? else {
        return Err(ToolError::invalid(format!(
            "There is no message {message_id} in <#{channel_id}> (deleted already, or another channel)."
        )));
    };

    Ok(Found { channel, message })
}

/// A role of the server, by mention, id or name.
pub async fn role(rest: &Rest, guild_id: &str, input: &str) -> Result<Value, ToolError> {
    let roles = rest.roles(guild_id, false).await?;
    let mut matches: Vec<Value> = match snowflake(input, &["@&"]) {
        Some(id) => roles
            .iter()
            .filter(|role| id_of(role) == id)
            .cloned()
            .collect(),
        None => by_name(&roles, input.trim().trim_start_matches('@'))
            .into_iter()
            .cloned()
            .collect(),
    };

    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(ToolError::invalid(format!(
            "There is no role \"{input}\". Use server_overview to see the roles."
        ))),
        _ => Err(ToolError::invalid(format!(
            "Several roles match \"{input}\": {}. Ask which one is meant.",
            matches
                .iter()
                .take(8)
                .map(|role| format!(
                    "{} ({})",
                    role["name"].as_str().unwrap_or_default(),
                    id_of(role)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ID: &str = "100000000000000001";

    #[test]
    fn ids_bare_or_in_markup() {
        assert_eq!(snowflake(ID, &[]).as_deref(), Some(ID));
        assert_eq!(
            snowflake(&format!(" <@!{ID}> "), &["@!", "@"]).as_deref(),
            Some(ID)
        );
        assert_eq!(
            snowflake(&format!("<@{ID}>"), &["@!", "@"]).as_deref(),
            Some(ID)
        );
        assert_eq!(snowflake(&format!("<#{ID}>"), &["#"]).as_deref(), Some(ID));
        assert_eq!(
            snowflake(&format!("<@&{ID}>"), &["@&"]).as_deref(),
            Some(ID)
        );
        // a role mention is no member, a short number no id
        assert_eq!(snowflake(&format!("<@&{ID}>"), &["@!", "@"]), None);
        assert_eq!(snowflake("12345", &[]), None);
        assert_eq!(snowflake("@ann", &["@"]), None);
    }

    #[test]
    fn names_match_exactly_then_loosely_then_partly() {
        let items = vec![
            json!({"id": "1", "name": "🎉│giveaways"}),
            json!({"id": "2", "name": "giveaway-winners"}),
            json!({"id": "3", "name": "General"}),
        ];
        let ids = |name: &str| {
            by_name(&items, name)
                .into_iter()
                .map(|item| item["id"].as_str().unwrap())
                .collect::<Vec<_>>()
        };

        assert_eq!(ids("general"), vec!["3"]);
        assert_eq!(ids("giveaways"), vec!["1"]);
        assert_eq!(ids("giveaway"), vec!["1", "2"]);
        assert_eq!(ids("🎉"), Vec::<&str>::new());
        assert_eq!(plain("🎉│Give-aways"), "giveaways");
    }

    #[test]
    fn members_by_name() {
        let ann = json!({"user": {"id": "1", "username": "ann"}, "nick": null});
        let annie = json!({"user": {"id": "2", "username": "annie", "global_name": "Annie"}});

        assert_eq!(
            pick_member(&[ann.clone(), annie.clone()], "Ann").unwrap()["user"]["id"],
            "1"
        );
        assert_eq!(
            pick_member(&[annie.clone()], "an").unwrap()["user"]["id"],
            "2"
        );
        let Err(ToolError::Invalid(several)) = pick_member(&[ann.clone(), annie.clone()], "an")
        else {
            panic!("several match");
        };
        assert!(several.contains("ann (<@1>), Annie (<@2>)"));
        assert!(
            matches!(pick_member(&[], "zed"), Err(ToolError::Invalid(m)) if m.contains("No member's name"))
        );
    }

    #[test]
    fn message_links() {
        let link = format!("https://discord.com/channels/{ID}/{ID}/{ID}");
        assert_eq!(
            message_link(&link),
            Some((ID.to_string(), ID.to_string(), ID.to_string()))
        );
        assert!(
            message_link(&format!(
                "<https://ptb.discordapp.com/channels/{ID}/{ID}/{ID}>"
            ))
            .is_some()
        );
        assert!(message_link(&format!("https://example.com/channels/{ID}/{ID}/{ID}")).is_none());
    }
}
