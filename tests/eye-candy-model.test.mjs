import test from 'node:test'
import assert from 'node:assert/strict'
import { replacementEyeCandyModel } from '../src/lib/eye-candy-model.ts'

test('choosing a replacement keeps placement and compatible edits without mutating the old model', () => {
  const transform = { translation: [1, 2, 3], rotation: [0, 0.5, 0], scale: [2, 2, 2] }
  const old = {
    id: 'layer', name: 'Old', assetId: 'old', visible: false, flipTextureV: true,
    transform, renderStage: 'LIGHT',
    hiddenParts: ['shared', 'removed'],
    partTransforms: { shared: transform, removed: transform },
    partRenderStages: { shared: 'LIGHT', removed: 'LIGHT' },
    partRules: { shared: { preset: 'first' } },
    materialBindings: [{ materialId: 'shared', textureAssetId: 'texture' }, { materialId: 'removed' }],
  }
  const document = { models: [old, { ...old, id: 'second' }] }
  const before = structuredClone(document)
  const replacement = replacementEyeCandyModel(document, {
    id: 'new', name: 'New', materials: [{ id: 'shared' }], parts: [{ id: 'shared' }],
  })
  assert.equal(replacement.id, old.id)
  assert.equal(replacement.assetId, 'new')
  assert.equal(replacement.name, 'New')
  assert.deepEqual(replacement.transform, transform)
  assert.equal(replacement.visible, false)
  assert.equal(replacement.flipTextureV, true)
  assert.equal(replacement.renderStage, 'LIGHT')
  assert.deepEqual(replacement.hiddenParts, ['shared'])
  assert.deepEqual(replacement.partTransforms, { shared: transform })
  assert.deepEqual(replacement.partRenderStages, { shared: 'LIGHT' })
  assert.deepEqual(replacement.materialBindings, [{ materialId: 'shared', textureAssetId: 'texture' }])
  assert.deepEqual(replacement.partRules, {})
  assert.deepEqual(document, before)
  replacement.transform.translation[0] = 99
  assert.equal(old.transform.translation[0], 1)
})

test('an empty eye candy receives a visible model with no inherited overrides', () => {
  const model = replacementEyeCandyModel({ models: [] }, { id: 'asset', name: 'Bench' })
  assert.ok(model.id)
  assert.equal(model.assetId, 'asset')
  assert.equal(model.name, 'Bench')
  assert.equal(model.visible, true)
  assert.equal(model.flipTextureV, false)
  assert.deepEqual(model.materialBindings, [])
  assert.deepEqual(model.partRules, {})
})
