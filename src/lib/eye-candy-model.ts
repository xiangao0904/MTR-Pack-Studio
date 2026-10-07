import type { AssetDefinition, EyeCandyDefinition, ModelLayer } from './projects'

/** Replacing the one model keeps object placement and compatible part/material edits. */
export function replacementEyeCandyModel(document: EyeCandyDefinition, asset: AssetDefinition): ModelLayer {
  const previous = document.models[0]
  if (!previous) return { id: crypto.randomUUID(), name: asset.name, assetId: asset.id, visible: true, flipTextureV: false, materialBindings: [], partRules: {} }
  const layer: ModelLayer = JSON.parse(JSON.stringify(previous))
  const materials = new Set(asset.materials.map(material => material.id))
  const parts = new Set(asset.parts.map(part => part.id))
  layer.name = asset.name
  layer.assetId = asset.id
  layer.materialBindings = layer.materialBindings.filter(binding => materials.has(binding.materialId))
  layer.hiddenParts = (layer.hiddenParts || []).filter(id => parts.has(id))
  layer.partTransforms = Object.fromEntries(Object.entries(layer.partTransforms || {}).filter(([id]) => parts.has(id)))
  layer.partRenderStages = Object.fromEntries(Object.entries(layer.partRenderStages || {}).filter(([id]) => parts.has(id)))
  layer.partRules = {}
  return layer
}
