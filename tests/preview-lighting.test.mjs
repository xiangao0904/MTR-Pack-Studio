import test from 'node:test'
import assert from 'node:assert/strict'
import * as THREE from 'three'
import { applyPreviewLighting, createPreviewLighting, penumbraRadius } from '../src/lib/preview-lighting.ts'
import { createEnvironmentTexture, previewLightDirection } from '../src/lib/preview-environment-map.ts'
import { defaultViewportSettings, normalizeViewportSettings } from '../src/lib/viewport-settings.ts'

test('installed Three shader receives PCSS and AO only attenuates indirect lighting', () => {
  const material=new THREE.MeshStandardMaterial(),lighting=createPreviewLighting()
  applyPreviewLighting(material,lighting)
  const shader={uniforms:{},fragmentShader:THREE.ShaderLib.standard.fragmentShader}
  material.onBeforeCompile(shader)
  // Three's bundled shader strips comments; patching source-file comments silently fails.
  assert.match(shader.fragmentShader,/float separation =/)
  assert.match(shader.fragmentShader,/blockerDepth \/ blockers/)
  assert.match(shader.fragmentShader,/reflectedLight.indirectDiffuse \*= previewOcclusion/)
  assert.match(shader.fragmentShader,/reflectedLight.indirectSpecular \*= computeSpecularOcclusion/)
  assert.doesNotMatch(shader.fragmentShader,/reflectedLight.direct(?:Diffuse|Specular) \*=/)
  assert.equal(shader.uniforms.previewShadowExtent,lighting.previewShadowExtent)
  assert.equal(shader.uniforms.previewGI,lighting.previewGI)
})

test('transparent and transmissive surfaces do not sample opaque screen-space buffers', () => {
  for (const material of [new THREE.MeshStandardMaterial({transparent:true}),new THREE.MeshPhysicalMaterial({transmission:1})]) {
    applyPreviewLighting(material,createPreviewLighting())
    const shader={uniforms:{},fragmentShader:THREE.ShaderLib.physical.fragmentShader};material.onBeforeCompile(shader)
    assert.doesNotMatch(shader.fragmentShader,/vec2 previewUv/)
    assert.match(shader.fragmentShader,/float separation =/)
  }
})

test('penumbra scales with world-space separation and angular size, not map resolution', () => {
  assert.equal(penumbraRadius(0,12),0)
  assert.equal(penumbraRadius(-2,12),0)
  assert.equal(penumbraRadius(2,12),2*penumbraRadius(1,12))
  assert.ok(penumbraRadius(2,24)>penumbraRadius(2,12))
  const settings=normalizeViewportSettings('studio',{lightSize:Infinity,indirectIntensity:-3})
  assert.equal(settings.lightSize,12);assert.equal(settings.indirectIntensity,0)
  assert.equal(normalizeViewportSettings('minecraft',{lightSize:99,indirectIntensity:99}).lightSize,30)
})

test('procedural HDR environment bright region tracks main light across azimuths', () => {
  for(const lightAzimuth of [0,90,215]) {
    const settings={...defaultViewportSettings('studio'),lightAzimuth,lightElevation:35}
    const texture=createEnvironmentTexture('studio',settings),{data,width,height}=texture.image
    let brightest=0
    for(let i=4;i<data.length;i+=4)if(data[i]>data[brightest])brightest=i
    const pixel=brightest/4,u=(pixel%width+.5)/width,v=(Math.floor(pixel/width)+.5)/height
    const latitude=(v-.5)*Math.PI,longitude=(u-.5)*Math.PI*2
    const direction=new THREE.Vector3(Math.cos(longitude)*Math.cos(latitude),Math.sin(latitude),Math.sin(longitude)*Math.cos(latitude))
    assert.ok(direction.dot(previewLightDirection(settings))>.998)
    assert.equal(texture.colorSpace,THREE.LinearSRGBColorSpace);assert.ok(data[brightest]>1)
    texture.dispose()
  }
})
