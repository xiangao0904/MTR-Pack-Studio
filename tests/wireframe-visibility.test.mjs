import test from 'node:test'
import assert from 'node:assert/strict'
import { wireframeOpacity } from '../src/lib/wireframe-visibility.ts'

test('dense wireframes fade as projected edges crowd together', () => {
  const triangles = 20_000
  assert.ok(wireframeOpacity(900, triangles) > .7)
  assert.ok(wireframeOpacity(250, triangles) < wireframeOpacity(900, triangles))
  assert.equal(wireframeOpacity(100, triangles), 0)
  assert.ok(wireframeOpacity(250, 200) > wireframeOpacity(250, triangles))
  assert.equal(wireframeOpacity(NaN, triangles), 0)
})
