import * as THREE from 'three'
import { normalizeViewportSettings, type PreviewRenderMode, type ViewportSettings } from './viewport-settings.ts'
export type { PreviewRenderMode } from './viewport-settings.ts'

type MeshState = {material: THREE.Material | THREE.Material[]; castShadow: boolean; receiveShadow: boolean}
const GROUND_Y = -.02
const corners = (box: THREE.Box3) => [0,1,2,3,4,5,6,7].map(index => new THREE.Vector3(index&1 ? box.max.x : box.min.x,index&2 ? box.max.y : box.min.y,index&4 ? box.max.z : box.min.z))
function visible(object: THREE.Object3D): boolean { return object.visible && (!object.parent || visible(object.parent)) }

/** Owns preview-only clones and scenery. Source materials, textures and geometry stay unchanged. */
export class PreviewEnvironment {
  private scene: THREE.Scene
  private environmentMap?: THREE.Texture
  private scenery = new THREE.Group()
  private originals = new Map<THREE.Mesh, MeshState>()
  private materials = new Set<THREE.Material>()
  private textures = new Set<THREE.Texture>()
  private geometry = new Set<THREE.BufferGeometry>()
  private lights: THREE.DirectionalLight[] = []
  private bounds = new THREE.Box3()
  private mode: PreviewRenderMode = 'studio'
  private settings = normalizeViewportSettings('studio',undefined)
  constructor(scene: THREE.Scene, environmentMap?: THREE.Texture) { this.scene=scene;this.environmentMap=environmentMap;scene.add(this.scenery) }

  restoreMaterials() {
    for (const [mesh,state] of this.originals) {mesh.material=state.material;mesh.castShadow=state.castShadow;mesh.receiveShadow=state.receiveShadow}
    this.originals.clear()
    for (const material of this.materials) material.dispose()
    for (const texture of this.textures) texture.dispose()
    this.materials.clear();this.textures.clear()
  }
  private clearScenery() {
    for (const light of this.lights) light.shadow.dispose()
    this.lights=[]
    for (const geometry of this.geometry) geometry.dispose()
    this.geometry.clear();this.scenery.clear()
  }
  apply(root: THREE.Object3D | undefined, mode: PreviewRenderMode, settings?: ViewportSettings) {
    this.restoreMaterials();this.clearScenery();this.mode=mode;this.settings=normalizeViewportSettings(mode,settings)
    this.bounds.makeEmpty();root?.updateWorldMatrix(true,true)
    root?.traverse(object=>{if(object instanceof THREE.Mesh && visible(object)) {if(!object.geometry.boundingBox)object.geometry.computeBoundingBox();if(object.geometry.boundingBox)this.bounds.union(object.geometry.boundingBox.clone().applyMatrix4(object.matrixWorld))}})
    if(this.bounds.isEmpty())this.bounds.set(new THREE.Vector3(-1,0,-1),new THREE.Vector3(1,2,1))
    this.scene.background=new THREE.Color(mode==='minecraft'?0x83b6f4:mode==='material'?0x252b32:0x20252a)
    this.scene.fog=null;this.scene.environment=mode==='studio'?null:this.environmentMap??null
    this.scene.environmentIntensity=this.settings.environmentIntensity
    const center=this.bounds.getCenter(new THREE.Vector3()),extent=this.bounds.getSize(new THREE.Vector3())
    const size=Math.max(512,Math.ceil(Math.max(extent.x,extent.z))*2)
    if(mode==='minecraft')this.createMinecraftScenery(center,size)
    else if(mode==='material'&&this.settings.ground)this.createNeutralGround(center,size)
    if(mode==='studio') {
      this.scenery.add(new THREE.HemisphereLight(0xf0f5ff,0x65717c,this.settings.environmentIntensity*.8))
      for(const intensity of [this.settings.lightIntensity,1.1,1.5]) {const light=new THREE.DirectionalLight(0xffffff,intensity);this.lights.push(light);this.scenery.add(light,light.target)}
      this.updateStudioLights(new THREE.Quaternion())
    } else {
      this.scenery.add(new THREE.HemisphereLight(mode==='minecraft'?0xcfe5ff:0xe1e8f2,0x8b846e,this.settings.environmentIntensity*.3))
      const light=new THREE.DirectionalLight(mode==='minecraft'?0xfff5dd:0xffffff,this.settings.lightIntensity)
      const direction=this.lightDirection();light.position.copy(center).addScaledVector(direction,Math.max(30,extent.length()*2));light.target.position.copy(center)
      light.castShadow=true;light.shadow.mapSize.setScalar(this.settings.shadowQuality==='high'?4096:2048)
      light.shadow.bias=-.00005;light.shadow.normalBias=.025;light.shadow.radius=2
      this.lights.push(light);this.scenery.add(light,light.target);this.fitShadow(light,direction)
    }
    root?.traverse(object=>{
      if(!(object instanceof THREE.Mesh))return
      this.originals.set(object,{material:object.material,castShadow:object.castShadow,receiveShadow:object.receiveShadow})
      object.material=Array.isArray(object.material)?object.material.map(source=>this.convertMaterial(source,mode)):this.convertMaterial(object.material,mode)
      object.castShadow=mode!=='studio';object.receiveShadow=mode!=='studio'
    })
  }
  private lightDirection() {
    const azimuth=THREE.MathUtils.degToRad(this.settings.lightAzimuth),elevation=THREE.MathUtils.degToRad(this.settings.lightElevation)
    return new THREE.Vector3(Math.sin(azimuth)*Math.cos(elevation),Math.sin(elevation),Math.cos(azimuth)*Math.cos(elevation))
  }
  private fitShadow(light: THREE.DirectionalLight,direction: THREE.Vector3) {
    const points=corners(this.bounds)
    for(const point of [...points]) {const distance=Math.max(0,(point.y-GROUND_Y)/direction.y);points.push(point.clone().addScaledVector(direction,-distance))}
    this.scenery.updateWorldMatrix(true,true);light.shadow.updateMatrices(light)
    const local=new THREE.Box3().setFromPoints(points.map(point=>point.applyMatrix4(light.shadow.camera.matrixWorldInverse)))
    const margin=Math.max(.5,this.bounds.getSize(new THREE.Vector3()).length()*.02),camera=light.shadow.camera
    camera.left=local.min.x-margin;camera.right=local.max.x+margin;camera.bottom=local.min.y-margin;camera.top=local.max.y+margin
    camera.near=Math.max(.1,-local.max.z-margin);camera.far=Math.max(camera.near+1,-local.min.z+margin);camera.updateProjectionMatrix()
  }
  updateCamera(camera: THREE.Camera) { if(this.mode==='studio')this.updateStudioLights(camera.getWorldQuaternion(new THREE.Quaternion())) }
  private updateStudioLights(orientation: THREE.Quaternion) {
    const center=this.bounds.getCenter(new THREE.Vector3()),distance=Math.max(8,this.bounds.getSize(new THREE.Vector3()).length())
    const offsets=[new THREE.Vector3(-.7,1,1),new THREE.Vector3(1,.25,.4),new THREE.Vector3(.3,.7,-1)]
    this.lights.forEach((light,index)=>{light.position.copy(center).add(offsets[index]!.clone().applyQuaternion(orientation).multiplyScalar(distance));light.target.position.copy(center)})
  }
  private convertMaterial(source: THREE.Material,mode: PreviewRenderMode) {
    const original=source as THREE.MeshStandardMaterial
    const material=original.isMeshStandardMaterial ? original.clone() : new THREE.MeshStandardMaterial({color:original.color??0xffffff,map:original.map??null,alphaMap:original.alphaMap??null,opacity:source.opacity,transparent:source.transparent,alphaTest:source.alphaTest,side:source.side,depthWrite:source.depthWrite,depthTest:source.depthTest,vertexColors:source.vertexColors,wireframe:original.wireframe??false})
    material.name=source.name;material.userData={...source.userData};material.fog=mode==='minecraft'
    if(mode==='studio') {
      material.color.setHex(0xb9bec5);material.metalness=0;material.roughness=.85;material.metalnessMap=null;material.roughnessMap=null
      material.emissive.setHex(0x000000);material.emissiveMap=null;material.lightMap=null;material.vertexColors=false;material.envMap=null
      if(material instanceof THREE.MeshPhysicalMaterial) {material.clearcoat=0;material.sheen=0;material.iridescence=0;material.transmission=0;material.specularColor.setHex(0xffffff);material.specularColorMap=null;material.attenuationColor.setHex(0xffffff)}
      // Keep map alpha and UV transforms for cutouts, while suppressing its RGB tint.
      material.onBeforeCompile=shader=>{shader.fragmentShader=shader.fragmentShader.replace('#include <map_fragment>',THREE.ShaderChunk.map_fragment.replace('diffuseColor *= sampledDiffuseColor;','diffuseColor.a *= sampledDiffuseColor.a;'))}
      material.customProgramCacheKey=()=> 'studio-gray-alpha-v2'
    }
    this.materials.add(material);return material
  }
  private registerMaterial<T extends THREE.Material>(material:T) {this.materials.add(material);return material}
  private createNeutralGround(center:THREE.Vector3,size:number) {
    const geometry=new THREE.PlaneGeometry(size,size);this.geometry.add(geometry)
    const ground=new THREE.Mesh(geometry,this.registerMaterial(new THREE.MeshStandardMaterial({color:0x777e86,roughness:.94,metalness:0})))
    ground.rotation.x=-Math.PI/2;ground.position.set(Math.round(center.x),GROUND_Y,Math.round(center.z));ground.receiveShadow=true;this.scenery.add(ground)
  }
  private pixelTexture(kind:'grass'|'side'|'dirt') {
    const data=new Uint8Array(16*16*4)
    for(let y=0;y<16;y++)for(let x=0;x<16;x++) {const n=((x*37+y*71+x*y*13)%29)-14,grassy=kind==='grass'||(kind==='side'&&y>=13-((x*7)%3)),base=grassy?[102,151,64]:[121,87,58],at=(y*16+x)*4;data[at]=base[0]!+n;data[at+1]=base[1]!+n;data[at+2]=base[2]!+n;data[at+3]=255}
    const texture=new THREE.DataTexture(data,16,16,THREE.RGBAFormat);texture.colorSpace=THREE.SRGBColorSpace;texture.magFilter=THREE.NearestFilter;texture.minFilter=THREE.NearestMipmapLinearFilter;texture.generateMipmaps=true;texture.wrapS=texture.wrapT=THREE.RepeatWrapping;texture.needsUpdate=true;this.textures.add(texture);return texture
  }
  private createMinecraftScenery(center:THREE.Vector3,size:number) {
    this.scene.fog=new THREE.Fog(0xbed9f1,size*.35,size*.85)
    {
      const top=this.pixelTexture('grass');top.repeat.set(size,size);const side=this.pixelTexture('side');side.repeat.set(size,1);const bottom=this.pixelTexture('dirt');bottom.repeat.set(size,size)
      const make=(map:THREE.Texture)=>this.registerMaterial(new THREE.MeshStandardMaterial({map,roughness:1,metalness:0}))
      const geometry=new THREE.BoxGeometry(size,1,size);this.geometry.add(geometry)
      const ground=new THREE.Mesh(geometry,[make(side),make(side),make(top),make(bottom),make(side),make(side)])
      ground.position.set(Math.round(center.x),GROUND_Y-.5,Math.round(center.z));ground.receiveShadow=true;this.scenery.add(ground)
    }
    const geometry=new THREE.BoxGeometry(1,1,1);this.geometry.add(geometry)
    const material=this.registerMaterial(new THREE.MeshStandardMaterial({color:0xf7fbff,roughness:1}))
    for(let index=0;index<24;index++) {const cloud=new THREE.Mesh(geometry,material);cloud.position.set(center.x+((index*83)%380)-190,44+(index%3)*2,center.z+((index*137)%380)-190);cloud.scale.set(15+(index%4)*9,3,9+(index%5)*6);this.scenery.add(cloud)}
    const sun=new THREE.Sprite(this.registerMaterial(new THREE.SpriteMaterial({color:0xfff8d5,fog:false,toneMapped:false})))
    sun.position.copy(center).addScaledVector(this.lightDirection(),Math.max(220,size*.43));sun.scale.set(22,22,1);this.scenery.add(sun)
  }
  dispose() {this.restoreMaterials();this.clearScenery();this.scene.remove(this.scenery);this.scene.fog=null;if(this.scene.environment===this.environmentMap)this.scene.environment=null}
}
