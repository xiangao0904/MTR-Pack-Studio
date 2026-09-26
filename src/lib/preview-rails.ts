import * as THREE from 'three'
import { OBJLoader } from 'three/examples/jsm/loaders/OBJLoader.js'

export interface RailGuide { z: number; length: number }

/** Align repeated 0.6 m source segments on a fixed world grid, with a 2 m apron. */
export function railSegmentPositions(guides: RailGuide[], segmentLength: number): number[] {
  if (!Number.isFinite(segmentLength) || segmentLength <= 0) return []
  const valid = guides.filter(guide => Number.isFinite(guide.z) && Number.isFinite(guide.length) && guide.length > 0)
  if (!valid.length) return []
  const start = Math.floor((Math.min(...valid.map(guide => guide.z - guide.length / 2)) - 2) / segmentLength)
  const end = Math.ceil((Math.max(...valid.map(guide => guide.z + guide.length / 2)) + 2) / segmentLength)
  // Guard pathological documents without silently drawing a truncated track.
  if (end - start > 20000) throw new Error('Track preview exceeds the supported length.')
  return Array.from({ length: end - start + 1 }, (_, index) => (start + index) * segmentLength)
}

/** The supplied OBJ is a complete two-rail/sleeper section, in metres along Z. */
export function repeatRailModel(source: THREE.Group, guides: RailGuide[], repeatInterval?: number): THREE.Group {
  const bounds = new THREE.Box3().setFromObject(source)
  const positions = railSegmentPositions(guides, repeatInterval ?? (bounds.max.z - bounds.min.z))
  const group = new THREE.Group()
  group.name = 'Preview tracks'
  group.position.y = -1
  for (const z of positions) {
    const segment = source.clone(true)
    if (repeatInterval === undefined) segment.position.set(-(bounds.min.x + bounds.max.x) / 2, -bounds.max.y, z - (bounds.min.z + bounds.max.z) / 2)
    else segment.position.z = z // Project rails retain authored origin and layer offsets.
    group.add(segment)
  }
  return group
}

export async function loadPreviewRail(): Promise<THREE.Group> {
  const model = await new OBJLoader().loadAsync('/models/rail.obj')
  try {
    const texture = await new THREE.TextureLoader().loadAsync('/models/rail_base_color.png')
    // Metasequoia stores top-left UVs, matching the normalized GLB pipeline.
    texture.flipY = false
    texture.colorSpace = THREE.SRGBColorSpace
    const material = new THREE.MeshStandardMaterial({ map: texture, roughness: .85, metalness: .1 })
    model.traverse(object => {
      if (!(object instanceof THREE.Mesh)) return
      for (const original of Array.isArray(object.material) ? object.material : [object.material]) original.dispose()
      object.material = material
      object.castShadow = true
      object.receiveShadow = true
    })
    return model
  } catch (error) {
    model.traverse(object => {
      if (!(object instanceof THREE.Mesh)) return
      object.geometry.dispose()
      for (const material of Array.isArray(object.material) ? object.material : [object.material]) material.dispose()
    })
    throw error
  }
}
