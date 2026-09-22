/** Browser demo only: embed replacement images using the same material IDs as native GLB output. */
export function bindPreviewTextures(source: ArrayBuffer, replacements: { materialId: string; bytes: ArrayBuffer }[]): ArrayBuffer {
  if (!replacements.length) return source
  const header = new DataView(source)
  const jsonLength = header.getUint32(12, true)
  const document = JSON.parse(new TextDecoder().decode(new Uint8Array(source, 20, jsonLength)))
  const binaryLength = header.getUint32(20 + jsonLength, true)
  const chunks = [new Uint8Array(source, 28 + jsonLength, binaryLength)]
  let offset = binaryLength
  document.images ||= []; document.textures ||= []
  for (const replacement of replacements) {
    const material = document.materials.find((value: { extras?: { materialId?: string } }) => value.extras?.materialId === replacement.materialId)
    if (!material) continue
    const padding = (4 - offset % 4) % 4; chunks.push(new Uint8Array(padding)); offset += padding
    const view = document.bufferViews.length
    document.bufferViews.push({ buffer: 0, byteOffset: offset, byteLength: replacement.bytes.byteLength })
    chunks.push(new Uint8Array(replacement.bytes)); offset += replacement.bytes.byteLength
    const imageIndex = document.images.length; document.images.push({ bufferView: view, mimeType: 'image/png' })
    const textureIndex = document.textures.length; document.textures.push({ source: imageIndex })
    material.pbrMetallicRoughness.baseColorTexture = { index: textureIndex }
  }
  document.buffers[0].byteLength = offset
  const json = new TextEncoder().encode(JSON.stringify(document))
  const paddedJsonLength = Math.ceil(json.length / 4) * 4
  const paddedBinaryLength = Math.ceil(offset / 4) * 4
  const output = new ArrayBuffer(28 + paddedJsonLength + paddedBinaryLength)
  const view = new DataView(output); const bytes = new Uint8Array(output)
  view.setUint32(0, 0x46546c67, true); view.setUint32(4, 2, true); view.setUint32(8, output.byteLength, true)
  view.setUint32(12, paddedJsonLength, true); view.setUint32(16, 0x4e4f534a, true)
  bytes.fill(32, 20, 20 + paddedJsonLength); bytes.set(json, 20)
  view.setUint32(20 + paddedJsonLength, paddedBinaryLength, true); view.setUint32(24 + paddedJsonLength, 0x004e4942, true)
  let cursor = 28 + paddedJsonLength
  for (const chunk of chunks) { bytes.set(chunk, cursor); cursor += chunk.length }
  return output
}
