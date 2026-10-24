import type { Notes, Todo } from '../types'

const DAY30 = 30 * 24 * 3600 * 1000
const str = (v: unknown) => (typeof v === 'string' ? v : '')

export const loadNotes = (raw: unknown): Notes | null => {
  if (!raw || typeof raw !== 'object') return null
  const r = raw as Record<string, unknown>

  return {
    objectif: str(r.objectif),
    notes: str(r.notes),
    attente: str(r.attente),
    todos: Array.isArray(r.todos) ? (r.todos as Todo[]) : [],
    updatedAt: typeof r.updatedAt === 'number' ? r.updatedAt : 0,
  }
}

export const mergeNotes = (
  cur: Notes | null,
  // A function patch edits the current list (Task* mirror); an array replaces it (TodoWrite).
  patch: Partial<Pick<Notes, 'objectif' | 'notes' | 'attente'>> & { todos?: Todo[] | ((cur: Todo[]) => Todo[]) },
  now: number,
): Notes => {
  const base = cur ?? { objectif: '', notes: '', attente: '', todos: [], updatedAt: 0 }
  const next = { ...base, updatedAt: now }
  for (const k of ['objectif', 'notes', 'attente'] as const) if (typeof patch[k] === 'string') next[k] = patch[k]
  if (patch.todos) next.todos = typeof patch.todos === 'function' ? patch.todos(base.todos) : patch.todos

  return next
}

export const isEmpty = (n: Notes | null): boolean =>
  !n || !(n.objectif || n.notes || n.attente || n.todos.length)

export const purgeKeys = (entries: [string, unknown][], now: number): string[] =>
  entries
    .filter(([k, v]) => k.startsWith('session:') && now - (loadNotes(v)?.updatedAt ?? 0) > DAY30)
    .map(([k]) => k)
