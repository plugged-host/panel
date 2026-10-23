@AGENTS.md

## Notes for Claude Code

- `.claude/rules/` holds short notes for particular folders (database, models, routes, settings,
  permissions, frontend, strings); they load when you work on matching files. Follow them like the
  rules above.
- Don't open a browser, a headless browser or a dev server to check a UI change. Build and
  typecheck it, then end with a short list of what a person should click through.
- Before writing a feature, read the upstream feature it resembles and its docs page if one exists;
  match its file layout and naming exactly.
- After any change to an upstream file, add it to the change list in `docs/upstream.md`.
- Commit messages: imperative subject, a body that says why. Push after committing.
