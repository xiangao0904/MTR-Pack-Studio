import test from 'node:test'
import assert from 'node:assert/strict'
import * as THREE from 'three'
import { PreviewEnvironment } from '../src/lib/preview-environment.ts'

test('preview modes preserve original multi-material texture and transparency and release replacements', () => {
  const scene=new THREE.Scene(), root=new THREE.Group()
  const texture=new THREE.Texture()
  const original=new THREE.MeshStandardMaterial({color:0x4599bb,map:texture,transparent:true,opacity:.4,alphaTest:.1,side:THREE.DoubleSide})
  const solid=new THREE.MeshStandardMaterial({color:0xff5533})
  const mesh=new THREE.Mesh(new THREE.BoxGeometry(),[original,solid]);root.add(mesh);scene.add(root)
  const environment=new PreviewEnvironment(scene)
  let sourceDisposed=0;texture.addEventListener('dispose',()=>sourceDisposed++)
  environment.apply(root,'unlit')
  assert.equal(mesh.material[0].type,'MeshBasicMaterial')
  assert.equal(mesh.material[0].map,texture)
  assert.equal(mesh.material[0].opacity,.4)
  assert.equal(mesh.material[0].transparent,true)
  assert.equal(mesh.material[0].alphaTest,.1)
  assert.equal(mesh.material[0].side,THREE.DoubleSide)
  assert.equal(mesh.material[0].color.getHex(),original.color.getHex())
  let basicDisposed=0;mesh.material[0].addEventListener('dispose',()=>basicDisposed++)
  environment.apply(root,'minecraft')
  assert.equal(basicDisposed,1)
  assert.notEqual(mesh.material[0].map,texture)
  assert.equal(mesh.material[0].map.magFilter,THREE.NearestFilter)
  assert.equal(texture.magFilter,THREE.LinearFilter)
  let cloneDisposed=0;mesh.material[0].map.addEventListener('dispose',()=>cloneDisposed++)
  environment.apply(root,'studio')
  assert.deepEqual(mesh.material,[original,solid])
  assert.equal(cloneDisposed,1);assert.equal(sourceDisposed,0)
  environment.dispose();assert.equal(sourceDisposed,0)
  mesh.geometry.dispose();original.dispose();solid.dispose();texture.dispose()
})

test('Minecraft ground uses one metre tiles, stays below origin, covers long consists and cleans scene resources', () => {
  const scene=new THREE.Scene(),root=new THREE.Group()
  const model=new THREE.Mesh(new THREE.BoxGeometry(3,4,900),new THREE.MeshStandardMaterial());model.position.z=300;root.add(model);scene.add(root)
  const environment=new PreviewEnvironment(scene);environment.apply(root,'minecraft')
  let ground
  scene.traverse(object=>{if(object instanceof THREE.Mesh && Array.isArray(object.material) && object.material.length===6)ground=object})
  assert.ok(ground)
  const {width,height,depth}=ground.geometry.parameters
  assert.ok(width>=1800);assert.equal(width,depth)
  assert.equal(width/ground.material[2].map.repeat.x,1)
  assert.equal(depth/ground.material[2].map.repeat.y,1)
  assert.ok(Math.abs(ground.position.y+height/2+.02)<1e-12)
  assert.equal(ground.parent.position.z,300)
  let geometryDisposed=0,textureDisposed=0
  ground.geometry.addEventListener('dispose',()=>geometryDisposed++)
  ground.material[2].map.addEventListener('dispose',()=>textureDisposed++)
  environment.dispose()
  assert.equal(geometryDisposed,1);assert.equal(textureDisposed,1)
  assert.equal(scene.fog,null);assert.deepEqual(scene.children,[root])
  model.geometry.dispose();model.material.dispose()
})
