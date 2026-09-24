import test from 'node:test'
import assert from 'node:assert/strict'
import { minecraftAtmosphere } from '../src/lib/minecraft-sky.ts'
import { defaultViewportSettings, normalizeViewportSettings } from '../src/lib/viewport-settings.ts'

test('legacy and malformed sky preferences remain finite and bounded',()=>{
  const settings=normalizeViewportSettings('minecraft',{cloudCover:-10,skyHaze:Infinity})
  assert.equal(settings.cloudCover,0);assert.equal(settings.skyHaze,.35)
  assert.equal(normalizeViewportSettings('minecraft',{skyHaze:100,cloudCover:9}).cloudCover,1)
  assert.equal(normalizeViewportSettings('minecraft',{skyHaze:100}).skyHaze,1)
})

test('low sun warms direct light and horizon while keeping a blue upper sky',()=>{
  const settings=defaultViewportSettings('minecraft')
  const day=minecraftAtmosphere({...settings,lightElevation:60}),sunset=minecraftAtmosphere({...settings,lightElevation:10})
  assert.ok(sunset.sun.r/sunset.sun.b>day.sun.r/day.sun.b)
  assert.ok(sunset.horizon.r/sunset.horizon.b>day.horizon.r/day.horizon.b)
  assert.ok(sunset.zenith.b>sunset.zenith.r)
  for(const palette of [day,sunset])for(const color of [palette.sun,palette.horizon,palette.zenith])assert.ok(color.toArray().every(Number.isFinite))
})

test('sky defaults preserve existing indirect illumination and shadow quality',()=>{
  const settings=normalizeViewportSettings('minecraft',{})
  assert.equal(settings.indirectIntensity,1);assert.equal(settings.ambientOcclusion,true)
  assert.equal(settings.shadowQuality,'standard');assert.ok(!('minecraftQuality' in settings))
})
