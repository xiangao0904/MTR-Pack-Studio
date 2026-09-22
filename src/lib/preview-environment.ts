import * as THREE from 'three'

export type PreviewRenderMode = 'studio' | 'unlit' | 'minecraft'
type MeshMaterial = THREE.Material | THREE.Material[]

/** Owns preview-only materials and scenery. Source GLB materials are never changed. */
export class PreviewEnvironment {
  private scenery = new THREE.Group()
  private originals = new Map<THREE.Mesh, MeshMaterial>()
  private materials = new Set<THREE.Material>()
  private textures = new Set<THREE.Texture>()
  private geometry = new Set<THREE.BufferGeometry>()
  private scene: THREE.Scene
  constructor(scene: THREE.Scene) { this.scene = scene; scene.add(this.scenery) }

  restoreMaterials() {
    for (const [mesh, material] of this.originals) mesh.material = material
    this.originals.clear()
    for (const material of this.materials) material.dispose()
    for (const texture of this.textures) texture.dispose()
    this.materials.clear(); this.textures.clear()
  }

  apply(root: THREE.Object3D | undefined, mode: PreviewRenderMode) {
    this.restoreMaterials()
    this.scenery.clear(); this.scenery.position.set(0,0,0)
    for (const geometry of this.geometry) geometry.dispose()
    this.geometry.clear()
    this.scene.background = new THREE.Color(mode === 'minecraft' ? 0x83b6f4 : 0x151a1f)
    this.scene.fog = mode === 'minecraft' ? new THREE.Fog(0xbed9f1, 180, 430) : null
    if (mode === 'studio') {
      this.scenery.add(new THREE.HemisphereLight(0xdcecff,0x30353a,2.4))
      const light = new THREE.DirectionalLight(0xffffff,2.2); light.position.set(8,12,9); this.scenery.add(light)
    }
    if (mode === 'minecraft') this.createMinecraftScenery(root)
    if (mode === 'studio' || !root) return
    root.traverse(object => {
      if (!(object instanceof THREE.Mesh)) return
      this.originals.set(object,object.material)
      const convert = (source: THREE.Material) => this.convertMaterial(source,mode)
      object.material = Array.isArray(object.material) ? object.material.map(convert) : convert(object.material)
    })
  }

  private convertMaterial(source: THREE.Material, mode: PreviewRenderMode) {
    const original = source as THREE.MeshStandardMaterial
    const material = new THREE.MeshBasicMaterial({
      color: original.color?.clone() ?? new THREE.Color(0xffffff), map: original.map ?? null,
      alphaMap: original.alphaMap ?? null, transparent: source.transparent, opacity: source.opacity,
      alphaTest: source.alphaTest, side: source.side, depthWrite: source.depthWrite, depthTest: source.depthTest,
      vertexColors: source.vertexColors, blending: source.blending, premultipliedAlpha: source.premultipliedAlpha,
      wireframe: original.wireframe ?? false, fog: mode === 'minecraft',
    })
    material.name = source.name; material.userData = {...source.userData}
    if (mode === 'minecraft') {
      if (material.map) {
        const texture = material.map.clone(); texture.magFilter = THREE.NearestFilter; texture.needsUpdate = true
        material.map = texture; this.textures.add(texture)
      }
      material.onBeforeCompile = shader => {
        shader.vertexShader = 'varying vec3 previewFacingNormal;\n' + shader.vertexShader
        shader.vertexShader = shader.vertexShader.replace('#include <begin_vertex>', '#include <begin_vertex>\npreviewFacingNormal = inverseTransformDirection(normalMatrix * normal, viewMatrix);')
        shader.fragmentShader = 'varying vec3 previewFacingNormal;\n' + shader.fragmentShader
        shader.fragmentShader = shader.fragmentShader.replace('#include <opaque_fragment>', `
          vec3 facing = normalize(previewFacingNormal);
          float shade = dot(facing * facing, vec3(0.6, facing.y >= 0.0 ? 1.0 : 0.5, 0.8));
          outgoingLight *= shade;
          #include <opaque_fragment>
        `)
      }
      material.customProgramCacheKey = () => 'studio-minecraft-facing-v1'
    }
    this.materials.add(material); return material
  }

  private pixelTexture(kind: 'grass' | 'side' | 'dirt') {
    const data = new Uint8Array(16*16*4)
    for (let y=0;y<16;y++) for(let x=0;x<16;x++) {
      const n = ((x*37+y*71+x*y*13)%29)-14
      const grassy = kind === 'grass' || (kind === 'side' && y >= 13 - ((x*7)%3))
      const base = grassy ? [102,151,64] : [121,87,58]
      const at=(y*16+x)*4
      data[at]=base[0]!+n;data[at+1]=base[1]!+n;data[at+2]=base[2]!+n;data[at+3]=255
    }
    const texture=new THREE.DataTexture(data,16,16,THREE.RGBAFormat)
    texture.colorSpace=THREE.SRGBColorSpace;texture.magFilter=THREE.NearestFilter;texture.minFilter=THREE.NearestMipmapLinearFilter
    texture.generateMipmaps=true;texture.wrapS=texture.wrapT=THREE.RepeatWrapping;texture.needsUpdate=true
    this.textures.add(texture);return texture
  }

  private createMinecraftScenery(root?: THREE.Object3D) {
    const bounds=root ? new THREE.Box3().setFromObject(root) : new THREE.Box3()
    const extent=bounds.isEmpty() ? new THREE.Vector3() : bounds.getSize(new THREE.Vector3())
    const center=bounds.isEmpty() ? new THREE.Vector3() : bounds.getCenter(new THREE.Vector3())
    const size=Math.max(512,Math.ceil(Math.max(extent.x,extent.z))*2)
    this.scenery.position.set(Math.round(center.x),0,Math.round(center.z))
    this.scene.fog=new THREE.Fog(0xbed9f1,size*.35,size*.85)
    const top=this.pixelTexture('grass');top.repeat.set(size,size)
    const side=this.pixelTexture('side');side.repeat.set(size,1)
    const bottom=this.pixelTexture('dirt');bottom.repeat.set(size,size)
    const make=(map:THREE.Texture,color=0xffffff)=>{const material=new THREE.MeshBasicMaterial({map,color});this.materials.add(material);return material}
    const groundGeometry=new THREE.BoxGeometry(size,1,size);this.geometry.add(groundGeometry)
    const ground=new THREE.Mesh(groundGeometry,[make(side,0xaaaaaa),make(side,0xaaaaaa),make(top),make(bottom,0x808080),make(side,0xcccccc),make(side,0xcccccc)])
    ground.position.y=-.52;this.scenery.add(ground)
    const cloudGeometry=new THREE.BoxGeometry(1,1,1);this.geometry.add(cloudGeometry)
    const cloudMaterial=new THREE.MeshBasicMaterial({color:0xf7fbff});this.materials.add(cloudMaterial)
    for(let index=0;index<24;index++) {
      const cloud=new THREE.Mesh(cloudGeometry,cloudMaterial)
      cloud.position.set(((index*83)%380)-190,44+(index%3)*2,((index*137)%380)-190)
      cloud.scale.set(15+(index%4)*9,3,9+(index%5)*6);this.scenery.add(cloud)
    }
    const sunMaterial=new THREE.SpriteMaterial({color:0xfff8d5,fog:false});this.materials.add(sunMaterial)
    const sun=new THREE.Sprite(sunMaterial);sun.position.set(-110,55,-180);sun.scale.set(22,22,1);this.scenery.add(sun)
  }

  dispose() {
    this.restoreMaterials()
    for (const geometry of this.geometry) geometry.dispose()
    this.geometry.clear();this.scene.remove(this.scenery);this.scenery.clear();this.scene.fog=null
  }
}

