import * as THREE from 'three'
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer.js'
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass.js'
import { GTAOPass } from 'three/examples/jsm/postprocessing/GTAOPass.js'
import { SMAAPass } from 'three/examples/jsm/postprocessing/SMAAPass.js'
import { createPreviewLighting } from './preview-lighting.ts'
import { PreviewIndirectPass } from './preview-indirect.ts'
import { OutputPass } from 'three/examples/jsm/postprocessing/OutputPass.js'

/** Normal/depth rendering must retain cutouts and leave glass out of the occluders. */
export function aoMaterial(source: THREE.Material): THREE.MeshNormalMaterial {
  const surface = source as THREE.MeshStandardMaterial
  const material = new THREE.MeshNormalMaterial({ side: source.side, flatShading: surface.flatShading, normalMap: surface.normalMap, normalScale: surface.normalScale })
  material.visible = source.visible && !source.transparent && !surface.wireframe && !(source instanceof THREE.MeshPhysicalMaterial && source.transmission > 0)
  if (surface.map && source.alphaTest > 0) {
    const map = surface.map
    map.updateMatrix()
    material.onBeforeCompile = shader => {
      shader.uniforms.cutoutMap = { value: map }
      shader.uniforms.cutoutTransform = { value: map.matrix }
      shader.uniforms.cutoutThreshold = { value: source.alphaTest }
      shader.uniforms.cutoutOpacity = { value: source.opacity }
      shader.vertexShader = 'varying vec2 vCutoutUv; uniform mat3 cutoutTransform;\n' + shader.vertexShader.replace('#include <uv_vertex>', '#include <uv_vertex>\nvCutoutUv = (cutoutTransform * vec3(uv, 1.0)).xy;')
      shader.fragmentShader = 'varying vec2 vCutoutUv; uniform sampler2D cutoutMap; uniform float cutoutThreshold; uniform float cutoutOpacity;\n' + shader.fragmentShader.replace('#include <clipping_planes_fragment>', '#include <clipping_planes_fragment>\nif (texture2D(cutoutMap, vCutoutUv).a * cutoutOpacity < cutoutThreshold) discard;')
    }
    material.customProgramCacheKey = () => 'ao-cutout-v1'
  }
  return material
}

class PreviewAO extends GTAOPass {
  private normals = new Map<THREE.Material, THREE.MeshNormalMaterial>()

  clearMaterials() { for (const material of this.normals.values()) material.dispose(); this.normals.clear() }

  // GTAOPass's normal override ignores alpha masks; supply per-material normals instead.
  _renderOverride(renderer: THREE.WebGLRenderer, _material: THREE.Material, target: THREE.WebGLRenderTarget) {
    const saved = new Map<THREE.Mesh, THREE.Material | THREE.Material[]>()
    const hidden: THREE.Object3D[] = []
    const background = this.scene.background, override = this.scene.overrideMaterial
    const clear = renderer.getClearColor(new THREE.Color()), alpha = renderer.getClearAlpha()
    const shadowUpdate = renderer.shadowMap.needsUpdate
    const normal = (source: THREE.Material) => {
      if (!this.normals.has(source)) this.normals.set(source, aoMaterial(source))
      return this.normals.get(source)!
    }
    try {
      this.scene.traverse(object => {
        if (object instanceof THREE.Mesh) { saved.set(object, object.material); object.material = Array.isArray(object.material) ? object.material.map(normal) : normal(object.material) }
        else if (object instanceof THREE.Sprite && object.visible) { object.visible = false; hidden.push(object) }
      })
      this.scene.background = null; this.scene.overrideMaterial = null
      renderer.shadowMap.needsUpdate = false
      renderer.setRenderTarget(target); renderer.setClearColor(0x7777ff, 1); renderer.clear(); renderer.render(this.scene, this.camera)
    } finally {
      for (const [mesh, material] of saved) mesh.material = material
      for (const object of hidden) object.visible = true
      this.scene.background = background; this.scene.overrideMaterial = override
      renderer.setClearColor(clear, alpha); renderer.shadowMap.needsUpdate = shadowUpdate
    }
  }

  dispose() { this.clearMaterials(); super.dispose(); this.gtaoMaterial.dispose(); this.blendMaterial.dispose() }
}

export class PreviewRenderer {
  private composer: EffectComposer
  private beauty: RenderPass
  private ao: PreviewAO
  private output = new OutputPass()
  private aa = new SMAAPass()
  private indirect: PreviewIndirectPass
  private base: RenderPass
  readonly lighting = createPreviewLighting()
  private indirectIntensity = 1
  private aoEnabled = true

  constructor(renderer: THREE.WebGLRenderer, scene: THREE.Scene, camera: THREE.Camera) {
    const target = new THREE.WebGLRenderTarget(1, 1, { type: THREE.HalfFloatType, samples: Math.min(4, renderer.capabilities.maxSamples) })
    this.composer = new EffectComposer(renderer, target)
    this.beauty = new RenderPass(scene, camera)
    this.ao = new PreviewAO(scene, camera)
    // Model coordinates are metres: occlusion stays local even for long consists.
    this.ao.updateGtaoMaterial({ radius: .45, thickness: 1, distanceFallOff: 1, samples: 16, screenSpaceRadius: false })
    this.ao.output = GTAOPass.OUTPUT.Off; this.ao.needsSwap = false
    this.lighting.previewAO.value = this.ao.gtaoMap
    this.indirect = new PreviewIndirectPass(camera, this.ao.depthTexture, this.ao.normalTexture)
    this.lighting.previewGI.value = this.indirect.texture
    this.base = new RenderPass(scene, camera)
    const baseRender = this.base.render.bind(this.base), beautyRender = this.beauty.render.bind(this.beauty)
    this.base.render = (...args) => {
      this.lighting.previewGIIntensity.value = 0
      // The radiance source must describe the same opaque surfaces as normal/depth.
      const hidden: THREE.Object3D[] = [], materials = new Set<THREE.Material>()
      scene.traverse(object => {
        if (object instanceof THREE.Mesh) {
          for (const material of Array.isArray(object.material) ? object.material : [object.material]) {
            if (material.visible && (material.transparent || material instanceof THREE.MeshPhysicalMaterial && material.transmission > 0)) { materials.add(material); material.visible = false }
          }
        } else if (object.visible && (object instanceof THREE.Line || object instanceof THREE.Points || object instanceof THREE.Sprite)) { hidden.push(object); object.visible = false }
      })
      try { baseRender(...args) } finally { for (const object of hidden) object.visible = true; for (const material of materials) material.visible = true }
    }
    this.beauty.render = (...args) => { this.lighting.previewGIIntensity.value = this.indirectIntensity; beautyRender(...args) }
    this.composer.addPass(this.ao); this.composer.addPass(this.base); this.composer.addPass(this.indirect)
    this.composer.addPass(this.beauty); this.composer.addPass(this.aa); this.composer.addPass(this.output)
  }

  setCamera(camera: THREE.Camera) {
    this.beauty.camera = camera; this.base.camera = camera; this.indirect.camera = camera; this.ao.camera = camera
    for (const material of [this.ao.gtaoMaterial, this.ao.depthRenderMaterial]) {
      const perspective = camera instanceof THREE.PerspectiveCamera ? 1 : 0
      if (material.defines.PERSPECTIVE_CAMERA !== perspective) { material.defines.PERSPECTIVE_CAMERA = perspective; material.needsUpdate = true }
    }
  }
  setSize(width: number, height: number, ratio: number) { this.lighting.previewResolution.value.set(Math.max(1, width) * ratio, Math.max(1, height) * ratio); this.composer.setPixelRatio(ratio); this.composer.setSize(Math.max(1, width), Math.max(1, height)) }
  setAO(enabled: boolean) { this.aoEnabled = enabled; this.updatePasses() }
  setIndirect(intensity: number) { this.indirectIntensity = intensity; this.updatePasses() }
  private updatePasses() {
    this.ao.enabled = this.aoEnabled || this.indirectIntensity > 0
    this.lighting.previewAOIntensity.value = this.aoEnabled ? .65 : 0
    this.base.enabled = this.indirect.enabled = this.indirectIntensity > 0
  }
  clearMaterials() { this.ao.clearMaterials() }
  render() { this.composer.render() }
  dispose() { this.base.dispose(); this.indirect.dispose(); this.aa.dispose(); this.beauty.dispose(); this.ao.dispose(); this.output.dispose(); this.composer.dispose() }
}

export function filterModelTextures(root: THREE.Object3D, maxAnisotropy: number, pixelated = false) {
  const visited = new Set<THREE.Texture>()
  root.traverse(object => {
    if (!(object instanceof THREE.Mesh)) return
    for (const material of Array.isArray(object.material) ? object.material : [object.material]) {
      for (const texture of Object.values(material)) {
        if (!(texture instanceof THREE.Texture) || visited.has(texture) || texture.isRenderTargetTexture) continue
        visited.add(texture)
        texture.magFilter = pixelated ? THREE.NearestFilter : THREE.LinearFilter
        texture.minFilter = THREE.LinearMipmapLinearFilter
        texture.generateMipmaps = true; texture.anisotropy = Math.min(8, maxAnisotropy); texture.needsUpdate = true
      }
    }
  })
}

/** One embedded image may serve color and data channels; those need distinct texture views. */
export function prepareMaterialTextures(root: THREE.Object3D) {
  const variants = new Map<THREE.Texture, Map<string, THREE.Texture>>()
  root.traverse(object => {
    if (!(object instanceof THREE.Mesh)) return
    for (const material of Array.isArray(object.material) ? object.material : [object.material]) {
      const surface = material as THREE.MeshStandardMaterial
      for (const key of ['map','emissiveMap','normalMap','metalnessMap','roughnessMap','aoMap','alphaMap'] as const) {
        const texture = surface[key]
        if (!texture) continue
        const colorSpace = key === 'map' || key === 'emissiveMap' ? THREE.SRGBColorSpace : THREE.NoColorSpace
        let spaces = variants.get(texture)
        if (!spaces) { spaces = new Map(); variants.set(texture, spaces) }
        if (!spaces.has(colorSpace)) {
          const variant = spaces.size ? texture.clone() : texture
          variant.colorSpace = colorSpace; variant.needsUpdate = true; spaces.set(colorSpace, variant)
        }
        surface[key] = spaces.get(colorSpace)!
      }
    }
  })
}
