import type { MaterialBinding } from './projects'
/** Browser demo only: embed replacement images using the same material IDs as native GLB output. */
export function bindPreviewTextures(source: ArrayBuffer, replacements: { materialId: string; bytes: ArrayBuffer; channel?: string }[], bindings: MaterialBinding[] = []): ArrayBuffer {
  if (!replacements.length && !bindings.length) return source
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
    const key=replacement.channel || 'baseColor'
    if(key==='baseColor' || key==='metallicRoughness')material.pbrMetallicRoughness[`${key}Texture`]={index:textureIndex}
    else material[`${key}Texture`]={index:textureIndex}
  }
  for (const binding of bindings) {
    const material=document.materials.find((value: {extras?: {materialId?: string}})=>value.extras?.materialId===binding.materialId)
    if(!material)continue
    const p=binding.properties || {},pbr=material.pbrMetallicRoughness
    if(p.metalness!=null)pbr.metallicFactor=p.metalness
    if(p.roughness!=null)pbr.roughnessFactor=p.roughness
    if(p.emissive!=null)material.emissiveFactor=p.emissive
    if(p.opacity!=null){pbr.baseColorFactor ||= [1,1,1,1];pbr.baseColorFactor[3]=p.opacity;if(p.alphaMode==null && p.opacity<1)material.alphaMode='BLEND'}
    if(p.alphaMode!=null)material.alphaMode=p.alphaMode
    if(p.alphaCutoff!=null)material.alphaCutoff=p.alphaCutoff
    if(p.normalScale!=null && material.normalTexture)material.normalTexture.scale=p.normalScale
    if(p.doubleSided!=null)material.doubleSided=p.doubleSided
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

/** Match the native packer: scalar source maps use R; glTF uses G for roughness, B for metalness. */
export async function packMetallicRoughness(metal?: ArrayBuffer, rough?: ArrayBuffer): Promise<ArrayBuffer> {
  const images=await Promise.all([metal,rough].map(bytes=>bytes ? createImageBitmap(new Blob([bytes]), {colorSpaceConversion:'none',premultiplyAlpha:'none'}) : undefined))
  try {
    const width=Math.max(...images.map(image=>image?.width || 1)),height=Math.max(...images.map(image=>image?.height || 1))
    const canvas=new OffscreenCanvas(width,height),context=canvas.getContext('2d')!
    const channels=images.map(image=>{if(!image)return undefined;context.clearRect(0,0,width,height);context.drawImage(image,0,0,width,height);return context.getImageData(0,0,width,height).data})
    const result=context.createImageData(width,height)
    for(let i=0;i<result.data.length;i+=4){result.data[i]=255;result.data[i+1]=channels[1]?.[i] ?? 255;result.data[i+2]=channels[0]?.[i] ?? 255;result.data[i+3]=255}
    context.putImageData(result,0,0)
    return (await canvas.convertToBlob({type:'image/png'})).arrayBuffer()
  } finally {for(const image of images)image?.close()}
}
