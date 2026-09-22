import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { bindPreviewTextures } from '../src/lib/preview-glb.ts'

function buffer(file) {
  const bytes = readFileSync(new URL(file, import.meta.url))
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength)
}
function decode(bytes) {
  const header = new DataView(bytes)
  assert.equal(header.getUint32(0, true), 0x46546c67)
  assert.equal(header.getUint32(8, true), bytes.byteLength)
  const length = header.getUint32(12, true)
  assert.equal(length % 4, 0)
  return { document: JSON.parse(new TextDecoder().decode(bytes.slice(20, 20 + length))), binary: bytes.slice(28 + length) }
}

test('browser material replacement embeds the image without changing geometry or other materials', () => {
  const source = buffer('../public/fixtures/studio-train.glb')
  const png = buffer('../public/fixtures/checker.png')
  const before = decode(source)
  const result = decode(bindPreviewTextures(source, [{ materialId: 'material-0', bytes: png }]))
  assert.deepEqual(result.document.meshes, before.document.meshes)
  assert.deepEqual(result.document.nodes, before.document.nodes)
  assert.deepEqual(result.document.materials[1], before.document.materials[1])
  assert.deepEqual(result.binary.slice(0, before.binary.byteLength), before.binary)
  const material = result.document.materials[0]
  const texture = result.document.textures[material.pbrMetallicRoughness.baseColorTexture.index]
  const image = result.document.images[texture.source]
  const view = result.document.bufferViews[image.bufferView]
  assert.equal(view.byteOffset % 4, 0)
  assert.deepEqual(result.binary.slice(view.byteOffset, view.byteOffset + view.byteLength), png)
  assert.equal(image.mimeType, 'image/png')
  assert.equal(bindPreviewTextures(source, []), source)
})
