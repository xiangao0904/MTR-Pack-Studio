/** Tracks preview changes by carriage, independently of which carriage is selected. */
export class ThumbnailRefreshTracker {
  private seen = new Map<string,string>()
  private pending = new Map<string,string>()

  shouldRefresh(carriageId: string | undefined, signature: string | undefined, savedSignature: string | undefined): boolean {
    if (!carriageId || !signature) return false
    const previous = this.seen.get(carriageId)
    if (previous === undefined) {
      this.seen.set(carriageId,signature)
      return false
    }
    if (previous !== signature) {
      this.seen.set(carriageId,signature)
      this.pending.set(carriageId,signature)
    }
    if (savedSignature === signature) this.pending.delete(carriageId)
    return this.pending.get(carriageId) === signature
  }
}
