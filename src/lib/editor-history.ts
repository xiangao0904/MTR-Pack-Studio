/** History stores JSON documents, not reactive objects or backend save revisions. */
export interface EditorHistoryOptions<T> {
  normalize?: (snapshot: T) => T
  /** Maximum undo steps, excluding the current document. */
  limit?: number
}

function copy<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T
}

function key(value: unknown): string {
  return JSON.stringify(value, (_name, child: unknown) => {
    if (child && typeof child === 'object' && !Array.isArray(child)) {
      return Object.fromEntries(Object.entries(child).sort(([a], [b]) => a.localeCompare(b)))
    }
    return child
  })
}

/**
 * Call reset on load and record at an edit boundary (debounce or immediate action).
 * Undo also handles edits made since the last record. For a reactive UI, include
 * hasChanges(current) alongside canUndo, and update a reactive tick after mutations.
 * Applying a returned document must not itself call record.
 */
export class EditorHistory<T> {
  private past: T[] = []
  private future: T[] = []
  private readonly normalize: (snapshot: T) => T
  private readonly limit: number

  constructor(options: EditorHistoryOptions<T> = {}) {
    this.normalize = options.normalize ?? (value => value)
    this.limit = Math.max(1, Math.floor(options.limit ?? 100))
  }

  private snapshot(value: T): T { return copy(this.normalize(copy(value))) }
  private equal(a: T, b: T): boolean { return key(a) === key(b) }
  private append(value: T): void {
    this.past.push(value)
    if (this.past.length > this.limit + 1) this.past.shift()
  }

  reset(document: T): void {
    this.past = [this.snapshot(document)]
    this.future = []
  }

  get canUndo(): boolean { return this.past.length > 1 }
  get canRedo(): boolean { return this.future.length > 0 }

  hasChanges(current: T): boolean {
    const latest = this.past.at(-1)
    return latest !== undefined && !this.equal(latest, this.snapshot(current))
  }

  record(document: T): boolean {
    const value = this.snapshot(document)
    const latest = this.past.at(-1)
    if (latest !== undefined && this.equal(latest, value)) return false
    this.append(value)
    this.future = []
    return true
  }

  undo(current: T): T | undefined {
    const latest = this.past.at(-1)
    if (latest === undefined) return undefined
    const value = this.snapshot(current)
    if (!this.equal(latest, value)) {
      // A fresh, unrecorded edit starts a new branch, even after an earlier undo.
      this.future = [value]
      return copy(latest)
    }
    if (!this.canUndo) return undefined
    this.future.push(this.past.pop()!)
    return copy(this.past.at(-1)!)
  }

  redo(current: T): T | undefined {
    if (this.hasChanges(current)) {
      this.future = []
      return undefined
    }
    const value = this.future.pop()
    if (value === undefined) return undefined
    this.append(value)
    return copy(value)
  }
}

/** Generated thumbnails and server revisions do not represent user edits. */
export function trainHistorySnapshot<T extends {
  revision?: number
  carriages?: Array<{ thumbnailHash?: string | null }>
}>(document: T): T {
  const snapshot = copy(document)
  if ('revision' in snapshot) snapshot.revision = 0
  for (const carriage of snapshot.carriages ?? []) delete carriage.thumbnailHash
  return snapshot
}
