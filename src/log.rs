use std::sync::OnceLock;

use serenity::all::ChannelId;

pub static LOGGING_CHANNEL: OnceLock<ChannelId> = OnceLock::new();

pub fn init(channel_id: ChannelId) {
    LOGGING_CHANNEL
        .set(channel_id)
        .expect("Logging channel already initialized");
}

macro_rules! discord_log {
    ($ctx:expr, $($arg:tt)*) => {{
        let content = {
            let s = format!($($arg)*);
            match s.char_indices().nth(serenity::constants::MESSAGE_CODE_LIMIT) {
                Some((i, _)) => format!("{}...", &s[..i-3]),
                None => s,
            }
        };
        async move {
            if let Some(channel) = crate::log::LOGGING_CHANNEL.get() {
                if let Err(e) = channel.say(&$ctx.http, &content).await {
                    eprintln!("Failed to send log: {e:?}");
                }
            } else {
                eprintln!("{content}");
            }
        }
    }};
}

pub(crate) use discord_log;
