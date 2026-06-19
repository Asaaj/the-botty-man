# the-botty-man

A Discord bot built with [serenity](https://docs.rs/serenity/0.12) and tokio.

## Running

```
DISCORD_TOKEN=<token> LOGGING_CHANNEL_ID=<snowflake> GUILD_ID=<snowflake> cargo run
```

`GUILD_ID` is optional. When set, slash commands register instantly on that guild (good for development). When omitted, commands register globally (up to 1 hour propagation).

`MESSAGE_CONTENT` is a privileged gateway intent — enable it in the Discord developer portal under Bot → Privileged Gateway Intents.

## Project layout

```
src/
  main.rs          — Config (clap), client setup, Ctrl+C shutdown
  bot.rs           — EventHandler: routes gateway events, registers slash commands on ready
  log.rs           — Global logging channel + discord_log! macro
  skills/
    mod.rs         — all_commands() registry and interaction dispatch
    schedule.rs    — /schedule command and its modal handler
```

## Adding a skill

1. Create `src/skills/<name>.rs` with:
   - `pub const NAME: &str = "<name>";`
   - `pub fn register() -> CreateCommand` — command definition
   - `pub async fn run(ctx: &Context, interaction: &CommandInteraction)` — slash command handler
   - If the command opens a modal: `pub const MODAL_ID: &str = "<name>_modal";` and `pub async fn handle_modal(ctx: &Context, interaction: &ModalInteraction)`

2. Add `pub mod <name>;` to `skills/mod.rs`

3. Add `<name>::register()` to the `vec!` in `all_commands()`

4. Add match arms in `dispatch()` for `Interaction::Command` and `Interaction::Modal` as needed

See `skills/schedule.rs` as the reference implementation.

## Serenity gotchas

**`CreateInputText::new` argument order is `(style, label, custom_id)`** — label and custom_id look similar and are easy to swap. Swapping them causes modal field values to always come back empty because Discord echoes back the custom_id set at send time.

**`MESSAGE_CODE_LIMIT` is 2000 Unicode code points**, not bytes. `discord_log!` truncates automatically using `char_indices`, not byte length.

**`Command::set_global_commands` replaces all global commands atomically** on every `ready` event (including reconnects). This is intentional — commands always stay in sync with what's registered in `all_commands()`.

**Modal submit values** arrive as `Option<String>` on `InputText.value`, but are always `Some` when Discord sends a submit interaction. Use `.unwrap_or_default()` defensively.

**`ME` (the bot's own `UserId`)** is populated in `ready` via `OnceLock`. It is safe to read in `message` because `message` events can't arrive before `ready`.

## Logging

Use `discord_log!(ctx, "format {}", args).await` anywhere you have a `Context`. It sends to the configured Discord channel, or falls back to `eprintln!` if the channel isn't initialized. Use `eprintln!` directly where there's no context (startup errors, Ctrl+C handler).

## Formatting

`.rustfmt.toml` is configured with `imports_granularity = "Crate"` and `group_imports = "StdExternalCrate"`. Run `cargo fmt` before committing — a pre-commit hook enforces this and will reject the commit if formatting is off.
