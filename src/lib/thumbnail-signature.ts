import type { AssetDefinition, CarriageDefinition, ModelLayer } from './projects'

/** Only model changes affect the saved studio thumbnail. Bump the version when its framing or lighting changes. */
export function carriageThumbnailSignature(carriage: CarriageDefinition, assets: Record<string, AssetDefinition>): string {
  const layer = (item: ModelLayer) => ({
    assetId: item.assetId,
    previewHash: assets[item.assetId]?.previewHash,
    legacyUvCorrection: assets[item.assetId]?.legacyUvCorrection,
    visible: item.visible,
    flipTextureV: item.flipTextureV,
    transform: item.transform,
    partTransforms: item.partTransforms,
    hiddenParts: [...(item.hiddenParts ?? [])].sort(),
    materialBindings: item.materialBindings,
  })
  return JSON.stringify({
    version: 'studio-thumbnail-v2',
    body: carriage.bodyModels.map(layer),
    bogie1: { position: carriage.bogie1Models.length ? carriage.bogie1Position : undefined, layers: carriage.bogie1Models.map(layer) },
    bogie2: { position: carriage.bogie2Models.length ? carriage.bogie2Position : undefined, layers: carriage.bogie2Models.map(layer) },
  })
}
