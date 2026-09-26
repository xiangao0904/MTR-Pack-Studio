import test from 'node:test'
import assert from 'node:assert/strict'
import { SaveQueue } from '../src/lib/save-queue.ts'
import { arrangeConsist, matchesExpression, matchesPlacement } from '../src/lib/train-preview.ts'

test('flush drains edits arriving during a write and shares concurrent calls', async () => {
  let release
  const gate = new Promise(resolve => { release = resolve })
  let writes = 0
  const queue = new SaveQueue(async () => { writes++; if (writes === 1) await gate })
  queue.markDirty()
  const first = queue.flush()
  await Promise.resolve()
  queue.markDirty()
  const second = queue.flush()
  assert.equal(first, second)
  release()
  await first
  assert.equal(writes, 2)
  assert.equal(queue.pending, false)
})
test('failed saves remain dirty, propagate rejection, and can be retried', async () => {
  let fail = true
  const queue = new SaveQueue(async () => { if (fail) throw new Error('disk full') })
  queue.markDirty()
  await assert.rejects(queue.flush(), /disk full/)
  assert.equal(queue.pending, true)
  fail = false
  await queue.flush()
  assert.equal(queue.pending, false)
})
test('placement filters handle negative indices, periodic offsets and native filter strengths', () => {
  assert.equal(matchesExpression('-1', 4, 4), true)
  assert.equal(matchesExpression('%2+1', 3, 5), true)
  assert.equal(matchesExpression('%2', 3, 5), false)
  assert.equal(matchesExpression('%0', 2, 5), false)
  assert.equal(matchesExpression('%3+1', 2, 5), true)
  assert.equal(matchesExpression('%3+-1', 4, 5), true)
  assert.equal(matchesPlacement({preset:'custom',whitelist:'1',blacklist:'1',offset:0}, 1, 3), true)
  assert.equal(matchesPlacement({preset:'custom',whitelist:'1',blacklist:'',offset:0}, 2, 3), true)
  assert.equal(matchesPlacement({preset:'custom',whitelist:'1,%2',blacklist:'-1',offset:0}, 6, 6), false)
  assert.equal(matchesPlacement({preset:'every',every:3,offset:1,whitelist:'',blacklist:''}, 4, 6), true)
})
test('consist repeats definitions and reversal swaps coupling ends', () => {
  const a={id:'a',length:20,width:3,couplingPadding1:1,couplingPadding2:2}
  const train={carriages:[a],previewConsist:[{carriageId:'a',reversed:false},{carriageId:'a',reversed:true}]}
  const positions=arrangeConsist(train)
  assert.equal(positions.length,2)
  assert.equal(positions[0].z-positions[1].z,24)
  assert.equal(positions[0].carriage,positions[1].carriage)
  assert.equal(positions[1].reversed,true)
})


test('discard waits for an in-flight save without starting a dirty save', async () => {
  let writes = 0, release
  const gate = new Promise(resolve => { release = resolve })
  const queue = new SaveQueue(async () => { writes++; await gate })
  queue.markDirty()
  await queue.waitForIdle()
  assert.equal(writes, 0)
  const saving = queue.flush()
  await Promise.resolve()
  let idle = false
  const waiting = queue.waitForIdle().then(() => { idle = true })
  await Promise.resolve()
  assert.equal(idle, false)
  release()
  await Promise.all([saving, waiting])
  assert.equal(idle, true)
  assert.equal(writes, 1)
})
