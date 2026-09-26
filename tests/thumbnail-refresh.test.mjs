import test from 'node:test'
import assert from 'node:assert/strict'
import { ThumbnailRefreshTracker } from '../src/lib/thumbnail-refresh.ts'

test('carriage selection never schedules a thumbnail without a model change', () => {
  const tracker = new ThumbnailRefreshTracker()
  assert.equal(tracker.shouldRefresh('a','model-a',undefined),false)
  assert.equal(tracker.shouldRefresh('b','model-b',undefined),false)
  assert.equal(tracker.shouldRefresh('a','model-a',undefined),false)
  assert.equal(tracker.shouldRefresh('b','model-b',undefined),false)
})

test('an edited carriage keeps its pending thumbnail across selection changes', () => {
  const tracker = new ThumbnailRefreshTracker()
  tracker.shouldRefresh('a','old',undefined)
  assert.equal(tracker.shouldRefresh('a','new','old'),true)
  assert.equal(tracker.shouldRefresh('b','other',undefined),false)
  assert.equal(tracker.shouldRefresh('a','new','old'),true)
  assert.equal(tracker.shouldRefresh('a','new','new'),false)
  assert.equal(tracker.shouldRefresh('a','new','new'),false)
})

test('initial asset metadata and reverting an edit do not request capture', () => {
  const tracker = new ThumbnailRefreshTracker()
  assert.equal(tracker.shouldRefresh('a',undefined,undefined),false)
  assert.equal(tracker.shouldRefresh('a','original','original'),false)
  assert.equal(tracker.shouldRefresh('a','edited','original'),true)
  assert.equal(tracker.shouldRefresh('a','original','original'),false)
})
