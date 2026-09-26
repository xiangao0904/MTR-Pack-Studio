<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import * as THREE from 'three'
import { loadPreviewRail, repeatRailModel } from '../lib/preview-rails'
import { type BuiltInRailId } from '../lib/rail-preview-options'
import { PreviewRenderer, filterModelTextures, prepareMaterialTextures } from '../lib/preview-renderer'
import { PreviewEnvironment, type PreviewLightingMood } from '../lib/preview-environment'
import type { PreviewRenderMode, ViewportSettings } from '../lib/viewport-settings'
import { PreviewEnvironmentMap } from '../lib/preview-environment-map'
import { defaultViewportSettings, normalizeViewportSettings } from '../lib/viewport-settings'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { getModelPreview, getModelAsset, type RailDefinition, type MaterialBinding, type RenderStage } from '../lib/projects'
import { t } from '../i18n'

export interface PreviewTransform { translation: [number, number, number]; rotation: [number, number, number]; scale: [number, number, number] }
export interface PreviewLayer { transform?: PreviewTransform; partTransforms?: Record<string, PreviewTransform>; renderStage?: RenderStage; partRenderStages?: Record<string,RenderStage>; key: string; assetId: string; layerId: string; carriageId: string; visible: boolean; z: number; reversed: boolean; bogieOffset: number; flipV: boolean; legacyUvCorrection?: boolean; hiddenParts: string[]; bindings: MaterialBinding[] }
export interface PreviewGuide { key: string; length: number; width: number; z: number; reversed: boolean }
const props = withDefaults(defineProps<{ assets: PreviewLayer[]; guides: PreviewGuide[]; selectedPart?: string; selectedLayer?: string; groundHeight?: number; previewRail?: RailDefinition; builtInRailId?: BuiltInRailId; showRails?: boolean; showGrid?: boolean; wireframe?: boolean; vehicleLightsOn?: boolean; thumbnailCarriageId?: string; thumbnailModelSignature?: string; thumbnailSavedSignature?: string; renderMode?: PreviewRenderMode; settings?: ViewportSettings; cameraView?: 'perspective' | 'front' | 'back' | 'left' | 'right' | 'top'; selectedInstanceKey?: string }>(), { builtInRailId: '', showRails: false, showGrid: true, wireframe: false, vehicleLightsOn: true, renderMode: 'studio' })
const emit = defineEmits<{ select: [selection: { partId: string; layerId: string; carriageId: string; instanceKey?: string }]; clearSelection: []; error: [message: string]; thumbnail: [carriageId: string, signature: string, bytes: Uint8Array] }>()
const host = ref<HTMLDivElement>()
const modeTransitionKey = ref(0)
const orientationAxes = ref([{name:'X',color:'#ed777c',x:28,y:0},{name:'Y',color:'#87dca3',x:0,y:-28},{name:'Z',color:'#80b6f1',x:-20,y:18}])
const lastOrientation = new THREE.Quaternion(0,0,0,0)
function updateOrientation() {
  if (!camera || lastOrientation.equals(camera.quaternion)) return
  lastOrientation.copy(camera.quaternion)
  const inverse = camera.quaternion.clone().invert()
  orientationAxes.value = [new THREE.Vector3(1,0,0),new THREE.Vector3(0,1,0),new THREE.Vector3(0,0,1)].map((axis,index) => {
    axis.applyQuaternion(inverse)
    return {name:['X','Y','Z'][index]!,color:['#ed777c','#87dca3','#80b6f1'][index]!,x:axis.x*28,y:-axis.y*28}
  })
}
let renderer: THREE.WebGLRenderer | undefined
let pipeline: PreviewRenderer | undefined
let scene: THREE.Scene | undefined
let camera: THREE.PerspectiveCamera | THREE.OrthographicCamera | undefined
let environment: PreviewEnvironment | undefined
let environmentMap: PreviewEnvironmentMap | undefined
let skyTimer: number | undefined
let activeSettings: ViewportSettings | undefined
let selectedObject: THREE.Object3D | undefined
let orthoHeight = 20
let perspectivePose: {position: THREE.Vector3; target: THREE.Vector3; zoom: number} | undefined
let controls: OrbitControls | undefined
let resize: ResizeObserver | undefined
let frame = 0
const previewRoot = new THREE.Group()
let railSource: THREE.Group | undefined
let railSourceKey = ''
let railGeneration = 0
let rails: THREE.Group | undefined
let content: THREE.Group | undefined
let guides: THREE.Group | undefined
let selection: THREE.Box3Helper | undefined
let grid: THREE.GridHelper | undefined
let generation = 0
let disposed = false
let previousKeys = ''
let minecraftFramed = false
let cameraTween: {start: number; fromPosition: THREE.Vector3; toPosition: THREE.Vector3; fromTarget: THREE.Vector3; toTarget: THREE.Vector3; fromHeight: number; toHeight: number} | undefined
let lightingTween: {start: number; from: PreviewLightingMood; to: PreviewLightingMood} | undefined
const reducedMotion = () => window.matchMedia('(prefers-reduced-motion: reduce)').matches
const requestedThumbnails = new Map<string, string>()
let pointerStart = new THREE.Vector2()
const byteCache = new Map<string, Promise<ArrayBuffer>>()

function disposeObject(root: THREE.Object3D) {
  const geometries = new Set<THREE.BufferGeometry>()
  const materials = new Set<THREE.Material>()
  const textures = new Set<THREE.Texture>()
  root.traverse(object => {
    if (object instanceof THREE.Mesh || object instanceof THREE.LineSegments || object instanceof THREE.Line) {
      geometries.add(object.geometry)
      for (const material of Array.isArray(object.material) ? object.material : [object.material]) {
        materials.add(material)
        for (const value of Object.values(material)) if (value instanceof THREE.Texture) textures.add(value)
      }
    }
  })
  geometries.forEach(geometry => geometry.dispose())
  materials.forEach(material => material.dispose())
  textures.forEach(texture => texture.dispose())
}
async function rebuild() {
  if (!scene) return
  const version = ++generation
  const group = new THREE.Group()
  const parsed = new Map<string, Promise<THREE.Group>>()
  let complete = true
  try {
    const results = await Promise.allSettled(props.assets.map(async asset => {
      const cacheKey = JSON.stringify([asset.assetId, asset.bindings])
      if (!byteCache.has(cacheKey)) byteCache.set(cacheKey, getModelPreview(asset.assetId, asset.bindings).catch(error => { byteCache.delete(cacheKey); throw error }))
      if (!parsed.has(cacheKey)) parsed.set(cacheKey, (async () => {
        const gltf = await new GLTFLoader().parseAsync(await byteCache.get(cacheKey)!, '')
        prepareMaterialTextures(gltf.scene)
        return gltf.scene
      })())
      const model = (await parsed.get(cacheKey)!).clone(true)
      const instance = new THREE.Group(); instance.position.z = asset.z; instance.rotation.y = asset.reversed ? Math.PI : 0
      const offset = new THREE.Group(); offset.position.z = asset.bogieOffset
      applyTransform(model, asset.transform)
      model.userData.isLayerRoot = true
      model.visible = asset.visible
      const flipped = new Map<THREE.BufferGeometry, THREE.BufferGeometry>()
      model.traverse(object => {
        object.userData.instanceKey = asset.key; object.userData.layerId = asset.layerId; object.userData.carriageId = asset.carriageId
        if (object.userData.partId) applyTransform(object, asset.partTransforms?.[object.userData.partId])
        if (asset.hiddenParts.includes(object.userData.partId)) object.visible = false
        if (object instanceof THREE.Mesh) {
          if (asset.flipV !== Boolean(asset.legacyUvCorrection)) {
            const source = object.geometry
            const existing = flipped.get(source)
            const geometry = existing ?? source.clone()
            if (!existing) {
              const uv = geometry.getAttribute('uv')
              if (uv) { for (let index = 0; index < uv.count; index++) uv.setY(index, 1 - uv.getY(index)); uv.needsUpdate = true }
              flipped.set(source, geometry)
            }
            object.geometry = geometry
          }
          const stage = asset.partRenderStages?.[object.userData.partId] || asset.renderStage || 'EXTERIOR'
          const configure = (source: THREE.Material) => {
            const material = source.clone()
            if (material instanceof THREE.MeshStandardMaterial) {
              material.wireframe = props.wireframe
              if (stage === 'INTERIOR_TRANSLUCENT' || stage === 'ALWAYS_ON_LIGHT') { material.transparent = true; material.depthWrite = false }
              if (stage === 'ALWAYS_ON_LIGHT' || (props.vehicleLightsOn && (stage === 'LIGHT' || stage === 'INTERIOR' || stage === 'INTERIOR_TRANSLUCENT'))) {
                material.emissive = new THREE.Color(stage === 'LIGHT' || stage === 'ALWAYS_ON_LIGHT' ? 0xffffff : 0x777777)
                material.emissiveMap ||= material.map
                material.emissiveIntensity = stage === 'LIGHT' || stage === 'ALWAYS_ON_LIGHT' ? 1 : .4
              }
            }
            return material
          }
          object.material = Array.isArray(object.material) ? object.material.map(configure) : configure(object.material)
        }
      })
      // Imported documents use the pack's reflected X axis. Undo that reflection
      // at the preview boundary so labels and asymmetric details match the source model.
      const coordinateFrame = new THREE.Group()
      coordinateFrame.scale.x = -1
      coordinateFrame.add(model)
      offset.add(coordinateFrame); instance.add(offset); group.add(instance)
    }))
    const failure = results.find(result => result.status === 'rejected')
    if (failure?.status === 'rejected') throw failure.reason
  } catch (cause) { complete = false; if (version === generation) emit('error', cause instanceof Error ? cause.message : String(cause)) }
  if (disposed || version !== generation) { disposeObject(group); return }
  pipeline?.clearMaterials()
  environment?.restoreMaterials()
  selectedObject = undefined
  if (content) { previewRoot.remove(content); disposeObject(content) }
  content = group; previewRoot.add(content); updateEnvironment(); rebuildGuides(); updateSelection()
  const keys = props.assets.map(item => item.key).join('|') + props.guides.map(item => item.key).join('|')
  if (keys !== previousKeys) { frameContent(false); previousKeys = keys }
  const carriageId=props.thumbnailCarriageId,signature=props.thumbnailModelSignature
  if (complete && carriageId && signature && signature!==props.thumbnailSavedSignature && requestedThumbnails.get(carriageId)!==signature && props.assets.length) {
    requestedThumbnails.set(carriageId,signature)
    try { captureThumbnail(carriageId,signature) } catch(cause) { requestedThumbnails.delete(carriageId);emit('error',cause instanceof Error?cause.message:String(cause)) }
  }
}
async function loadProjectRail(rail: RailDefinition) {
  const root = new THREE.Group()
  // Match the coordinate boundary used for train and rail editor assets.
  root.scale.x = -1
  try {
    for (const layer of rail.models.filter(layer => layer.visible)) {
      const asset = await getModelAsset(layer.assetId)
      const { scene: model } = await new GLTFLoader().parseAsync(await getModelPreview(layer.assetId, layer.materialBindings), '')
      root.add(model)
      prepareMaterialTextures(model)
      applyTransform(model, layer.transform)
      const flipped = new Set<THREE.BufferGeometry>()
      model.traverse(object => {
        if (object.userData.partId) applyTransform(object, layer.partTransforms?.[object.userData.partId])
        if (layer.hiddenParts?.includes(object.userData.partId)) object.visible = false
        if (object instanceof THREE.Mesh) {
          object.castShadow = true; object.receiveShadow = true
          if (layer.flipTextureV !== Boolean(asset.legacyUvCorrection) && !flipped.has(object.geometry)) {
            const uv = object.geometry.getAttribute('uv')
            if (uv) { for (let index=0;index<uv.count;index++) uv.setY(index,1-uv.getY(index)); uv.needsUpdate=true }
            flipped.add(object.geometry)
          }
        }
      })
    }
    return root
  } catch (cause) { disposeObject(root); throw cause }
}
async function rebuildRails() {
  if (!scene || disposed) return
  const version = ++railGeneration
  const rail = props.previewRail
  const key = JSON.stringify(rail ?? props.builtInRailId)
  let replacement: THREE.Group | undefined
  try {
    if (props.showRails && (!railSource || railSourceKey !== key)) {
      replacement = rail ? await loadProjectRail(rail) : await loadPreviewRail(props.builtInRailId)
      if (disposed || version !== railGeneration) { disposeObject(replacement); return }
    }
    // Restore materials before detaching or disposing shared segment resources.
    pipeline?.clearMaterials(); environment?.restoreMaterials()
    if (rails) { previewRoot.remove(rails); rails = undefined }
    if (replacement) {
      if (railSource) disposeObject(railSource)
      railSource = replacement; railSourceKey = key; replacement = undefined
    }
    if (props.showRails && railSource) {
      rails = repeatRailModel(railSource, props.guides, rail?.repeatInterval)
      previewRoot.add(rails)
    }
    updateEnvironment()
  } catch (cause) {
    if (replacement) disposeObject(replacement)
    if (disposed || version !== railGeneration) return
    pipeline?.clearMaterials(); environment?.restoreMaterials()
    if (rails) { previewRoot.remove(rails); rails = undefined }
    updateEnvironment()
    emit('error', `${t('railPreviewLoadFailed')} ${cause instanceof Error ? cause.message : String(cause)}`)
  }
}
function rebuildGuides() {
  if (!scene) return
  if (guides) { scene.remove(guides); disposeObject(guides) }
  guides = new THREE.Group()
  for (const guide of props.guides) {
    const width=Math.max(.1,guide.width),length=Math.max(.1,guide.length)
    const plane=new THREE.Mesh(new THREE.PlaneGeometry(width,length),new THREE.MeshStandardMaterial({color:0x3f9fd4,emissive:0x3f9fd4,emissiveIntensity:.3,roughness:1,metalness:0,transparent:true,opacity:.14,side:THREE.DoubleSide,depthWrite:false}))
    plane.rotation.x=-Math.PI/2;plane.position.set(0,.012,guide.z);guides.add(plane)
    const shape=new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(-width/2,.018,-length/2),new THREE.Vector3(width/2,.018,-length/2),new THREE.Vector3(width/2,.018,length/2),new THREE.Vector3(-width/2,.018,length/2)])
    const outline=new THREE.LineLoop(shape,new THREE.LineBasicMaterial({color:0x78c7ef}));outline.position.z=guide.z;guides.add(outline)
  }
  scene.add(guides)
}
function contentBounds() {
  content?.updateWorldMatrix(true, true)
  const box = new THREE.Box3()
  content?.traverse(object => { if (object instanceof THREE.Mesh && isVisible(object)) box.expandByObject(object) })
  for (const guide of props.guides) { box.expandByPoint(new THREE.Vector3(-guide.width/2,0,guide.z-guide.length/2)); box.expandByPoint(new THREE.Vector3(guide.width/2,3,guide.z+guide.length/2)) }
  return box
}
function positionCamera(target: THREE.PerspectiveCamera, box: THREE.Box3, mode: PreviewRenderMode) {
  const center=box.getCenter(new THREE.Vector3()); const size=Math.max(2,box.getSize(new THREE.Vector3()).length())
  target.position.copy(center).add(mode === 'minecraft' ? new THREE.Vector3(size*.72,size*.1-1,size*.82) : new THREE.Vector3(size*.65,size*.4-1,size*.75)); target.near=Math.max(.01,size/1000); target.far=Math.max(100,size*20); target.lookAt(center); target.updateProjectionMatrix(); return center
}
function applyTransform(object: THREE.Object3D, transform?: PreviewTransform) {
  if (!transform) return
  object.position.fromArray(transform.translation)
  object.rotation.set(...transform.rotation, 'XYZ')
  object.scale.fromArray(transform.scale)
}
function updateProjection() {
  if (!camera || !host.value) return
  const aspect = Math.max(1, host.value.clientWidth) / Math.max(1, host.value.clientHeight)
  if (camera instanceof THREE.PerspectiveCamera) camera.aspect = aspect
  else { camera.left = -orthoHeight * aspect / 2; camera.right = orthoHeight * aspect / 2; camera.top = orthoHeight / 2; camera.bottom = -orthoHeight / 2 }
  camera.updateProjectionMatrix()
}
function frameContent(animate = true) {
  if (!camera || !controls) return
  const box = contentBounds(); if (box.isEmpty()) return
  const fromPosition = camera.position.clone(), fromTarget = controls.target.clone(), fromHeight = orthoHeight
  const center = box.getCenter(new THREE.Vector3()); const extent = box.getSize(new THREE.Vector3())
  const size = Math.max(2, extent.length()); camera.near = .01; camera.far = Math.max(1000, size * 20)
  if (camera instanceof THREE.PerspectiveCamera) positionCamera(camera, box, props.renderMode)
  else {
    const directions = {front: [0,0,1], back: [0,0,-1], left: [1,0,0], right: [-1,0,0], top: [0,1,0]} as const
    const view = props.cameraView === 'perspective' ? 'front' : props.cameraView ?? 'front'
    camera.up.set(0, view === 'top' ? 0 : 1, view === 'top' ? -1 : 0)
    camera.position.copy(center).add(new THREE.Vector3(...directions[view]).multiplyScalar(size * 2))
    const aspect = Math.max(1,host.value?.clientWidth ?? 1) / Math.max(1,host.value?.clientHeight ?? 1)
    const width = view === 'left' || view === 'right' ? extent.z : extent.x
    const height = view === 'top' ? extent.z : extent.y
    orthoHeight = Math.max(2, height, width / aspect) * 1.2; camera.zoom = 1; updateProjection()
  }
  if (props.renderMode === 'minecraft') minecraftFramed = true
  controls.target.copy(center); camera.lookAt(center); controls.update()
  const toPosition = camera.position.clone(), toTarget = controls.target.clone(), toHeight = orthoHeight
  cameraTween = undefined
  if (animate && !reducedMotion()) {
    camera.position.copy(fromPosition); controls.target.copy(fromTarget); orthoHeight = fromHeight; updateProjection(); camera.lookAt(fromTarget)
    cameraTween = {start: performance.now(), fromPosition, toPosition, fromTarget, toTarget, fromHeight, toHeight}
  }
}
function changeCamera() {
  if (!renderer) return
  cameraTween = undefined
  const oldTarget = controls?.target.clone() ?? new THREE.Vector3()
  if (camera instanceof THREE.PerspectiveCamera && controls) perspectivePose = {position: camera.position.clone(), target: oldTarget.clone(), zoom: camera.zoom}
  controls?.dispose()
  camera = props.cameraView && props.cameraView !== 'perspective' ? new THREE.OrthographicCamera(-10,10,10,-10,.01,10000) : new THREE.PerspectiveCamera(45,1,.01,10000)
  camera.position.set(8,4,12)
  pipeline?.setCamera(camera)
  controls = new OrbitControls(camera,renderer.domElement); controls.enableDamping = true
  controls.enableRotate = camera instanceof THREE.PerspectiveCamera; controls.target.copy(oldTarget)
  if (camera instanceof THREE.PerspectiveCamera && perspectivePose) {
    camera.position.copy(perspectivePose.position); camera.zoom = perspectivePose.zoom
    controls.target.copy(perspectivePose.target); camera.lookAt(controls.target)
    updateProjection(); controls.update()
  } else { updateProjection(); frameContent(false) }
}
function captureThumbnail(carriageId: string, signature: string) {
  if (!renderer || !content) return
  const box = contentBounds(); if(box.isEmpty())return
  const thumbCamera = new THREE.PerspectiveCamera(45, 1.5, .01, 10000); positionCamera(thumbCamera,box,'studio')
  const thumbScene=new THREE.Scene(),thumbContent=content.clone(true);thumbScene.add(thumbContent)
  const size = renderer.getSize(new THREE.Vector2()),ratio=renderer.getPixelRatio()
  let thumbPipeline:PreviewRenderer|undefined,thumbMap:PreviewEnvironmentMap|undefined,thumbEnvironment:PreviewEnvironment|undefined,data:string|undefined
  try {
    const settings=defaultViewportSettings('studio')
    thumbPipeline=new PreviewRenderer(renderer,thumbScene,thumbCamera)
    thumbMap=new PreviewEnvironmentMap(renderer)
    thumbEnvironment=new PreviewEnvironment(thumbScene,thumbMap.update('studio',settings),thumbPipeline.lighting)
    thumbEnvironment.apply(thumbContent,'studio',settings)
    thumbContent.traverse(object=>{if(object instanceof THREE.Mesh)for(const material of Array.isArray(object.material)?object.material:[object.material])material.wireframe=false})
    filterModelTextures(thumbContent,renderer.capabilities.getMaxAnisotropy(),false)
    thumbPipeline.setAO(settings.ambientOcclusion);thumbPipeline.setIndirect(settings.indirectIntensity)
    renderer.setPixelRatio(1);renderer.setSize(240,160,false);thumbPipeline.setSize(240,160,1);renderer.shadowMap.needsUpdate=true;thumbPipeline.render()
    data=renderer.domElement.toDataURL('image/png')
  } finally {
    renderer.setPixelRatio(ratio);renderer.setSize(size.x,size.y,false);renderer.shadowMap.needsUpdate=true
    if(content)filterModelTextures(content,renderer.capabilities.getMaxAnisotropy(),props.settings?.pixelTextures)
    thumbEnvironment?.dispose();thumbPipeline?.dispose();thumbMap?.dispose()
  }
  if(data)emit('thumbnail', carriageId, signature, Uint8Array.from(atob(data.split(',')[1]!), character => character.charCodeAt(0)))
}
function updateEnvironment() {
  lightingTween=undefined
  if(skyTimer!==undefined) {clearTimeout(skyTimer);skyTimer=undefined}
  pipeline?.clearMaterials()
  const settings = normalizeViewportSettings(props.renderMode, props.settings)
  activeSettings=settings
  if (environmentMap) environment?.setEnvironmentMap(environmentMap.update(props.renderMode, settings), environmentMap.background)
  environment?.apply(previewRoot, props.renderMode, settings, props.groundHeight)
  pipeline?.setIndirect(props.wireframe ? 0 : settings.indirectIntensity)
  if(renderer)filterModelTextures(previewRoot,renderer.capabilities.getMaxAnisotropy(),props.settings?.pixelTextures)
  pipeline?.setAO(props.settings?.ambientOcclusion !== false && !props.wireframe)
  if (renderer) { renderer.shadowMap.enabled = true; renderer.shadowMap.needsUpdate = true }
}
function updateViewportSettings() {
  lightingTween=undefined
  if(!renderer || !environment || !environmentMap) return
  const settings=normalizeViewportSettings(props.renderMode,props.settings)
  if(!environment.updateSettings(props.renderMode,settings)) {updateEnvironment();return}
  const previous=activeSettings
  activeSettings=settings
  if(previous?.pixelTextures!==settings.pixelTextures)filterModelTextures(previewRoot,renderer.capabilities.getMaxAnisotropy(),settings.pixelTextures)
  pipeline?.setAO(settings.ambientOcclusion && !props.wireframe)
  pipeline?.setIndirect(props.wireframe?0:settings.indirectIntensity)
  if(!previous || previous.lightAzimuth!==settings.lightAzimuth || previous.lightElevation!==settings.lightElevation || previous.lightSize!==settings.lightSize || previous.shadowQuality!==settings.shadowQuality)renderer.shadowMap.needsUpdate=true
  if(environmentMap.needsUpdate(props.renderMode,settings)) {
    if(skyTimer!==undefined)clearTimeout(skyTimer)
    skyTimer=window.setTimeout(()=>{
      skyTimer=undefined
      if(disposed || !environment || !environmentMap)return
      const latest=normalizeViewportSettings(props.renderMode,props.settings)
      environment.setEnvironmentMap(environmentMap.update(props.renderMode,latest),environmentMap.background)
      environment.refreshEnvironment()
    },120)
  }
}
function updateWireframe() { pipeline?.setIndirect(props.wireframe ? 0 : normalizeViewportSettings(props.renderMode, props.settings).indirectIntensity); pipeline?.clearMaterials();pipeline?.setAO(props.settings?.ambientOcclusion !== false && !props.wireframe); if(renderer)renderer.shadowMap.needsUpdate=true;content?.traverse(object => { if(object instanceof THREE.Mesh)for(const material of Array.isArray(object.material)?object.material:[object.material])material.wireframe=props.wireframe }) }
function updateSelection() {
  if (!scene) return
  selectedObject = undefined
  if (selection) {scene.remove(selection); disposeObject(selection)} selection = undefined
  content?.traverse(object => {
    if (selectedObject || !isVisible(object) || object.userData.layerId !== props.selectedLayer) return
    if (props.selectedInstanceKey && object.userData.instanceKey !== props.selectedInstanceKey) return
    if (props.selectedPart ? object.userData.partId === props.selectedPart : object.userData.isLayerRoot) selectedObject = object
  })
  if (selectedObject) {
    selection = new THREE.Box3Helper(new THREE.Box3(),0xe0e3e9); scene.add(selection); updateSelectionBounds()
  }
}
function updateSelectionBounds() {
  if (!selection || !selectedObject) return
  selectedObject.updateWorldMatrix(true, true)
  selection.box.makeEmpty()
  selectedObject.traverse(object => {
    if (!(object instanceof THREE.Mesh) || !isVisible(object)) return
    if (!object.geometry.boundingBox) object.geometry.computeBoundingBox()
    if (object.geometry.boundingBox) selection!.box.union(object.geometry.boundingBox.clone().applyMatrix4(object.matrixWorld))
  })
  selection.visible = !selection.box.isEmpty()
}
function isVisible(object: THREE.Object3D): boolean { return object.visible && (!object.parent || isVisible(object.parent)) }
function pointerDown(event:PointerEvent){pointerStart.set(event.clientX,event.clientY)}
function cancelCameraTween(){cameraTween=undefined}
function advanceCameraTween(now:number){
  if(!cameraTween||!camera||!controls)return
  const tween=cameraTween, progress=Math.min(1,(now-tween.start)/260), eased=1-Math.pow(1-progress,3)
  camera.position.lerpVectors(tween.fromPosition,tween.toPosition,eased)
  controls.target.lerpVectors(tween.fromTarget,tween.toTarget,eased)
  if(camera instanceof THREE.OrthographicCamera){orthoHeight=THREE.MathUtils.lerp(tween.fromHeight,tween.toHeight,eased);updateProjection()}
  camera.lookAt(controls.target)
  if(progress===1)cameraTween=undefined
}
function click(event: MouseEvent) {
  if (!renderer || !camera || !content || pointerStart.distanceTo(new THREE.Vector2(event.clientX,event.clientY))>4) return
  const bounds=renderer.domElement.getBoundingClientRect();const pointer=new THREE.Vector2((event.clientX-bounds.left)/bounds.width*2-1,-(event.clientY-bounds.top)/bounds.height*2+1)
  const raycaster=new THREE.Raycaster();raycaster.setFromCamera(pointer,camera)
  let current:THREE.Object3D|null|undefined=raycaster.intersectObject(content,true).find(hit=>isVisible(hit.object))?.object
  if(!current){emit('clearSelection');return}
  while(current&&!current.userData.partId)current=current.parent
  if(current?.userData.partId)emit('select',{partId:current.userData.partId,layerId:current.userData.layerId,carriageId:current.userData.carriageId,instanceKey:current.userData.instanceKey})
}
onMounted(()=>{
  if(!host.value)return
  scene=new THREE.Scene();scene.add(previewRoot);scene.background=new THREE.Color(0x191b1f);camera=new THREE.PerspectiveCamera(45,1,.01,10000);camera.position.set(8,4,12)
  try { renderer=new THREE.WebGLRenderer({antialias:true}) } catch(cause) { emit('error',String(cause)); return }
  renderer.setPixelRatio(Math.min(devicePixelRatio,2));renderer.outputColorSpace=THREE.SRGBColorSpace;renderer.toneMapping=THREE.ACESFilmicToneMapping;renderer.toneMappingExposure=1;renderer.shadowMap.enabled=true;renderer.shadowMap.type=THREE.BasicShadowMap;renderer.shadowMap.autoUpdate=false;host.value.append(renderer.domElement)
  pipeline=new PreviewRenderer(renderer,scene,camera)
  changeCamera()
  environmentMap = new PreviewEnvironmentMap(renderer)
  environment = new PreviewEnvironment(scene, undefined, pipeline.lighting); updateEnvironment()
  grid=new THREE.GridHelper(100,100,0x42454b,0x26292e);grid.visible=props.showGrid;scene.add(grid)
  renderer.domElement.addEventListener('click',click);renderer.domElement.addEventListener('pointerdown',pointerDown);renderer.domElement.addEventListener('pointerdown',cancelCameraTween);renderer.domElement.addEventListener('wheel',cancelCameraTween)
  resize=new ResizeObserver(()=>{if(!host.value||!renderer||!camera)return;const{clientWidth,clientHeight}=host.value;renderer.setSize(clientWidth,clientHeight,false);pipeline?.setSize(clientWidth,clientHeight,renderer.getPixelRatio());updateProjection()});resize.observe(host.value)
  const animate=(now:number)=>{frame=requestAnimationFrame(animate);advanceCameraTween(now);if(lightingTween&&environment){const tween=lightingTween,progress=Math.min(1,(now-tween.start)/260);environment.blendLighting(tween.from,tween.to,1-Math.pow(1-progress,3));if(progress===1)lightingTween=undefined}controls?.update();updateOrientation();updateSelectionBounds();if(scene&&camera)pipeline?.render()};frame=requestAnimationFrame(animate);void rebuild();void rebuildRails()
})
watch(()=>JSON.stringify([props.assets,props.vehicleLightsOn,props.thumbnailCarriageId,props.thumbnailModelSignature]),()=>void rebuild())
watch(()=>JSON.stringify(props.guides),()=>{rebuildGuides();void rebuildRails()})
watch(()=>[props.showRails,JSON.stringify(props.previewRail),props.builtInRailId],()=>void rebuildRails())
watch(()=>props.groundHeight,updateEnvironment)
watch(()=>[props.selectedPart,props.selectedLayer,props.selectedInstanceKey],updateSelection)
watch(()=>props.cameraView,changeCamera)
watch(()=>props.showGrid,value=>{if(grid)grid.visible=value})
watch(()=>props.wireframe,updateWireframe)
watch(()=>props.settings,updateViewportSettings,{deep:true})
watch(()=>props.renderMode,()=>{const from=environment?.lightingMood();updateEnvironment();const to=environment?.lightingMood();if(from&&to&&!reducedMotion()){lightingTween={start:performance.now(),from,to};environment?.blendLighting(from,to,0);modeTransitionKey.value++}updateWireframe();if(props.renderMode === 'minecraft' && !minecraftFramed)frameContent()})
onBeforeUnmount(()=>{disposed=true;generation++;railGeneration++;if(skyTimer!==undefined)clearTimeout(skyTimer);cancelAnimationFrame(frame);resize?.disconnect();controls?.dispose();pipeline?.dispose();environment?.dispose();environmentMap?.dispose();if(rails)previewRoot.remove(rails);if(railSource)disposeObject(railSource);if(scene)disposeObject(scene);byteCache.clear();renderer?.domElement.removeEventListener('pointerdown',cancelCameraTween);renderer?.domElement.removeEventListener('wheel',cancelCameraTween);renderer?.dispose();renderer?.domElement.remove()})
defineExpose({fitView:()=>frameContent()})
</script>
<template><div ref="host" class="model-viewport"><div v-if="modeTransitionKey" :key="modeTransitionKey" class="viewport-mode-wash" aria-hidden="true"/><svg class="orientation" viewBox="-45 -45 90 90" aria-hidden="true"><g v-for="axis in orientationAxes" :key="axis.name" :stroke="axis.color" :fill="axis.color"><line x1="0" y1="0" :x2="axis.x" :y2="axis.y" stroke-width="1.6"/><text :x="axis.x * 1.3" :y="axis.y * 1.3 + 4" text-anchor="middle" stroke="none">{{ axis.name }}</text></g></svg><div v-if="!assets.length" class="viewport-empty">{{ t('previewEmpty') }}</div></div></template>
<style scoped>.orientation{position:absolute;z-index:2;right:12px;top:12px;width:88px;height:88px;pointer-events:none;font-size:11px;font-weight:600}.model-viewport{position:relative;width:100%;height:100%;min-height:220px;overflow:hidden;background:#191b1f}.model-viewport :deep(canvas){display:block;width:100%;height:100%}.viewport-mode-wash{position:absolute;z-index:1;inset:0;pointer-events:none;background:#252930;animation:mode-reveal 260ms ease-out both}@keyframes mode-reveal{from{opacity:.22}to{opacity:0}}.viewport-empty{position:absolute;z-index:1;inset:0;display:grid;place-items:center;color:#77838e;font-size:12px;pointer-events:none}@media(prefers-reduced-motion:reduce){.viewport-mode-wash{animation:none;opacity:0}}</style>
