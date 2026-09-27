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

test('24-hour sky moves through daylight, sunset, and dark night',()=>{
  const settings=defaultViewportSettings('minecraft')
  const day=minecraftAtmosphere({...settings,timeOfDay:12}),sunset=minecraftAtmosphere({...settings,timeOfDay:18}),night=minecraftAtmosphere({...settings,timeOfDay:0})
  assert.ok(sunset.sun.r/sunset.sun.b>day.sun.r/day.sun.b)
  assert.ok(sunset.horizon.r/sunset.horizon.b>day.horizon.r/day.horizon.b)
  assert.ok(sunset.zenith.b>sunset.zenith.r)
  assert.ok(night.zenith.r<day.zenith.r*.2)
  assert.ok(night.environmentFactor<day.environmentFactor*.2)
  assert.ok(night.directFactor<day.directFactor*.1)
  for(const palette of [day,sunset,night])for(const color of [palette.sun,palette.horizon,palette.zenith])assert.ok(color.toArray().every(Number.isFinite))
})

test('red twilight keeps warm light and visible surface illumination at 18:15',()=>{
  const settings=defaultViewportSettings('minecraft')
  const dusk=minecraftAtmosphere({...settings,timeOfDay:18.25})
  const night=minecraftAtmosphere({...settings,timeOfDay:0})
  assert.ok(dusk.horizon.r>dusk.horizon.b*3)
  assert.ok(dusk.sun.r>dusk.sun.b*1.5)
  assert.ok(dusk.directFactor>.35)
  assert.ok(dusk.environmentFactor>.35)
  assert.ok(dusk.fillFactor>.3)
  assert.ok(dusk.illumination>night.illumination+.3)
  assert.ok(night.fillFactor<dusk.fillFactor*.2)
})

test('sky defaults preserve existing indirect illumination and shadow quality',()=>{
  const settings=normalizeViewportSettings('minecraft',{})
  assert.equal(settings.indirectIntensity,1);assert.equal(settings.ambientOcclusion,true)
  assert.equal(settings.shadowQuality,'standard');assert.ok(!('minecraftQuality' in settings))
})
