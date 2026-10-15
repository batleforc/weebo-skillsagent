# graphify
- **graphify** - any input to knowledge graph. Trigger: `/graphify`

graphify is an installer item, not vendored here: `task skills:install PROFILE=base`
runs `graphify install --project --platform claude`, which writes
`.claude/skills/graphify/` (gitignored). If that directory is absent, run that task.
When the user types `/graphify`, use the installed graphify skill or instructions before doing anything else.
