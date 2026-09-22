import test from 'node:test'
import assert from 'node:assert/strict'
import { buildPartTree, flattenPartTree } from '../src/lib/model-tree.ts'

const parts = [
  { id: 'a', name: 'Doors/Left' },
  { id: 'b', name: 'Doors/Right' },
  { id: 'c', name: 'Body' },
]

test('hierarchy preserves stable part ids and descendant visibility targets', () => {
  const tree = buildPartTree(parts)
  assert.deepEqual(tree[0].partIds, ['a', 'b'])
  assert.deepEqual(flattenPartTree(tree, new Set(['group:/Doors'])).map(n => n.id), ['group:/Doors', 'c'])
  assert.deepEqual(flattenPartTree(tree, new Set()).map(n => n.depth), [0, 1, 1, 0])
})

test('search reveals matches inside collapsed groups without changing collapse state', () => {
  const collapsed = new Set(['group:/Doors'])
  assert.deepEqual(flattenPartTree(buildPartTree(parts), collapsed, 'right').map(n => n.id), ['group:/Doors', 'b'])
  assert.equal(collapsed.has('group:/Doors'), true)
})

test('flat names share groups only when a prefix repeats', () => {
  const tree = buildPartTree([{ id: 'a', name: 'Door_L' }, { id: 'b', name: 'Door_R' }, { id: 'c', name: 'Body_Shell' }])
  assert.deepEqual(tree.map(n => n.name), ['Door', 'Body_Shell'])
  assert.deepEqual(tree[0].partIds, ['a', 'b'])
})
