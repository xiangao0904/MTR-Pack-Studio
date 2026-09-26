import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import * as THREE from 'three'
import { OBJLoader } from 'three/examples/jsm/loaders/OBJLoader.js'
import { loadPreviewRail, railSegmentPositions, repeatRailModel } from '../src/lib/preview-rails.ts'
import { builtInRailInterval, builtInRails } from '../src/lib/rail-preview-options.ts'

test('track coverage follows the complete consist in metres and rejects unbounded allocation', () => {
  const positions = railSegmentPositions([{ z: -30, length: 20 }, { z: 0, length: 20 }], .6)
  assert.ok(positions[0] <= -42)
  assert.ok(positions.at(-1) >= 12)
  for (let i=1;i<positions.length;i++) assert.ok(Math.abs(positions[i]-positions[i-1]-.6)<1e-10)
  assert.deepEqual(railSegmentPositions([], .6), [])
  assert.deepEqual(railSegmentPositions([{ z: NaN, length: 20 }], .6), [])
  assert.throws(() => railSegmentPositions([{ z: 0, length: 1e9 }], .6))
})

test('bundled complete rail model repeats along Z without duplicating or scaling the two rails', () => {
  const source = new OBJLoader().parse(fs.readFileSync(new URL('../public/models/rail.obj', import.meta.url), 'utf8'))
  const sourceBounds = new THREE.Box3().setFromObject(source)
  assert.ok(Math.abs(sourceBounds.max.z-sourceBounds.min.z-.6)<1e-6)
  assert.ok(Math.abs(sourceBounds.max.x-sourceBounds.min.x-2.337364)<1e-5)
  const tracks = repeatRailModel(source, [{ z: 10, length: 20 }])
  const bounds = new THREE.Box3().setFromObject(tracks)
  assert.ok(Math.abs(bounds.max.y + 0.75)<1e-7, 'track scenery is lowered by three quarters of a metre')
  assert.ok(bounds.min.z <= -2 && bounds.max.z >= 22)
  assert.equal(source.parent, null, 'source stays separately owned')
  assert.equal(tracks.children[0].children[0].geometry, source.children[0].geometry, 'segments share geometry')
})

test('all bundled rail variants repeat on the same 0.6 metre pitch', () => {
  assert.equal(builtInRailInterval, .6)
  assert.equal(builtInRails.length, 3)
  for (const variant of builtInRails) {
    const source = new OBJLoader().parse(fs.readFileSync(new URL(`../public/models/${variant.model}`, import.meta.url), 'utf8'))
    assert.ok(fs.existsSync(new URL(`../public/models/${variant.texture}`, import.meta.url)))
    const tracks = repeatRailModel(source, [{ z: 0, length: 3 }])
    assert.ok(tracks.children.length > 2, variant.id)
    assert.ok(Math.abs(tracks.children[1].position.z - tracks.children[0].position.z - .6) < 1e-10, variant.id)
    source.traverse(object => { if (object instanceof THREE.Mesh) { object.geometry.dispose(); object.material.dispose() } })
  }
})


test('default rail texture preserves Metasequoia top-left UVs like imported GLB previews', async () => {
  const obj = fs.readFileSync(new URL('../public/models/rail.obj', import.meta.url), 'utf8')
  assert.ok(obj.startsWith('# Created by Metasequoia'))
  const source = new OBJLoader().parse(obj)
  const texture = new THREE.Texture()
  const objLoad = OBJLoader.prototype.loadAsync, textureLoad = THREE.TextureLoader.prototype.loadAsync
  OBJLoader.prototype.loadAsync = async () => source
  THREE.TextureLoader.prototype.loadAsync = async () => texture
  try {
    const loaded = await loadPreviewRail()
    assert.equal(loaded, source)
    assert.equal(texture.flipY, false)
    assert.equal(texture.colorSpace, THREE.SRGBColorSpace)
    loaded.traverse(object => {
      if (object instanceof THREE.Mesh) {
        assert.equal(object.material.map, texture)
        assert.equal(object.castShadow, true)
        assert.equal(object.receiveShadow, true)
      }
    })
  } finally {
    OBJLoader.prototype.loadAsync = objLoad
    THREE.TextureLoader.prototype.loadAsync = textureLoad
    source.traverse(object => { if (object instanceof THREE.Mesh) { object.geometry.dispose(); object.material.dispose() } })
    texture.dispose()
  }
})

test('each bundled rail selects its own model and texture', async () => {
  const requested = []
  const objLoad = OBJLoader.prototype.loadAsync, textureLoad = THREE.TextureLoader.prototype.loadAsync
  OBJLoader.prototype.loadAsync = async url => { requested.push(url); return new THREE.Group() }
  THREE.TextureLoader.prototype.loadAsync = async url => { requested.push(url); return new THREE.Texture() }
  try {
    for (const variant of builtInRails) await loadPreviewRail(variant.id)
    assert.deepEqual(requested, builtInRails.flatMap(variant => [`/models/${variant.model}`, `/models/${variant.texture}`]))
  } finally {
    OBJLoader.prototype.loadAsync = objLoad
    THREE.TextureLoader.prototype.loadAsync = textureLoad
  }
})


test('project tracks use their repeat interval and preserve their authored origin', () => {
  const source = new THREE.Group()
  const mesh = new THREE.Mesh(new THREE.BoxGeometry(2, .2, .6))
  mesh.position.set(.4, .3, .1)
  source.add(mesh)
  const tracks = repeatRailModel(source, [{z:0,length:6}], 1.25)
  assert.equal(tracks.position.y, -0.75, 'project tracks receive the same scene offset')
  assert.ok(tracks.children.length > 2)
  assert.equal(tracks.children[1].position.z - tracks.children[0].position.z, 1.25)
  assert.equal(tracks.children[0].position.x, 0)
  assert.equal(tracks.children[0].position.y, 0)
  assert.equal(tracks.children[0].children[0].position.y, .3)
  mesh.geometry.dispose(); mesh.material.dispose()
})
