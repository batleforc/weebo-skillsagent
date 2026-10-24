import type { On } from 'claude-code'
import { expect, mock, test } from 'claude-code/testing'

import { isEmpty, loadNotes, mergeNotes, purgeKeys } from '../hooks/notes'

const PANE = {
  plugin: 'weebo-hud',
  surface: 'terminal',
  component: 'Pane',
  requestId: 'weebo-hud',
  props: {
    title: 'Reprise',
    isFocused: false,
    bodyColumns: 60,
    placement: 'dock',
    scroll: { offset: 0, bodyRows: 20 },
    view: {},
  },
} as const

const RUN = { args: '', origin: { kind: 'user' }, presentation: { kind: 'composer' } } as any
let up = false
let idFails = false
const OPEN = { id: 'weebo-hud', title: 'Reprise', isShown: true, isFocused: false, isPlaced: true }
const SID = 'S1'
let sid = SID
let registered: string[] = []

// Stands for the engine and the world beneath the mod.
const world = (on: On, entries?: Record<string, unknown>) => {
  const opens: unknown[] = []
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  idFails = false
  sid = SID
  registered = []
  on('session.id', () => {
    if (idFails) throw new Error('no id')

    return { value: sid }
  })
  on('session.cwd', () => ({ value: '/x' }))
  on('env.get', () => ({ value: '/home/x' }))
  on('fs.read', () => ({ value: 'MemTotal: 2048 kB\nMemAvailable: 1024 kB\n' }))
  on('process.run', () => ({
    value: { exitCode: 0, stdout: 'x', stderr: '', isStdoutTruncated: false, isStderrTruncated: false },
  }))
  on('command.register', () => {
    registered.push('etat')

    return { value: { command: 'etat' } }
  })
  on('tool.register', () => {
    registered.push('set_status')

    return { value: { tool: 'mcp__weebo-hud__set_status' } }
  })
  on('ui.open', (_, e) => {
    opens.push(e)

    return { value: { isPlaced: true } }
  })
  up = false
  on('ui.panes', () => ({ value: up ? [OPEN] : [] }))
  on('ui.close', () => {
    up = false

    return { value: undefined }
  })
  mock.clock(on, { now: 10 })
  mock.store(on, entries)

  return opens
}
const start = ($: any) => $.session.start({ cwd: '/', surface: 'terminal', isInteractive: true })

test('loadNotes normalises', () => {
  expect(loadNotes('x')).toBe(null)
  expect(loadNotes({ objectif: 'A' })).toEqual({ objectif: 'A', notes: '', attente: '', todos: [], updatedAt: 0 })
})

test('mergeNotes clears a given empty field and keeps the others', () => {
  const n = mergeNotes(null, { objectif: 'A', notes: 'n' }, 1)
  expect(mergeNotes(n, { notes: '' }, 5)).toEqual({ ...n, notes: '', updatedAt: 5 })
  expect(isEmpty(mergeNotes(null, {}, 1))).toBe(true)
})

test('purgeKeys keeps fresh and foreign keys', () => {
  const now = 31 * 24 * 3600 * 1000
  expect(
    purgeKeys([['session:a', { updatedAt: 0 }], ['session:b', { updatedAt: now }], ['other', {}]], now),
  ).toEqual(['session:a'])
})

test('set_status fills the pane', async ($, on) => {
  world(on)
  await start($)
  await $.tool.call({ tool: 'mcp__weebo-hud__set_status', objectif: 'Livrer le HUD', attente: 'Valider Q3' })

  const ui = await $.ui.mount(PANE)
  expect((await ui.find({ type: 'Text', text: /Livrer le HUD/ }))?.text).toMatch(/Livrer le HUD/)
  expect((await ui.find({ type: 'Text', text: /En attente/ }))?.text).toMatch(/En attente/)
  await ui.unmount()
})

test('TodoWrite is mirrored', async ($, on) => {
  world(on)
  on('tool.call', () => ({ result: {} }))
  await start($)
  await $.tool.call({
    tool: 'TodoWrite',
    todos: [
      { content: 'A', status: 'completed', activeForm: 'A…' },
      { content: 'B', status: 'in_progress', activeForm: 'Faire B' },
    ],
  })

  const ui = await $.ui.mount(PANE)
  expect((await ui.find({ type: 'Text', text: /✓ A/ }))?.text).toMatch(/✓ A/)
  expect((await ui.find({ type: 'Text', text: /▸ Faire B/ }))?.text).toMatch(/▸ Faire B/)
  await ui.unmount()
})

test('a stored session resumes and opens the pane', async ($, on) => {
  const opens = world(on, { [`session:${SID}`]: { objectif: 'Reprendre ici', updatedAt: 1 } })
  await start($)

  const ui = await $.ui.mount(PANE)
  expect((await ui.find({ type: 'Text', text: /Reprendre ici/ }))?.text).toMatch(/Reprendre ici/)
  await ui.unmount()
  expect(opens).toHaveLength(1)
})

test('paneAuto off keeps the pane shut', { options: { paneAuto: false } }, async ($, on) => {
  const opens = world(on, { [`session:${SID}`]: { objectif: 'Reprendre ici', updatedAt: 1 } })
  await start($)
  expect(opens).toHaveLength(0)
})

test('/etat opens then closes', async ($, on) => {
  const opens = world(on)
  await start($)
  const a = await $.command.run({ command: 'etat', ...RUN })
  expect(a.text).toMatch(/ouvert/)
  expect(opens).toHaveLength(1)
  up = true
  const b = await $.command.run({ command: 'etat', ...RUN })
  expect(b.text).toMatch(/ferm/)
})

test('a TodoWrite the engine failed leaves the plan and the result alone', async ($, on) => {
  world(on)
  on('tool.call', () => ({ result: 'boom', isError: true }))
  await start($)
  const ran = await $.tool.call({ tool: 'TodoWrite', todos: [{ content: 'A', status: 'pending', activeForm: 'A' }] })
  expect(ran).toMatchObject({ result: 'boom', isError: true })

  const ui = await $.ui.mount(PANE)
  expect((await ui.find({ type: 'Text', text: /Aucune note/ }))?.text).toMatch(/Aucune note/)
  await ui.unmount()
})

test('set_status before the load keeps the stored objectif', async ($, on) => {
  world(on, { [`session:${SID}`]: { objectif: 'Gardé', updatedAt: 1 } })
  // No session.start: the atom is still null.
  await $.tool.call({ tool: 'mcp__weebo-hud__set_status', attente: 'Réponse' })

  const ui = await $.ui.mount(PANE)
  expect((await ui.find({ type: 'Text', text: /Gardé/ }))?.text).toMatch(/Gardé/)
  expect((await ui.find({ type: 'Text', text: /Réponse/ }))?.text).toMatch(/Réponse/)
  await ui.unmount()
})

test('a failing save does not break TodoWrite', async ($, on) => {
  world(on)
  on('tool.call', () => ({ result: { ok: 1 } }))
  await start($)
  idFails = true // save() rejects after next(): stands for any store/ui failure
  const ran = await $.tool.call({ tool: 'TodoWrite', todos: [{ content: 'A', status: 'pending', activeForm: 'A' }] })
  expect(ran).toMatchObject({ result: { ok: 1 } })
})

test('auto-open fires once: a closed pane stays closed', async ($, on) => {
  const opens = world(on)
  await start($)
  await $.tool.call({ tool: 'mcp__weebo-hud__set_status', objectif: 'Un' })
  expect(opens).toHaveLength(1)
  up = false // the user closed it
  await $.tool.call({ tool: 'mcp__weebo-hud__set_status', objectif: 'Deux' })
  expect(opens).toHaveLength(1)
})

const text = async (ui: any, re: RegExp) => (await ui.find({ type: 'Text', text: re }))?.text

test('a session switch does not carry the old objectif into the new session', async ($, on) => {
  world(on)
  await start($)
  await $.tool.call({ tool: 'mcp__weebo-hud__set_status', objectif: 'Objectif A' })
  sid = 'B' // /clear: new id, no session.start
  await $.tool.call({ tool: 'mcp__weebo-hud__set_status', attente: 'Réponse B' })

  const ui = await $.ui.mount(PANE)
  expect(await text(ui, /Objectif A/)).toBeUndefined()
  expect(await text(ui, /Réponse B/)).toMatch(/Réponse B/)
  await ui.unmount()
})

test('a session switch keeps the new session\'s stored notes and never writes under the old key', async ($, on) => {
  world(on, { 'session:B': { objectif: 'Objectif B', updatedAt: 1 } })
  await start($)
  await $.tool.call({ tool: 'mcp__weebo-hud__set_status', objectif: 'Objectif A' })
  sid = 'B' // /resume B in the REPL

  // Render alone (no save yet) already shows B's stored notes.
  let ui = await $.ui.mount(PANE)
  expect(await text(ui, /Objectif B/)).toMatch(/Objectif B/)
  await ui.unmount()

  await $.tool.call({ tool: 'mcp__weebo-hud__set_status', attente: 'Réponse B' })
  ui = await $.ui.mount(PANE)
  expect(await text(ui, /Objectif B/)).toMatch(/Objectif B/)
  expect(await text(ui, /Réponse B/)).toMatch(/Réponse B/)
  await ui.unmount()

  sid = SID // back to A: its key holds only A's notes
  ui = await $.ui.mount(PANE)
  expect(await text(ui, /Objectif A/)).toMatch(/Objectif A/)
  expect(await text(ui, /Réponse B/)).toBeUndefined()
  await ui.unmount()
})

test('session.end lets the pane auto-open again in the next session', async ($, on) => {
  const opens = world(on)
  on('session.end', (_, e) => ({ sessionId: e.sessionId }))
  await start($)
  await $.tool.call({ tool: 'mcp__weebo-hud__set_status', objectif: 'Un' })
  expect(opens).toHaveLength(1)
  up = false
  await $.session.end({ reason: 'clear', sessionId: SID, resume: {} as any })
  sid = 'B'
  await $.tool.call({ tool: 'mcp__weebo-hud__set_status', objectif: 'Deux' })
  expect(opens).toHaveLength(2)
})

const taskTools = (on: On) =>
  on('tool.call', (_, e: any) =>
    e.tool === 'TaskCreate'
      ? ({ result: { task: { id: e.subject === 'Tests' ? '1' : '2', subject: e.subject } } } as any)
      : ({ result: { success: true, taskId: e.taskId, updatedFields: ['status'] } } as any),
  )

test('TaskCreate then TaskUpdate in_progress shows the active form', async ($, on) => {
  world(on)
  taskTools(on)
  await start($)
  await $.tool.call({ tool: 'TaskCreate', subject: 'Tests', description: 'd', activeForm: 'Lancer les tests' })
  await $.tool.call({ tool: 'TaskCreate', subject: 'Docs', description: 'd' })
  let ui = await $.ui.mount(PANE)
  expect(await text(ui, /· Tests/)).toMatch(/· Tests/)
  await ui.unmount()

  await $.tool.call({ tool: 'TaskUpdate', taskId: '1', status: 'in_progress' })
  await $.tool.call({ tool: 'TaskUpdate', taskId: '2', status: 'in_progress' })
  ui = await $.ui.mount(PANE)
  expect(await text(ui, /▸ Lancer les tests/)).toMatch(/▸ Lancer les tests/)
  expect(await text(ui, /▸ Docs/)).toMatch(/▸ Docs/) // no activeForm: the subject
  await ui.unmount()

  await $.tool.call({ tool: 'TaskUpdate', taskId: '1', status: 'completed' })
  await $.tool.call({ tool: 'TaskUpdate', taskId: '2', status: 'deleted' })
  ui = await $.ui.mount(PANE)
  expect(await text(ui, /✓ Tests/)).toMatch(/✓ Tests/)
  expect(await text(ui, /Docs/)).toBeUndefined()
  await ui.unmount()
})

test('a failed TaskUpdate leaves the plan and its result alone', async ($, on) => {
  world(on)
  on('tool.call', (_, e: any) =>
    e.tool === 'TaskCreate'
      ? ({ result: { task: { id: '1', subject: e.subject } } } as any)
      : ({ result: { success: false, taskId: '1', updatedFields: [], error: 'nope' } } as any),
  )
  await start($)
  await $.tool.call({ tool: 'TaskCreate', subject: 'Tests', description: 'd' })
  const ran = await $.tool.call({ tool: 'TaskUpdate', taskId: '1', status: 'completed' })
  expect(ran).toMatchObject({ result: { success: false, error: 'nope' } })
  const ui = await $.ui.mount(PANE)
  expect(await text(ui, /· Tests/)).toMatch(/· Tests/)
  await ui.unmount()
})

test('subagent calls never touch the pane', async ($, on) => {
  world(on)
  taskTools(on)
  await start($)
  const r = await $.tool.call({ tool: 'mcp__weebo-hud__set_status', objectif: 'Sous-agent', agentId: 'a1' } as any)
  expect(r).toMatchObject({ result: 'ok' })
  const ran = await $.tool.call({
    tool: 'TodoWrite',
    todos: [{ content: 'X', status: 'pending', activeForm: 'X' }],
    agentId: 'a1',
  } as any)
  expect(ran).toMatchObject({ result: { success: true } })
  await $.tool.call({ tool: 'TaskCreate', subject: 'Tests', description: 'd', agentId: 'a1' } as any)

  const ui = await $.ui.mount(PANE)
  expect(await text(ui, /Aucune note/)).toMatch(/Aucune note/)
  await ui.unmount()
})

test('headless runs register neither the tool nor /etat', async ($, on) => {
  world(on)
  await $.session.start({ cwd: '/', surface: null, isInteractive: false })
  expect(registered).toEqual([])
  await start($)
  expect(registered).toEqual(['set_status', 'etat'])
})
