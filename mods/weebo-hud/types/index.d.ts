// `id` only on items mirrored from TaskCreate/TaskUpdate.
export type Todo = { id?: string; content: string; status: 'pending' | 'in_progress' | 'completed'; activeForm: string }
export type Notes = { objectif: string; notes: string; attente: string; todos: Todo[]; updatedAt: number }

declare module 'claude-code' {
  interface PluginState {
    'weebo-hud': { notes: Notes | null }
  }
}
