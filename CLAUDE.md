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
  cache.rs         — On-disk JSON cache (CacheContainer::load / ::save)
  skills/
    mod.rs         — Skill trait, SkillRegistry, interaction dispatch
    schedule.rs    — /schedule: the reference form workflow
    form/          — reusable spine for menu/form workflows
      mod.rs         — Input: a decoded interaction; re-exports
      screen.rs      — Screen vocabulary + into_response() (the ONLY serenity-builder code)
      route.rs       — Route: typed custom_id encode/decode
      item.rs        — FormItem trait (per-record field/parse knowledge)
      collection.rs  — FormCollection<T>: cache-backed storage + Effect hook
```

## Skills

A skill implements the `Skill` trait (`skills/mod.rs`): `name()`, `register() -> CreateCommand`, and optional `command` / `component` / `modal` async handlers (each defaults to an error). Skills are constructed in `SkillRegistry::new`, exposed to Discord via `all_commands()`, and routed in `dispatch()`:

- `Interaction::Command` routes by `command.data.name`.
- `Interaction::Component` / `Interaction::Modal` route by the **first segment of the custom_id**, decoded via `Route` (`form/route.rs`). Every component/modal custom_id a skill emits must therefore be `"<name>/..."` — `skill_for_id` matches that first segment against the registry. (No more prefix-matching or `:`-smuggling of payloads in ids.)

### Adding a basic skill

1. Create `src/skills/<name>.rs` with a struct implementing `Skill` (`const NAME`, `name()`, `register()`, plus whichever handlers you need).
2. Add `pub mod <name>;` to `skills/mod.rs`.
3. Add `Box::new(<Name>::new(ctx).await)` to the `vec!` in `SkillRegistry::new`.

`all_commands()` and `dispatch()` pick it up from the registry automatically.

### Form workflows (the `form` module)

`/schedule` is the reference. Multi-step menu/form skills are modeled as a state machine; the `form` module is the reusable spine and confines all serenity rendering to one place.

- **`Step`** (skill-specific, in your skill file) enumerates the workflow states. Provide:
  - `route()` / `from_route()` — the bijection between steps and custom_ids.
  - `advance(self, &Input) -> Next` — a **pure** transition. `Next::Show(step)` renders a step, `Next::Run(Effect)` requests a mutation, `Next::Reply(msg)` is a terminal message. Keep mutation out of here.
  - `screen(&self, &[T]) -> Screen` — builds the UI for renderable states.
- **`FormItem`** (impl for your record type): `blank_fields`/`edit_fields` (feed the modal), `label` (menu option + the key `Effect::Update` matches on), `summary` (confirmation text), `create`/`update` (parse `Input` → record; parse before mutating so a bad field can't half-update a persisted record).
- **`FormCollection<T>`** holds the records — load with `FormCollection::load(ctx, cache, file)`. Its `run(ctx, effect, input)` is the side-effect hook: it applies the `Effect` and **persists after every mutation** (save runs while the lock is held; a save failure is logged, not fatal).
- The skill's handlers just decode an `Input` (`Input::from_component` / `from_modal`) and drive it: `from_route` → `advance` → match `Next` (call `screen`, or `collection.run`) → `Screen::into_response()`.

Key boundary: workflow code never constructs serenity components — it produces a `Screen` (`form/screen.rs`), and `Screen::into_response()` is the only code that touches `CreateButton`/`CreateModal`/`CreateSelectMenu`.

## Serenity gotchas

**`CreateInputText::new` argument order is `(style, label, custom_id)`** — label and custom_id look similar and are easy to swap. Swapping them causes modal field values to always come back empty because Discord echoes back the custom_id set at send time. (Lives in `form/screen.rs`, `Field::into_row`.)

**`MESSAGE_CODE_LIMIT` is 2000 Unicode code points**, not bytes. `discord_log!` truncates automatically using `char_indices`, not byte length.

**`Command::set_global_commands` replaces all global commands atomically** on every `ready` event (including reconnects). This is intentional — commands always stay in sync with what's registered in `all_commands()`.

**Modal submit values** arrive as `Option<String>` on `InputText.value`, but are always `Some` when Discord sends a submit interaction. Use `.unwrap_or_default()` defensively.

**`ME` (the bot's own `UserId`)** is populated in `ready` via `OnceLock`. It is safe to read in `message` because `message` events can't arrive before `ready`.

## Logging

Use `discord_log!(ctx, "format {}", args).await` anywhere you have a `Context`. It sends to the configured Discord channel, or falls back to `eprintln!` if the channel isn't initialized. Use `eprintln!` directly where there's no context (startup errors, Ctrl+C handler).

## Formatting

`.rustfmt.toml` is configured with `imports_granularity = "Crate"` and `group_imports = "StdExternalCrate"`. Run `cargo fmt` before committing — a pre-commit hook enforces this and will reject the commit if formatting is off.
