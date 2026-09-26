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
  const source = buffer('./fixtures/preview-glb/studio-train.glb')
  const png = buffer('./fixtures/preview-glb/checker.png')
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

test('PBR edits preserve geometry and independent materials while setting data and color channels', () => {
  const source=buffer('./fixtures/preview-glb/studio-train.glb'), png=buffer('./fixtures/preview-glb/checker.png')
  const before=decode(source)
  const result=decode(bindPreviewTextures(source,[{materialId:'material-0',channel:'normal',bytes:png},{materialId:'material-0',channel:'metallicRoughness',bytes:png},{materialId:'material-0',channel:'emissive',bytes:png}], [{materialId:'material-0',properties:{metalness:.9,roughness:.2,emissive:[.4,.2,.1],opacity:.5,alphaMode:'BLEND',normalScale:.75,doubleSided:false}}]))
  const material=result.document.materials[0]
  assert.equal(material.pbrMetallicRoughness.metallicFactor,.9)
  assert.equal(material.pbrMetallicRoughness.roughnessFactor,.2)
  assert.equal(material.pbrMetallicRoughness.baseColorFactor[3],.5)
  assert.equal(material.alphaMode,'BLEND')
  assert.equal(material.normalTexture.scale,.75)
  assert.equal(material.doubleSided,false)
  assert.deepEqual(material.emissiveFactor,[.4,.2,.1])
  assert.ok(material.emissiveTexture)
  assert.ok(material.pbrMetallicRoughness.metallicRoughnessTexture)
  assert.deepEqual(result.document.meshes,before.document.meshes)
  assert.deepEqual(result.document.materials[1],before.document.materials[1])
  assert.deepEqual(decode(bindPreviewTextures(source,[],[])),before)
})
