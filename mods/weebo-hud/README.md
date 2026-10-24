# weebo-hud

A Claude Code mod: the **resume pane**, a pane written in French (objectif, plan, notes, en attente) that Claude keeps current, so you can pick a session back up at a glance. `/etat` toggles it.

The status line under the prompt (folder, model, context, quotas, free RAM, CPU and disk) is not in this mod: a plugin cannot set `statusLine`. It comes from `wska hud statusline`, installed by `wska hud install`.

Install both at user scope:

```bash
task skills:install ITEM=weebo-hud SCOPE=user
# or by hand: wska hud install && claude plugin install weebo-hud@weebo --scope user
```

Full documentation is in [docs/content/docs/weebo-hud.md](../../docs/content/docs/weebo-hud.md). The design is in [the spec](../../docs/content/docs/superpowers/specs/2026-10-24-weebo-hud-design.md).
