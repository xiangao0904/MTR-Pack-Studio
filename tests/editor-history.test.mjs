import test from 'node:test'
import assert from 'node:assert/strict'
import { EditorHistory, trainHistorySnapshot } from '../src/lib/editor-history.ts'

test('undo records pending edits and redo restores them', () => {
  const history = new EditorHistory()
  history.reset({ name: 'A' })
  assert.equal(history.canUndo, false)
  assert.equal(history.hasChanges({ name: 'B' }), true)
  assert.deepEqual(history.undo({ name: 'B' }), { name: 'A' })
  assert.equal(history.canRedo, true)
  assert.deepEqual(history.redo({ name: 'A' }), { name: 'B' })
  assert.deepEqual(history.undo({ name: 'B' }), { name: 'A' })
})

test('record boundaries undo in order and a new edit clears redo', () => {
  const history = new EditorHistory()
  history.reset({ n: 0 })
  history.record({ n: 1 })
  history.record({ n: 2 })
  assert.deepEqual(history.undo({ n: 2 }), { n: 1 })
  assert.deepEqual(history.undo({ n: 1 }), { n: 0 })
  assert.deepEqual(history.redo({ n: 0 }), { n: 1 })
  history.record({ n: 3 })
  assert.equal(history.canRedo, false)
  assert.deepEqual(history.undo({ n: 3 }), { n: 1 })
})

test('unrecorded edits invalidate an old redo branch', () => {
  const history = new EditorHistory()
  history.reset({ n: 0 })
  history.record({ n: 1 })
  history.undo({ n: 1 })
  assert.equal(history.redo({ n: 2 }), undefined)
  assert.equal(history.canRedo, false)
  assert.deepEqual(history.undo({ n: 2 }), { n: 0 })
  assert.deepEqual(history.redo({ n: 0 }), { n: 2 })
})

test('generated metadata and property order do not add history entries', () => {
  const history = new EditorHistory({ normalize: trainHistorySnapshot })
  const initial = { revision: 1, name: 'A', carriages: [{ id: 'a', thumbnailHash: 'old' }] }
  history.reset(initial)
  assert.equal(history.record({ carriages: [{ thumbnailHash: 'new', id: 'a' }], name: 'A', revision: 2 }), false)
  assert.equal(history.hasChanges(initial), false)
  history.record({ ...initial, name: 'B' })
  const restored = history.undo({ ...initial, name: 'B', revision: 3 })
  assert.equal(restored.revision, 0)
  assert.equal(restored.carriages[0].thumbnailHash, undefined)
  assert.equal(initial.carriages[0].thumbnailHash, 'old')
  // Background save metadata must not clear an available redo.
  assert.equal(history.record({ ...initial, revision: 4 }), false)
  assert.equal(history.canRedo, true)
})

test('snapshots are defensive and limit caps undo steps', () => {
  const history = new EditorHistory({ limit: 2 })
  const original = { nested: { n: 0 } }
  history.reset(original)
  original.nested.n = 100
  history.record({ nested: { n: 1 } })
  history.record({ nested: { n: 2 } })
  history.record({ nested: { n: 3 } })
  const restored = history.undo({ nested: { n: 3 } })
  restored.nested.n = 200
  assert.deepEqual(history.undo({ nested: { n: 2 } }), { nested: { n: 1 } })
  assert.equal(history.undo({ nested: { n: 1 } }), undefined)
  history.reset({ nested: { n: 9 } })
  assert.equal(history.canRedo, false)
  assert.equal(history.canUndo, false)
})
