/** Serializes snapshots. A flush waits for edits arriving during the current write too. */
export class SaveQueue {
  private dirty = 0
  private saved = 0
  private running: Promise<void> | undefined
  private readonly write: () => Promise<void>
  constructor(write: () => Promise<void>) { this.write = write }
  markDirty() { this.dirty += 1 }
  get pending() { return this.dirty !== this.saved }
  /** Wait for an existing write before discarding edits, without starting a save. */
  waitForIdle(): Promise<void> { return this.running ?? Promise.resolve() }
  flush(): Promise<void> {
    if (this.running) return this.running
    this.running = Promise.resolve().then(() => this.drain()).finally(() => { this.running = undefined })
    return this.running
  }
  private async drain() {
    while (this.pending) {
      const version = this.dirty
      await this.write()
      this.saved = version
    }
  }
}
