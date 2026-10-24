import { atom, read, update } from 'claude-code'
import type { EngineInterface, PluginOptions, Register } from 'claude-code'

import type { Notes, Todo } from '../types'
import { isEmpty, loadNotes, mergeNotes, purgeKeys } from './notes'

const notesAtom = atom({ plugin: 'weebo-hud', key: 'notes' } as const, null)

const PANE = { id: 'weebo-hud', title: 'Reprise' }
const TOOL = 'mcp__weebo-hud__set_status'
const WARN = '#d7af00' // orange of "En attente"

// Opens the pane unasked, at most once, when it has content and is not already up.
const autoOpen = async ($: EngineInterface, n: Notes | null, options: PluginOptions, auto: { done: boolean }) => {
  if (auto.done || options.paneAuto === false || isEmpty(n)) return
  if ((await $.ui.panes()).some((p) => p.id === PANE.id)) return
  auto.done = true // once per session: a pane the user closed stays closed until /etat
  await $.ui.open(PANE)
}

const keyOf = (id: string) => `session:${id}`

// Main conversation only: a subagent's calls would clobber the pane.
const fromSubagent = (e: unknown) => !!(e as { agentId?: string }).agentId

const STATUSES = ['pending', 'in_progress', 'completed'] as const
const asStatus = (v: unknown): Todo['status'] | undefined => STATUSES.find((s) => s === v)

// Per-load state, owned by `register`. `synced` names the session the atom's notes belong to:
// /clear and an in-REPL /resume change the id without a session.start, so every save re-checks
// it; `ready` makes parallel saves share one reload.
type Ctx = { auto: { done: boolean }; synced?: { sid: string; ready: Promise<void> } }

const sync = async ($: EngineInterface, ctx: Ctx) => {
  const id = await $.session.id()
  if (ctx.synced?.sid !== id) {
    ctx.auto.done = false
    const load = async () => {
      const mine = loadNotes(await $.store.get(keyOf(id)))
      await update($, notesAtom, () => mine)
    }
    const mine: Ctx['synced'] = {
      sid: id,
      ready: load().catch((err) => {
        if (ctx.synced === mine) ctx.synced = undefined // retry the reload on the next save
        throw err
      }),
    }
    ctx.synced = mine
  }
  await ctx.synced!.ready

  return id
}

// Merges inside the atom's updater (atomic, so parallel saves keep each other's fields), then
// persists under the session the notes were loaded for, never another one's key.
const save = async ($: EngineInterface, ctx: Ctx, patch: Parameters<typeof mergeNotes>[1]) => {
  const id = await sync($, ctx)
  const now = await $.clock.now()
  if (ctx.synced?.sid !== id) throw new Error('session switched mid-save')
  await update($, notesAtom, (cur) => mergeNotes(cur, patch, now))
  const latest = await read($, notesAtom)
  if (ctx.synced?.sid === id && (await $.session.id()) === id) await $.store.set(keyOf(id), latest)

  return latest
}

// Mirrors a successful tool call into the plan; never alters `ran`, never throws.
const mirror = async (
  $: EngineInterface,
  ctx: Ctx,
  options: PluginOptions,
  e: unknown,
  ran: { deny?: string; isError?: boolean },
  todos: Parameters<typeof mergeNotes>[1]['todos'],
) => {
  if (fromSubagent(e) || ran.deny || ran.isError || !todos) return
  try {
    await autoOpen($, await save($, ctx, { todos }), options, ctx.auto)
  } catch {}
}

export const register: Register = (on, options) => {
  const ctx: Ctx = { auto: { done: false } }

  on('session.start', async ($, e, next) => {
    const started = await next(e)

    // Own try: a registration failure must not stop the purge and load below.
    // Headless runs (-p, SDK) get no tool and no command: nobody sees the pane.
    if (e.isInteractive) {
      try {
        await $.tool.register({
          name: 'set_status',
          description:
            "Update the user's resume pane (weebo-hud). Call it when work starts, after each step completed, and whenever you wait on the user. Main conversation only: subagents must not call it. Each field given replaces the stored one; an empty string clears it. Write in the conversation's language.",
          inputSchema: {
            type: 'object',
            properties: {
              objectif: { type: 'string', description: 'One-line goal of the current work.' },
              notes: { type: 'string', description: 'Where things stand and the decisions made.' },
              attente: { type: 'string', description: 'What you are waiting on from the user; empty string clears.' },
            },
            additionalProperties: false,
          },
          isDeferred: false,
        })
        await $.command.register({ name: 'etat', description: 'Ouvrir/fermer le panneau de reprise' })
      } catch {}
    }

    // Purge and load fail apart: a failing purge must not skip the resume load.
    try {
      const now = await $.clock.now()
      const keys = (await $.store.keys()).filter((k) => k.startsWith('session:'))
      const entries = await Promise.all(keys.map(async (k): Promise<[string, unknown]> => [k, await $.store.get(k)]))
      for (const k of purgeKeys(entries, now)) await $.store.delete(k)
    } catch {}
    try {
      ctx.synced = undefined // a start (or reload) always reloads from the store
      await sync($, ctx)
      await autoOpen($, await read($, notesAtom), options, ctx.auto)
    } catch {}

    return started
  })

  // The next save or render re-syncs to the new session.
  on('session.end', async ($, e, next) => {
    ctx.synced = undefined
    ctx.auto.done = false

    return next(e)
  })

  on('tool.call', { tool: TOOL }, async ($, e) => {
    if (fromSubagent(e)) return { result: 'ok' }
    const { objectif, notes, attente } = e as unknown as Record<string, unknown>
    const pick = (v: unknown) => (typeof v === 'string' ? v : undefined)
    // The result is 'ok' whatever the pane does after the write.
    try {
      const saved = await save($, ctx, { objectif: pick(objectif), notes: pick(notes), attente: pick(attente) })
      await autoOpen($, saved, options, ctx.auto).catch(() => undefined)
    } catch {}

    return { result: 'ok' }
  })

  on('tool.call', { tool: 'TodoWrite' }, async ($, e, next) => {
    const ran = await next(e)
    const todos = (e as unknown as { todos?: Todo[] }).todos
    await mirror($, ctx, options, e, ran, Array.isArray(todos) ? todos : undefined)

    return ran
  })

  on('tool.call', { tool: 'TaskCreate' }, async ($, e, next) => {
    const ran = await next(e)
    const id = (ran.result as { task?: { id?: unknown } } | undefined)?.task?.id
    const { subject, activeForm } = e as unknown as { subject?: unknown; activeForm?: unknown }
    const add =
      typeof id === 'string' && typeof subject === 'string'
        ? (cur: Todo[]) => [
            ...cur.filter((t) => t.id !== id),
            { id, content: subject, status: 'pending' as const, activeForm: typeof activeForm === 'string' ? activeForm : '' },
          ]
        : undefined
    await mirror($, ctx, options, e, ran, add)

    return ran
  })

  on('tool.call', { tool: 'TaskUpdate' }, async ($, e, next) => {
    const ran = await next(e)
    const ok = (ran.result as { success?: unknown } | undefined)?.success === true
    const { taskId, status, subject, activeForm } = e as unknown as Record<string, unknown>
    const edit =
      ok && typeof taskId === 'string'
        ? (cur: Todo[]) =>
            status === 'deleted'
              ? cur.filter((t) => t.id !== taskId)
              : cur.map((t) =>
                  t.id !== taskId
                    ? t
                    : {
                        ...t,
                        status: asStatus(status) ?? t.status,
                        content: typeof subject === 'string' ? subject : t.content,
                        activeForm: typeof activeForm === 'string' ? activeForm : t.activeForm,
                      },
                )
        : undefined
    await mirror($, ctx, options, e, ran, edit)

    return ran
  })

  on('command.run', { command: 'etat' }, async ($) => {
    if ((await $.ui.panes()).some((p) => p.id === PANE.id)) {
      await $.ui.close({ id: PANE.id })

      return { text: 'Panneau de reprise fermé' }
    }
    await $.ui.open({ ...PANE, focus: true })

    return { text: 'Panneau de reprise ouvert' }
  })

  on('ui.render', { component: 'Pane', requestId: PANE.id }, async ($, e) => {
    const { Box, Text } = $.ui.resolve(e)
    // Reading the atom subscribes the pane; after a session switch nothing has re-synced it yet,
    // so draw the current session's stored notes instead (render never writes state).
    const fromAtom = await read($, notesAtom)
    const id = await $.session.id().catch(() => undefined)
    const n =
      id !== undefined && id !== ctx.synced?.sid
        ? loadNotes(await $.store.get(keyOf(id)).catch(() => null))
        : fromAtom
    const head = (t: string) => <Text bold>{t}</Text>

    return (
      <Box flexDirection="column" width={e.props.bodyColumns}>
        {isEmpty(n) || !n ? (
          <Text dimColor>Aucune note pour cette session.</Text>
        ) : (
          <Box flexDirection="column">
            {n.objectif && head('Objectif')}
            {n.objectif && <Text>{n.objectif}</Text>}
            {n.todos.length > 0 && head('Plan')}
            {n.todos.map((t, i) => (
              <Text key={String(i)}>
                {t.status === 'completed' ? '✓' : t.status === 'in_progress' ? '▸' : '·'}{' '}
                {t.status === 'in_progress' ? t.activeForm || t.content : t.content}
              </Text>
            ))}
            {n.notes && head('Notes')}
            {n.notes && <Text>{n.notes}</Text>}
            {n.attente && <Text bold color={WARN}>En attente</Text>}
            {n.attente && <Text color={WARN}>{n.attente}</Text>}
          </Box>
        )}
      </Box>
    )
  })
}
