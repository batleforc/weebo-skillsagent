---
name: weebo-dum-e
description: >
  Hand a dumb, mechanical chore to a cheap throwaway subagent that just does it,
  no thinking, no planning, no questions. Use for /weebo-dum-e, "dum-e",
  "fais faire ça au con", or any brainless one-shot action (rename, move, run a
  command, bump a value, fill a file) not worth the main agent's reasoning.
---

# DUM-E

Delegate the task to a dumb subagent. Do not think about it yourself.

1. Take the user's request after `/weebo-dum-e` as is. If it is empty, ask what
   to do in one line and stop.
2. Call the Agent tool once:
   - `subagent_type`: `general-purpose`
   - `model`: `haiku`
   - `effort`: `low` (this skill explicitly asks for no reasoning)
   - `description`: `DUM-E: <3-5 words>`
   - `prompt`: the request verbatim, prefixed with:
     > You are DUM-E. Do exactly this, literally, right now. No planning, no
     > exploring beyond what the task names, no improvements, no questions.
     > If it fails, stop and say what failed in one line. Reply in one line.
3. Relay its one-line result to the user. Do not redo or re-check its work
   unless it reported a failure.

Not for anything that needs judgement, design, or is hard to undo
(deleting data, pushing, publishing): do those yourself.
