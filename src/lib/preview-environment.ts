import * as THREE from 'three'
import { minecraftAtmosphere } from './minecraft-sky.ts'
import { applyPreviewLighting, createPreviewLighting, penumbraRadius, type PreviewLighting } from './preview-lighting.ts'
import { previewLightDirection } from './preview-environment-map.ts'
import { normalizeViewportSettings, type PreviewRenderMode, type ViewportSettings } from './viewport-settings.ts'
export type { PreviewRenderMode } from './viewport-settings.ts'

type MeshState = {material: THREE.Material | THREE.Material[]; castShadow: boolean; receiveShadow: boolean}
export type PreviewLightingMood = { environment: number; direct: number; color: THREE.Color }
const GROUND_Y = -.02
const PREVIEW_GROUND_Y = GROUND_Y - 1
const corners = (box: THREE.Box3) => [0,1,2,3,4,5,6,7].map(index => new THREE.Vector3(index&1 ? box.max.x : box.min.x,index&2 ? box.max.y : box.min.y,index&4 ? box.max.z : box.min.z))
function visible(object: THREE.Object3D): boolean { return object.visible && (!object.parent || visible(object.parent)) }

/** Owns preview-only clones and scenery. Source materials, textures and geometry stay unchanged. */
export class PreviewEnvironment {
  private scene: THREE.Scene
  private environmentMap?: THREE.Texture
  private skyBackground?: THREE.Texture
  private scenery = new THREE.Group()
  private originals = new Map<THREE.Mesh, MeshState>()
  private materials = new Set<THREE.Material>()
  private textures = new Set<THREE.Texture>()
  private geometry = new Set<THREE.BufferGeometry>()
  private lights: THREE.DirectionalLight[] = []
  private bounds = new THREE.Box3()
  private groundY = PREVIEW_GROUND_Y
  private mode: PreviewRenderMode = 'studio'
  private clouds?: THREE.InstancedMesh
  private settings = normalizeViewportSettings('studio',undefined)
  private lighting: PreviewLighting
  constructor(scene: THREE.Scene, environmentMap?: THREE.Texture, lighting = createPreviewLighting()) { this.scene=scene;this.environmentMap=environmentMap;this.lighting=lighting;scene.add(this.scenery) }
  setEnvironmentMap(texture: THREE.Texture, background?: THREE.Texture) { this.environmentMap=texture;this.skyBackground=background }
  lightingMood(): PreviewLightingMood { return {environment:this.scene.environmentIntensity,direct:this.lights[0]?.intensity??0,color:this.lights[0]?.color.clone()??new THREE.Color(0xffffff)} }
  blendLighting(from: PreviewLightingMood, to: PreviewLightingMood, amount: number) {
    this.scene.environmentIntensity=THREE.MathUtils.lerp(from.environment,to.environment,amount)
    if(this.lights[0]){this.lights[0].intensity=THREE.MathUtils.lerp(from.direct,to.direct,amount);this.lights[0].color.copy(from.color).lerp(to.color,amount)}
  }

  restoreMaterials() {
    for (const [mesh,state] of this.originals) {mesh.material=state.material;mesh.castShadow=state.castShadow;mesh.receiveShadow=state.receiveShadow}
    this.originals.clear()
    for (const material of this.materials) material.dispose()
    for (const texture of this.textures) texture.dispose()
    this.materials.clear();this.textures.clear()
  }
  private detachMaterials(root?: THREE.Object3D) {
    root?.traverse(object => {
      if (!(object instanceof THREE.Mesh)) return
      const state = this.originals.get(object)
      if (!state) return
      for (const material of Array.isArray(object.material) ? object.material : [object.material]) {
        this.materials.delete(material)
        material.dispose()
      }
      object.material = state.material
      object.castShadow = state.castShadow
      object.receiveShadow = state.receiveShadow
      this.originals.delete(object)
    })
  }
  private attachMaterials(root?: THREE.Object3D) {
    root?.traverse(object => {
      if (!(object instanceof THREE.Mesh) || this.originals.has(object)) return
      this.originals.set(object,{material:object.material,castShadow:object.castShadow,receiveShadow:object.receiveShadow})
      object.material=Array.isArray(object.material)?object.material.map(source=>this.convertMaterial(source,this.mode)):this.convertMaterial(object.material,this.mode)
      object.castShadow=true;object.receiveShadow=true
    })
  }
  /** Swap preview geometry while retaining the current sky, ground, lights and environment map. */
  replaceObjects(previous: THREE.Object3D | undefined, next: THREE.Object3D | undefined, root: THREE.Object3D, retainPrevious = false) {
    if (previous !== next && !retainPrevious) this.detachMaterials(previous)
    this.attachMaterials(next)
    this.bounds.makeEmpty()
    root.updateWorldMatrix(true,true)
    root.traverse(object=>{if(object instanceof THREE.Mesh && visible(object)) {if(!object.geometry.boundingBox)object.geometry.computeBoundingBox();if(object.geometry.boundingBox)this.bounds.union(object.geometry.boundingBox.clone().applyMatrix4(object.matrixWorld))}})
    if(this.bounds.isEmpty())this.bounds.set(new THREE.Vector3(-1,0,-1),new THREE.Vector3(1,2,1))
    const light=this.lights[0]
    if(light) {
      const center=this.bounds.getCenter(new THREE.Vector3()),extent=this.bounds.getSize(new THREE.Vector3()),direction=this.lightDirection()
      light.position.copy(center).addScaledVector(direction,Math.max(30,extent.length()*2))
      light.target.position.copy(center)
      this.fitShadow(light,direction)
      const camera=light.shadow.camera
      this.lighting.previewShadowExtent.value.set(camera.right-camera.left,camera.top-camera.bottom,camera.far-camera.near)
      this.lighting.previewLightRadius.value=penumbraRadius(1,this.settings.lightSize)
      light.shadow.needsUpdate=true
    }
  }
  releaseObjects(root: THREE.Object3D) { this.detachMaterials(root) }
  private clearScenery() {
    if(this.clouds && !this.clouds.parent)this.clouds.dispose()
    for (const light of this.lights) light.shadow.dispose()
    this.lights=[]
    for (const geometry of this.geometry) geometry.dispose()
    this.geometry.clear();this.scenery.traverse(object=>{if(object instanceof THREE.InstancedMesh)object.dispose()});this.scenery.clear();this.clouds=undefined
  }
  apply(root: THREE.Object3D | undefined, mode: PreviewRenderMode, settings?: ViewportSettings, groundY = PREVIEW_GROUND_Y) {
    this.groundY = groundY
    this.restoreMaterials();this.clearScenery();this.mode=mode;this.settings=normalizeViewportSettings(mode,settings)
    this.bounds.makeEmpty();root?.updateWorldMatrix(true,true)
    root?.traverse(object=>{if(object instanceof THREE.Mesh && visible(object)) {if(!object.geometry.boundingBox)object.geometry.computeBoundingBox();if(object.geometry.boundingBox)this.bounds.union(object.geometry.boundingBox.clone().applyMatrix4(object.matrixWorld))}})
    if(this.bounds.isEmpty())this.bounds.set(new THREE.Vector3(-1,0,-1),new THREE.Vector3(1,2,1))
    this.scene.background=mode==='minecraft' && this.skyBackground ? this.skyBackground : new THREE.Color(mode==='minecraft'?0x83b6f4:mode==='material'?0x202226:0x191b1f)
    this.scene.fog=null;this.scene.environment=this.environmentMap??null
    this.scene.environmentIntensity=this.settings.environmentIntensity
    const center=this.bounds.getCenter(new THREE.Vector3()),extent=this.bounds.getSize(new THREE.Vector3())
    const size=Math.max(512,Math.ceil(Math.max(extent.x,extent.z))*2)
    if(mode==='minecraft')this.createMinecraftScenery(center,size)
    else if(mode==='material'&&this.settings.ground)this.createNeutralGround(center,size)
    const light=new THREE.DirectionalLight(mode==='minecraft'?minecraftAtmosphere(this.settings).sun:0xffffff,this.settings.lightIntensity)
    const direction=this.lightDirection();light.position.copy(center).addScaledVector(direction,Math.max(30,extent.length()*2));light.target.position.copy(center)
    light.castShadow=true;light.shadow.mapSize.setScalar(this.settings.shadowQuality==='high'?4096:2048)
    light.shadow.bias=-.00005;light.shadow.normalBias=.008
    this.lights.push(light);this.scenery.add(light,light.target);this.fitShadow(light,direction)
    const camera=light.shadow.camera
    this.lighting.previewShadowExtent.value.set(camera.right-camera.left,camera.top-camera.bottom,camera.far-camera.near)
    this.lighting.previewLightRadius.value=penumbraRadius(1,this.settings.lightSize)
    this.attachMaterials(root)
  }
  /** Update uniforms, lights and existing scenery without touching source model materials. */
  updateSettings(mode: PreviewRenderMode, settings: ViewportSettings) {
    if (mode !== this.mode || (mode === 'material' && settings.ground !== this.settings.ground) || !this.lights.length) return false
    const previous=this.settings
    this.settings=normalizeViewportSettings(mode,settings)
    this.scene.environmentIntensity=this.settings.environmentIntensity
    const light=this.lights[0]!
    light.intensity=this.settings.lightIntensity
    light.color.copy(mode==='minecraft'?minecraftAtmosphere(this.settings).sun:new THREE.Color(0xffffff))
    if (previous.shadowQuality !== this.settings.shadowQuality) light.shadow.mapSize.setScalar(this.settings.shadowQuality==='high'?4096:2048)
    if (previous.lightAzimuth !== this.settings.lightAzimuth || previous.lightElevation !== this.settings.lightElevation || previous.lightSize !== this.settings.lightSize || previous.shadowQuality !== this.settings.shadowQuality) {
      const center=this.bounds.getCenter(new THREE.Vector3()),extent=this.bounds.getSize(new THREE.Vector3()),direction=this.lightDirection()
      light.position.copy(center).addScaledVector(direction,Math.max(30,extent.length()*2))
      light.target.position.copy(center)
      this.fitShadow(light,direction)
      const camera=light.shadow.camera
      this.lighting.previewShadowExtent.value.set(camera.right-camera.left,camera.top-camera.bottom,camera.far-camera.near)
      this.lighting.previewLightRadius.value=penumbraRadius(1,this.settings.lightSize)
      light.shadow.needsUpdate=true
    }
    if (mode==='minecraft') {
      const extent=this.bounds.getSize(new THREE.Vector3())
      const size=Math.max(512,Math.ceil(Math.max(extent.x,extent.z))*2)
      if(this.scene.fog instanceof THREE.Fog) {
        this.scene.fog.color.copy(minecraftAtmosphere(this.settings).horizon)
        this.scene.fog.near=size*(.25-this.settings.skyHaze*.13)
        this.scene.fog.far=size*(.85-this.settings.skyHaze*.25)
      }
      if(previous.cloudCover!==this.settings.cloudCover)this.updateClouds(this.bounds.getCenter(new THREE.Vector3()))
    }
    return true
  }
  refreshEnvironment() {
    this.scene.environment=this.environmentMap??null
    if(this.mode==='minecraft' && this.skyBackground)this.scene.background=this.skyBackground
  }
  private lightDirection() { return previewLightDirection(this.settings) }
  private fitShadow(light: THREE.DirectionalLight,direction: THREE.Vector3) {
    const points=corners(this.bounds)
    const groundY=this.mode==='minecraft'||(this.mode==='material'&&this.settings.ground)?this.groundY:GROUND_Y
    for(const point of [...points]) {const distance=Math.max(0,(point.y-groundY)/direction.y);points.push(point.clone().addScaledVector(direction,-distance))}
    this.scenery.updateWorldMatrix(true,true);light.shadow.updateMatrices(light)
    const local=new THREE.Box3().setFromPoints(points.map(point=>point.applyMatrix4(light.shadow.camera.matrixWorldInverse)))
    const softnessMargin=penumbraRadius(Math.max(0,this.bounds.max.y-groundY)/direction.y,this.settings.lightSize)
    const margin=Math.max(.5,this.bounds.getSize(new THREE.Vector3()).length()*.02)+softnessMargin,camera=light.shadow.camera
    camera.left=local.min.x-margin;camera.right=local.max.x+margin;camera.bottom=local.min.y-margin;camera.top=local.max.y+margin
    camera.near=Math.max(.1,-local.max.z-margin);camera.far=Math.max(camera.near+1,-local.min.z+margin);camera.updateProjectionMatrix()
  }
  private convertMaterial(source: THREE.Material,mode: PreviewRenderMode) {
    const original=source as THREE.MeshStandardMaterial
    const material=original.isMeshStandardMaterial ? original.clone() : new THREE.MeshStandardMaterial({color:original.color??0xffffff,map:original.map??null,alphaMap:original.alphaMap??null,opacity:source.opacity,transparent:source.transparent,alphaTest:source.alphaTest,side:source.side,depthWrite:source.depthWrite,depthTest:source.depthTest,vertexColors:source.vertexColors,wireframe:original.wireframe??false})
    material.name=source.name;material.userData={...source.userData};material.fog=mode==='minecraft'
    applyPreviewLighting(material,this.lighting)
    this.materials.add(material);return material
  }
  private registerMaterial<T extends THREE.Material>(material:T) {if(material instanceof THREE.MeshStandardMaterial)applyPreviewLighting(material,this.lighting);this.materials.add(material);return material}
  private createNeutralGround(center:THREE.Vector3,size:number) {
    const geometry=new THREE.PlaneGeometry(size,size);this.geometry.add(geometry)
    const ground=new THREE.Mesh(geometry,this.registerMaterial(new THREE.MeshStandardMaterial({color:0x777e86,roughness:.94,metalness:0})))
    ground.rotation.x=-Math.PI/2;ground.position.set(Math.round(center.x),this.groundY,Math.round(center.z));ground.receiveShadow=true;this.scenery.add(ground)
  }
  private pixelTexture(kind:'grass'|'side'|'dirt') {
    const data=new Uint8Array(16*16*4)
    for(let y=0;y<16;y++)for(let x=0;x<16;x++) {const n=((x*37+y*71+x*y*13)%29)-14,grassy=kind==='grass'||(kind==='side'&&y>=13-((x*7)%3)),base=grassy?[102,151,64]:[121,87,58],at=(y*16+x)*4;data[at]=base[0]!+n;data[at+1]=base[1]!+n;data[at+2]=base[2]!+n;data[at+3]=255}
    const texture=new THREE.DataTexture(data,16,16,THREE.RGBAFormat);texture.colorSpace=THREE.SRGBColorSpace;texture.magFilter=THREE.NearestFilter;texture.minFilter=THREE.NearestMipmapLinearFilter;texture.generateMipmaps=true;texture.wrapS=texture.wrapT=THREE.RepeatWrapping;texture.needsUpdate=true;this.textures.add(texture);return texture
  }
  private createMinecraftScenery(center:THREE.Vector3,size:number) {
    this.scene.fog=new THREE.Fog(minecraftAtmosphere(this.settings).horizon,size*(.25-this.settings.skyHaze*.13),size*(.85-this.settings.skyHaze*.25))
    {
      const top=this.pixelTexture('grass');top.repeat.set(size,size);const side=this.pixelTexture('side');side.repeat.set(size,1);const bottom=this.pixelTexture('dirt');bottom.repeat.set(size,size)
      const make=(map:THREE.Texture)=>this.registerMaterial(new THREE.MeshStandardMaterial({map,roughness:1,metalness:0}))
      const geometry=new THREE.BoxGeometry(size,1,size);this.geometry.add(geometry)
      const ground=new THREE.Mesh(geometry,[make(side),make(side),make(top),make(bottom),make(side),make(side)])
      ground.position.set(Math.round(center.x),this.groundY-.5,Math.round(center.z));ground.receiveShadow=true;this.scenery.add(ground)
    }

    // Vanilla-style flat voxel clouds: one instanced draw instead of many cloud meshes.
    this.updateClouds(center)
  }
  private updateClouds(center: THREE.Vector3) {
    const cells: [number,number][]=[]
    for(let z=-12;z<=12;z++)for(let x=-12;x<=12;x++) {
      const cluster=((Math.floor((x+12)/3)*37+Math.floor((z+12)/3)*71)%101)/100
      const detail=((x+17)*29+(z+17)*43)%17/17
      if(cluster*.8+detail*.2 < this.settings.cloudCover*.78)cells.push([x,z])
    }
    if(cells.length && !this.clouds) {
      const geometry=new THREE.BoxGeometry(16,3,16);this.geometry.add(geometry)
      const material=this.registerMaterial(new THREE.MeshStandardMaterial({color:0xf7fbff,roughness:1,fog:true}))
      this.clouds=new THREE.InstancedMesh(geometry,material,625)
      this.clouds.name='Minecraft block clouds';this.scenery.add(this.clouds)
    }
    if(this.clouds) {
      this.clouds.count=cells.length
      const matrix=new THREE.Matrix4()
      cells.forEach(([x,z],index)=>{matrix.makeTranslation(center.x+x*16,48,center.z+z*16);this.clouds!.setMatrixAt(index,matrix)})
      this.clouds.instanceMatrix.needsUpdate=true;this.clouds.computeBoundingSphere()
      if(cells.length) {if(!this.clouds.parent)this.scenery.add(this.clouds)}
      else this.scenery.remove(this.clouds)
    }
  }
  dispose() {this.restoreMaterials();this.clearScenery();this.scene.remove(this.scenery);this.scene.fog=null;if(this.scene.environment===this.environmentMap)this.scene.environment=null;if(this.scene.background===this.skyBackground)this.scene.background=null}
}
