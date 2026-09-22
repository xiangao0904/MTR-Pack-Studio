import test from 'node:test'
import assert from 'node:assert/strict'
import { defaultViewportSettings, normalizeViewportSettings } from '../src/lib/viewport-settings.ts'

test('damaged viewport preferences restore safe defaults and clamp light controls', () => {
  const defaults = defaultViewportSettings('material')
  assert.deepEqual(normalizeViewportSettings('material', null), defaults)
  const recovered = normalizeViewportSettings('material', {lightElevation:-40, lightIntensity:Infinity, environmentIntensity:100, shadowQuality:'huge', ground:false})
  assert.equal(recovered.lightElevation,10)
  assert.equal(recovered.lightIntensity,defaults.lightIntensity)
  assert.equal(recovered.environmentIntensity,3)
  assert.equal(recovered.shadowQuality,'standard')
  assert.equal(recovered.ground,false)
})

test('mode defaults do not share mutable state', () => {
  const studio=defaultViewportSettings('studio'), minecraft=defaultViewportSettings('minecraft')
  minecraft.lightIntensity=8
  assert.equal(studio.lightIntensity,3)
  assert.equal(defaultViewportSettings('minecraft').lightIntensity,3)
})
