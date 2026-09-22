import test from 'node:test'
import assert from 'node:assert/strict'
import * as THREE from 'three'
import { PreviewEnvironment } from '../src/lib/preview-environment.ts'
import { defaultViewportSettings } from '../src/lib/viewport-settings.ts'

function fixture() {
  const scene=new THREE.Scene(),root=new THREE.Group(),map=new THREE.Texture(),normalMap=new THREE.Texture(),environmentMap=new THREE.Texture()
  const source=new THREE.MeshPhysicalMaterial({color:0x4599bb,map,normalMap,transparent:true,opacity:.4,alphaTest:.15,side:THREE.DoubleSide,roughness:.27,metalness:.8,clearcoat:.7,transmission:.25})
  const second=new THREE.MeshStandardMaterial({color:0xff5533})
  const mesh=new THREE.Mesh(new THREE.BoxGeometry(3,4,20),[source,second]);mesh.position.y=2;root.add(mesh);scene.add(root)
  const environment=new PreviewEnvironment(scene,environmentMap)
  return {scene,root,map,normalMap,source,second,mesh,environmentMap,environment,dispose(){environment.dispose();mesh.geometry.dispose();source.dispose();second.dispose();map.dispose();normalMap.dispose();environmentMap.dispose()}}
}

test('three preview modes clone PBR channels and preserve source textures, cutouts and shadow flags',()=>{
  const f=fixture();let sourceDisposed=0;f.map.addEventListener('dispose',()=>sourceDisposed++)
  f.environment.apply(f.root,'studio')
  const gray=f.mesh.material[0]
  assert.notEqual(gray,f.source);assert.equal(gray.color.getHex(),0xb9bec5)
  assert.equal(gray.map,f.map);assert.equal(gray.normalMap,f.normalMap)
  assert.equal(gray.alphaTest,.15);assert.equal(gray.transparent,true);assert.equal(gray.opacity,.4)
  const shader={fragmentShader:'#include <map_fragment>'};gray.onBeforeCompile(shader)
  assert.match(shader.fragmentShader,/diffuseColor.a \*= sampledDiffuseColor.a;/)
  assert.doesNotMatch(shader.fragmentShader,/diffuseColor \*= sampledDiffuseColor;/)
  assert.equal(f.source.color.getHex(),0x4599bb);assert.equal(f.source.clearcoat,.7)
  let grayDisposed=0;gray.addEventListener('dispose',()=>grayDisposed++)
  f.environment.apply(f.root,'material')
  const pbr=f.mesh.material[0]
  assert.equal(grayDisposed,1);assert.ok(pbr instanceof THREE.MeshPhysicalMaterial)
  assert.equal(pbr.map,f.map);assert.equal(pbr.normalMap,f.normalMap)
  assert.equal(pbr.clearcoat,.7);assert.equal(pbr.transmission,.25);assert.equal(pbr.roughness,.27);assert.equal(pbr.metalness,.8)
  assert.equal(f.scene.environment,f.environmentMap);assert.equal(f.mesh.castShadow,true);assert.equal(f.mesh.receiveShadow,true)
  f.environment.apply(f.root,'minecraft');assert.equal(f.mesh.material[0].map,f.map)
  assert.equal(f.map.magFilter,THREE.LinearFilter)
  f.environment.dispose();assert.deepEqual(f.mesh.material,[f.source,f.second]);assert.equal(f.mesh.castShadow,false);assert.equal(f.mesh.receiveShadow,false)
  assert.equal(sourceDisposed,0);assert.equal(f.scene.environment,null);f.dispose()
})

test('Minecraft ground remains one metre per tile and cleans scenery and shadow targets',()=>{
  const f=fixture();f.mesh.scale.z=45;f.mesh.position.z=300
  f.environment.apply(f.root,'minecraft')
  let ground,light
  f.scene.traverse(object=>{if(object instanceof THREE.Mesh&&Array.isArray(object.material)&&object.material.length===6)ground=object;if(object instanceof THREE.DirectionalLight)light=object})
  assert.ok(ground);const {width,height,depth}=ground.geometry.parameters
  assert.ok(width>=1800);assert.equal(width,depth);assert.equal(width/ground.material[2].map.repeat.x,1);assert.equal(depth/ground.material[2].map.repeat.y,1)
  assert.ok(Math.abs(ground.position.y+height/2+.02)<1e-12);assert.equal(ground.position.z,300);assert.equal(ground.receiveShadow,true)
  assert.ok(ground.material[2] instanceof THREE.MeshStandardMaterial)
  let geometryDisposed=0,textureDisposed=0,shadowDisposed=0
  ground.geometry.addEventListener('dispose',()=>geometryDisposed++);ground.material[2].map.addEventListener('dispose',()=>textureDisposed++)
  light.shadow.map=new THREE.WebGLRenderTarget(8,8);light.shadow.map.addEventListener('dispose',()=>shadowDisposed++)
  f.environment.dispose();assert.equal(geometryDisposed,1);assert.equal(textureDisposed,1);assert.equal(shadowDisposed,1)
  assert.equal(f.scene.fog,null);assert.deepEqual(f.scene.children,[f.root]);f.dispose()
})

test('sun and light agree and shadow frustum includes model plus low-angle ground projection',()=>{
  const f=fixture(),settings={...defaultViewportSettings('minecraft'),lightAzimuth:215,lightElevation:10,shadowQuality:'high',ground:false}
  f.environment.apply(f.root,'minecraft',settings)
  let light,sun;f.scene.traverse(object=>{if(object instanceof THREE.DirectionalLight)light=object;if(object instanceof THREE.Sprite)sun=object})
  assert.equal(light.shadow.mapSize.x,4096);assert.equal(light.castShadow,true)
  const direction=light.position.clone().sub(light.target.position).normalize()
  assert.ok(direction.distanceTo(sun.position.clone().sub(light.target.position).normalize())<1e-10)
  const camera=light.shadow.camera;camera.updateMatrixWorld(true)
  const bounds=new THREE.Box3().setFromObject(f.root)
  const points=[bounds.min.clone(),bounds.max.clone(),new THREE.Vector3(bounds.min.x,bounds.max.y,bounds.max.z),new THREE.Vector3(bounds.max.x,bounds.max.y,bounds.min.z)]
  for(const point of [...points])points.push(point.clone().addScaledVector(direction,-(point.y+.02)/direction.y))
  for(const point of points){const projected=point.clone().project(camera);assert.ok(Math.abs(projected.x)<=1.001&&Math.abs(projected.y)<=1.001&&Math.abs(projected.z)<=1.001)}
  assert.equal(f.scene.children.flatMap(group=>group.children).filter(object=>object instanceof THREE.Mesh&&object.receiveShadow).length,2)
  f.dispose()
})

test('studio lights follow camera without changing source models',()=>{
  const f=fixture();f.environment.apply(f.root,'studio')
  let light;f.scene.traverse(object=>{if(object instanceof THREE.DirectionalLight&&!light)light=object})
  const before=light.position.clone(),camera=new THREE.PerspectiveCamera();camera.rotation.y=Math.PI/2
  f.environment.updateCamera(camera);assert.ok(light.position.distanceTo(before)>1)
  assert.equal(f.mesh.position.y,2);assert.equal(f.mesh.scale.x,1);f.dispose()
})

test('only material mode has an optional floor; studio stays empty and Minecraft keeps its blocks',()=>{
  const f=fixture()
  const groundCount=()=>{let count=0;f.scene.traverse(object=>{if(object instanceof THREE.Mesh&&object!==f.mesh&&object.receiveShadow)count++});return count}
  f.environment.apply(f.root,'studio',{...defaultViewportSettings('studio'),ground:true});assert.equal(groundCount(),0)
  f.environment.apply(f.root,'material',{...defaultViewportSettings('material'),ground:false});assert.equal(groundCount(),0)
  f.environment.apply(f.root,'material',{...defaultViewportSettings('material'),ground:true});assert.equal(groundCount(),1)
  f.environment.apply(f.root,'minecraft',{...defaultViewportSettings('minecraft'),ground:false});assert.equal(groundCount(),1)
  f.dispose()
})
