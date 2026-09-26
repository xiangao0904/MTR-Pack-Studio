import test from 'node:test'
import assert from 'node:assert/strict'
import * as THREE from 'three'
import { aoMaterial, filterModelTextures, prepareMaterialTextures } from '../src/lib/preview-renderer.ts'

test('model filtering smooths minification in both texture modes and respects hardware limits', () => {
  const map = new THREE.Texture(), normalMap = new THREE.Texture()
  const material = new THREE.MeshStandardMaterial({map, normalMap})
  const mesh = new THREE.Mesh(new THREE.BoxGeometry(), material)
  filterModelTextures(mesh, 4)
  for (const texture of [map, normalMap]) {
    assert.equal(texture.magFilter, THREE.LinearFilter)
    assert.equal(texture.minFilter, THREE.LinearMipmapLinearFilter)
    assert.equal(texture.generateMipmaps, true)
    assert.equal(texture.anisotropy, 4)
  }
  const version=map.version
  filterModelTextures(mesh, 4)
  assert.equal(map.version,version)
  filterModelTextures(mesh, 16, true)
  assert.equal(map.magFilter, THREE.NearestFilter)
  assert.equal(map.minFilter, THREE.LinearMipmapLinearFilter)
  assert.equal(map.anisotropy, 8)
  mesh.geometry.dispose(); material.dispose(); map.dispose(); normalMap.dispose()
})

test('AO retains cutout UVs and excludes transparent glass and wireframes', () => {
  const map = new THREE.Texture(); map.offset.set(.2,.3)
  const source = new THREE.MeshStandardMaterial({map, alphaTest:.2, side:THREE.DoubleSide})
  const normal = aoMaterial(source)
  const shader = {uniforms:{},vertexShader:'#include <uv_vertex>',fragmentShader:'#include <clipping_planes_fragment>'}
  normal.onBeforeCompile(shader)
  assert.equal(normal.side,THREE.DoubleSide)
  assert.equal(shader.uniforms.cutoutMap.value,map)
  assert.equal(shader.uniforms.cutoutThreshold.value,.2)
  assert.match(shader.fragmentShader,/discard/)
  source.transparent=true
  const glass=aoMaterial(source);assert.equal(glass.visible,false)
  source.transparent=false;source.wireframe=true
  const wire=aoMaterial(source);assert.equal(wire.visible,false)
  normal.dispose();glass.dispose();wire.dispose();source.dispose();map.dispose()
})

test('shared embedded images use independent color spaces for PBR color and data channels', () => {
  const texture=new THREE.Texture(),material=new THREE.MeshStandardMaterial({map:texture,normalMap:texture,roughnessMap:texture,emissiveMap:texture})
  const mesh=new THREE.Mesh(new THREE.BoxGeometry(),material)
  prepareMaterialTextures(mesh)
  assert.equal(material.map.colorSpace,THREE.SRGBColorSpace)
  assert.equal(material.normalMap.colorSpace,THREE.NoColorSpace)
  assert.notEqual(material.map,material.normalMap)
  assert.equal(material.normalMap,material.roughnessMap)
  assert.equal(material.map,material.emissiveMap)
  material.normalMap.dispose();texture.dispose();material.dispose();mesh.geometry.dispose()
})
