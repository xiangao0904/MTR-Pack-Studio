import * as THREE from 'three'
import type { PreviewRenderMode, ViewportSettings } from './viewport-settings.ts'

export function previewLightDirection(settings: Pick<ViewportSettings, 'lightAzimuth' | 'lightElevation'>) {
  const a = THREE.MathUtils.degToRad(settings.lightAzimuth), e = THREE.MathUtils.degToRad(settings.lightElevation)
  return new THREE.Vector3(Math.sin(a) * Math.cos(e), Math.sin(e), Math.cos(a) * Math.cos(e))
}

/** Procedural linear HDR lighting: sky outdoors, broad neutral studio reflectors indoors. */
export function createEnvironmentTexture(mode: PreviewRenderMode, settings: ViewportSettings) {
  const width = 512, height = 256, data = new Float32Array(width * height * 4)
  const key = previewLightDirection(settings), direction = new THREE.Vector3()
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    const phi = ((x + .5) / width - .5) * Math.PI * 2, theta = (y + .5) / height * Math.PI
    direction.set(Math.cos(phi) * Math.sin(theta), -Math.cos(theta), Math.sin(phi) * Math.sin(theta))
    const up = Math.max(0, direction.y), down = Math.max(0, -direction.y)
    const glow = Math.pow(Math.max(0, direction.dot(key)), mode === 'minecraft' ? 16 : 24)
    const at = (y * width + x) * 4
    if (mode === 'minecraft') {
      data[at] = .25 + .15 * up - .15 * down + .35 * glow
      data[at + 1] = .32 + .23 * up - .20 * down + .30 * glow
      data[at + 2] = .40 + .38 * up - .32 * down + .22 * glow
    } else {
      const value = .13 + .24 * up - .08 * down + 2.5 * glow
      data[at] = value; data[at + 1] = value; data[at + 2] = value
    }
    data[at + 3] = 1
  }
  const texture = new THREE.DataTexture(data, width, height, THREE.RGBAFormat, THREE.FloatType)
  texture.mapping = THREE.EquirectangularReflectionMapping
  texture.colorSpace = THREE.LinearSRGBColorSpace; texture.needsUpdate = true
  return texture
}

export class PreviewEnvironmentMap {
  private generator: THREE.PMREMGenerator
  private target?: THREE.WebGLRenderTarget
  private signature = ''
  constructor(renderer: THREE.WebGLRenderer) { this.generator = new THREE.PMREMGenerator(renderer) }
  update(mode: PreviewRenderMode, settings: ViewportSettings) {
    const signature = `${mode}-${settings.lightAzimuth}-${settings.lightElevation}`
    if (signature !== this.signature) {
      const source = createEnvironmentTexture(mode, settings)
      const next = this.generator.fromEquirectangular(source)
      source.dispose(); this.target?.dispose(); this.target = next; this.signature = signature
    }
    return this.target!.texture
  }
  dispose() { this.target?.dispose(); this.generator.dispose() }
}
