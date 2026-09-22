<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import * as THREE from 'three'
import { PreviewRenderer, filterModelTextures, prepareMaterialTextures } from '../lib/preview-renderer'
import { PreviewEnvironment } from '../lib/preview-environment'
import type { PreviewRenderMode, ViewportSettings } from '../lib/viewport-settings'
import { RoomEnvironment } from 'three/examples/jsm/environments/RoomEnvironment.js'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { getModelPreview, type MaterialBinding } from '../lib/projects'
import { t } from '../i18n'

export interface PreviewTransform { translation: [number, number, number]; rotation: [number, number, number]; scale: [number, number, number] }
export interface PreviewLayer { transform?: PreviewTransform; partTransforms?: Record<string, PreviewTransform>; key: string; assetId: string; layerId: string; carriageId: string; visible: boolean; z: number; reversed: boolean; bogieOffset: number; flipV: boolean; hiddenParts: string[]; bindings: MaterialBinding[] }
export interface PreviewGuide { key: string; length: number; width: number; z: number; reversed: boolean }
const props = withDefaults(defineProps<{ assets: PreviewLayer[]; guides: PreviewGuide[]; selectedPart?: string; selectedLayer?: string; showGrid?: boolean; wireframe?: boolean; thumbnailCarriageId?: string; renderMode?: PreviewRenderMode; settings?: ViewportSettings; cameraView?: 'perspective' | 'front' | 'back' | 'left' | 'right' | 'top'; selectedInstanceKey?: string }>(), { showGrid: true, wireframe: false, renderMode: 'studio' })
const emit = defineEmits<{ select: [selection: { partId: string; layerId: string; carriageId: string; instanceKey?: string }]; error: [message: string]; thumbnail: [carriageId: string, bytes: Uint8Array] }>()
const host = ref<HTMLDivElement>()
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
let environmentTarget: THREE.WebGLRenderTarget | undefined
let selectedObject: THREE.Object3D | undefined
let orthoHeight = 20
let perspectivePose: {position: THREE.Vector3; target: THREE.Vector3; zoom: number} | undefined
let controls: OrbitControls | undefined
let resize: ResizeObserver | undefined
let frame = 0
let content: THREE.Group | undefined
let guides: THREE.Group | undefined
let selection: THREE.Box3Helper | undefined
let grid: THREE.GridHelper | undefined
let generation = 0
let disposed = false
let previousKeys = ''
let minecraftFramed = false
let thumbnailSignature = ''
let pointerStart = new THREE.Vector2()
const byteCache = new Map<string, Promise<ArrayBuffer>>()

function disposeObject(root: THREE.Object3D) {
  root.traverse(object => {
    if (object instanceof THREE.Mesh || object instanceof THREE.LineSegments || object instanceof THREE.Line) {
      object.geometry.dispose()
      for (const material of Array.isArray(object.material) ? object.material : [object.material]) {
        for (const value of Object.values(material)) if (value instanceof THREE.Texture) value.dispose()
        material.dispose()
      }
    }
  })
}
async function rebuild() {
  if (!scene) return
  const version = ++generation
  const group = new THREE.Group()
  let complete = true
  try {
    const results = await Promise.allSettled(props.assets.map(async asset => {
      const cacheKey = JSON.stringify([asset.assetId, asset.bindings])
      if (!byteCache.has(cacheKey)) byteCache.set(cacheKey, getModelPreview(asset.assetId, asset.bindings).catch(error => { byteCache.delete(cacheKey); throw error }))
      const gltf = await new GLTFLoader().parseAsync(await byteCache.get(cacheKey)!, '')
      prepareMaterialTextures(gltf.scene)
      const instance = new THREE.Group(); instance.position.z = asset.z; instance.rotation.y = asset.reversed ? Math.PI : 0
      const offset = new THREE.Group(); offset.position.z = asset.bogieOffset
      applyTransform(gltf.scene, asset.transform)
      gltf.scene.userData.isLayerRoot = true
      gltf.scene.visible = asset.visible
      const flipped = new Set<THREE.BufferGeometry>()
      gltf.scene.traverse(object => {
        object.userData.instanceKey = asset.key; object.userData.layerId = asset.layerId; object.userData.carriageId = asset.carriageId
        if (object.userData.partId) applyTransform(object, asset.partTransforms?.[object.userData.partId])
        if (asset.hiddenParts.includes(object.userData.partId)) object.visible = false
        if (object instanceof THREE.Mesh) {
          if (asset.flipV && !flipped.has(object.geometry)) { flipped.add(object.geometry); const uv = object.geometry.getAttribute('uv'); if (uv) { for (let index = 0; index < uv.count; index++) uv.setY(index, 1 - uv.getY(index)); uv.needsUpdate = true } }
          for (const material of Array.isArray(object.material) ? object.material : [object.material]) material.wireframe = props.wireframe
        }
      })
      offset.add(gltf.scene); instance.add(offset); group.add(instance)
    }))
    const failure = results.find(result => result.status === 'rejected')
    if (failure?.status === 'rejected') throw failure.reason
  } catch (cause) { complete = false; if (version === generation) emit('error', cause instanceof Error ? cause.message : String(cause)) }
  if (disposed || version !== generation) { disposeObject(group); return }
  pipeline?.clearMaterials()
  environment?.restoreMaterials()
  selectedObject = undefined
  if (content) { scene.remove(content); disposeObject(content) }
  content = group; scene.add(content); updateEnvironment(); rebuildGuides(); updateSelection()
  const keys = props.assets.map(item => item.key).join('|') + props.guides.map(item => item.key).join('|')
  if (keys !== previousKeys) { frameContent(); previousKeys = keys }
  const signature = JSON.stringify(props.assets)
  if (complete && props.thumbnailCarriageId && signature !== thumbnailSignature && props.assets.length) {
    thumbnailSignature = signature
    void captureThumbnail(props.thumbnailCarriageId)
  }
}
function rebuildGuides() {
  if (!scene) return
  if (guides) { scene.remove(guides); disposeObject(guides) }
  guides = new THREE.Group()
  for (const guide of props.guides) {
    const shape = new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(-guide.width/2, .015, -guide.length/2), new THREE.Vector3(guide.width/2,.015,-guide.length/2), new THREE.Vector3(guide.width/2,.015,guide.length/2), new THREE.Vector3(-guide.width/2,.015,guide.length/2)])
    const outline = new THREE.LineLoop(shape,new THREE.LineBasicMaterial({color:0x6b8293})); outline.position.z=guide.z; guides.add(outline)
  }
  guides.visible = props.showGrid; scene.add(guides)
}
function contentBounds() {
  content?.updateWorldMatrix(true, true)
  const box = new THREE.Box3()
  content?.traverse(object => { if (object instanceof THREE.Mesh && isVisible(object)) box.expandByObject(object) })
  for (const guide of props.guides) { box.expandByPoint(new THREE.Vector3(-guide.width/2,0,guide.z-guide.length/2)); box.expandByPoint(new THREE.Vector3(guide.width/2,3,guide.z+guide.length/2)) }
  return box
}
function positionCamera(target: THREE.PerspectiveCamera, box: THREE.Box3) {
  const center=box.getCenter(new THREE.Vector3()); const size=Math.max(2,box.getSize(new THREE.Vector3()).length())
  target.position.copy(center).add(props.renderMode === 'minecraft' ? new THREE.Vector3(size*.72,size*.1,size*.82) : new THREE.Vector3(size*.65,size*.4,size*.75)); target.near=Math.max(.01,size/1000); target.far=Math.max(100,size*20); target.lookAt(center); target.updateProjectionMatrix(); return center
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
function frameContent() {
  if (!camera || !controls) return
  const box = contentBounds(); if (box.isEmpty()) return
  const center = box.getCenter(new THREE.Vector3()); const extent = box.getSize(new THREE.Vector3())
  const size = Math.max(2, extent.length()); camera.near = .01; camera.far = Math.max(1000, size * 20)
  if (camera instanceof THREE.PerspectiveCamera) positionCamera(camera, box)
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
}
function changeCamera() {
  if (!renderer) return
  const oldTarget = controls?.target.clone() ?? new THREE.Vector3()
  if (camera instanceof THREE.PerspectiveCamera && controls) perspectivePose = {position: camera.position.clone(), target: oldTarget.clone(), zoom: camera.zoom}
  controls?.dispose()
  camera = props.cameraView && props.cameraView !== 'perspective' ? new THREE.OrthographicCamera(-10,10,10,-10,.01,10000) : new THREE.PerspectiveCamera(45,1,.01,10000)
  camera.position.set(8,5,12)
  pipeline?.setCamera(camera)
  controls = new OrbitControls(camera,renderer.domElement); controls.enableDamping = true
  controls.enableRotate = camera instanceof THREE.PerspectiveCamera; controls.target.copy(oldTarget)
  if (camera instanceof THREE.PerspectiveCamera && perspectivePose) {
    camera.position.copy(perspectivePose.position); camera.zoom = perspectivePose.zoom
    controls.target.copy(perspectivePose.target); camera.lookAt(controls.target)
    updateProjection(); controls.update()
  } else { updateProjection(); frameContent() }
}
function captureThumbnail(carriageId: string) {
  if (!renderer || !scene || !content) return
  const box = contentBounds(); if(box.isEmpty())return
  const thumbCamera = new THREE.PerspectiveCamera(45, 1.5, .01, 10000); positionCamera(thumbCamera,box)
  const size = renderer.getSize(new THREE.Vector2()); const ratio=renderer.getPixelRatio(); const gridVisible=grid?.visible; const guidesVisible=guides?.visible; const selectionVisible=selection?.visible
  let data: string
  try {
    if(grid)grid.visible=false;if(guides)guides.visible=false;if(selection)selection.visible=false
    renderer.setPixelRatio(1);renderer.setSize(240,160,false);pipeline?.setCamera(thumbCamera);pipeline?.setSize(240,160,1);pipeline?.render()
    data=renderer.domElement.toDataURL('image/png')
  } finally {
    renderer.setPixelRatio(ratio);renderer.setSize(size.x,size.y,false);if(camera)pipeline?.setCamera(camera);pipeline?.setSize(size.x,size.y,ratio)
    if(grid)grid.visible=!!gridVisible;if(guides)guides.visible=!!guidesVisible;if(selection)selection.visible=!!selectionVisible
  }
  emit('thumbnail', carriageId, Uint8Array.from(atob(data.split(',')[1]!), character => character.charCodeAt(0)))
}
function updateEnvironment() {
  pipeline?.clearMaterials()
  environment?.apply(content, props.renderMode, props.settings)
  if(content && renderer)filterModelTextures(content,renderer.capabilities.getMaxAnisotropy(),props.settings?.pixelTextures)
  pipeline?.setAO(props.settings?.ambientOcclusion !== false && !props.wireframe)
  if (renderer) { renderer.shadowMap.enabled = props.renderMode !== 'studio'; renderer.shadowMap.needsUpdate = true }
}
function updateWireframe() { pipeline?.clearMaterials();pipeline?.setAO(props.settings?.ambientOcclusion !== false && !props.wireframe); if(renderer)renderer.shadowMap.needsUpdate=true;content?.traverse(object => { if(object instanceof THREE.Mesh)for(const material of Array.isArray(object.material)?object.material:[object.material])material.wireframe=props.wireframe }) }
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
function click(event: MouseEvent) {
  if (!renderer || !camera || !content || pointerStart.distanceTo(new THREE.Vector2(event.clientX,event.clientY))>4) return
  const bounds=renderer.domElement.getBoundingClientRect();const pointer=new THREE.Vector2((event.clientX-bounds.left)/bounds.width*2-1,-(event.clientY-bounds.top)/bounds.height*2+1)
  const raycaster=new THREE.Raycaster();raycaster.setFromCamera(pointer,camera)
  let current:THREE.Object3D|null|undefined=raycaster.intersectObject(content,true).find(hit=>isVisible(hit.object))?.object
  while(current&&!current.userData.partId)current=current.parent
  if(current?.userData.partId)emit('select',{partId:current.userData.partId,layerId:current.userData.layerId,carriageId:current.userData.carriageId,instanceKey:current.userData.instanceKey})
}
onMounted(()=>{
  if(!host.value)return
  scene=new THREE.Scene();scene.background=new THREE.Color(0x191b1f);camera=new THREE.PerspectiveCamera(45,1,.01,10000);camera.position.set(8,5,12)
  try { renderer=new THREE.WebGLRenderer({antialias:true}) } catch(cause) { emit('error',String(cause)); return }
  renderer.setPixelRatio(Math.min(devicePixelRatio,2));renderer.outputColorSpace=THREE.SRGBColorSpace;renderer.toneMapping=THREE.ACESFilmicToneMapping;renderer.toneMappingExposure=1;renderer.shadowMap.enabled=true;renderer.shadowMap.type=THREE.PCFShadowMap;renderer.shadowMap.autoUpdate=false;host.value.append(renderer.domElement)
  pipeline=new PreviewRenderer(renderer,scene,camera)
  changeCamera()
  const room = new RoomEnvironment(); const pmrem = new THREE.PMREMGenerator(renderer)
  environmentTarget = pmrem.fromScene(room, 0.04); room.dispose(); pmrem.dispose()
  environment = new PreviewEnvironment(scene, environmentTarget.texture); updateEnvironment()
  grid=new THREE.GridHelper(100,100,0x42454b,0x26292e);grid.visible=props.showGrid;scene.add(grid)
  renderer.domElement.addEventListener('click',click);renderer.domElement.addEventListener('pointerdown',pointerDown)
  resize=new ResizeObserver(()=>{if(!host.value||!renderer||!camera)return;const{clientWidth,clientHeight}=host.value;renderer.setSize(clientWidth,clientHeight,false);pipeline?.setSize(clientWidth,clientHeight,renderer.getPixelRatio());updateProjection()});resize.observe(host.value)
  const animate=()=>{frame=requestAnimationFrame(animate);controls?.update();if(camera)environment?.updateCamera(camera);updateOrientation();updateSelectionBounds();if(scene&&camera)pipeline?.render()};animate();void rebuild()
})
watch(()=>JSON.stringify([props.assets,props.guides,props.thumbnailCarriageId]),()=>void rebuild())
watch(()=>[props.selectedPart,props.selectedLayer,props.selectedInstanceKey],updateSelection)
watch(()=>props.cameraView,changeCamera)
watch(()=>props.showGrid,value=>{if(grid)grid.visible=value;if(guides)guides.visible=value})
watch(()=>props.wireframe,updateWireframe)
watch(()=>props.settings,()=>{updateEnvironment();updateWireframe()},{deep:true})
watch(()=>props.renderMode,()=>{updateEnvironment();updateWireframe();if(props.renderMode === 'minecraft' && !minecraftFramed)frameContent()})
onBeforeUnmount(()=>{disposed=true;generation++;cancelAnimationFrame(frame);resize?.disconnect();controls?.dispose();pipeline?.dispose();environment?.dispose();environmentTarget?.dispose();if(scene)disposeObject(scene);byteCache.clear();renderer?.dispose();renderer?.domElement.remove()})
defineExpose({fitView:frameContent})
</script>
<template><div ref="host" class="model-viewport"><svg class="orientation" viewBox="-45 -45 90 90" aria-hidden="true"><g v-for="axis in orientationAxes" :key="axis.name" :stroke="axis.color" :fill="axis.color"><line x1="0" y1="0" :x2="axis.x" :y2="axis.y" stroke-width="1.6"/><text :x="axis.x * 1.3" :y="axis.y * 1.3 + 4" text-anchor="middle" stroke="none">{{ axis.name }}</text></g></svg><div v-if="!assets.length" class="viewport-empty">{{ t('previewEmpty') }}</div></div></template>
<style scoped>.orientation{position:absolute;z-index:2;right:12px;top:12px;width:88px;height:88px;pointer-events:none;font-size:11px;font-weight:600}.model-viewport{position:relative;width:100%;height:100%;min-height:220px;overflow:hidden;background:#191b1f}.model-viewport :deep(canvas){display:block;width:100%;height:100%}.viewport-empty{position:absolute;z-index:1;inset:0;display:grid;place-items:center;color:#77838e;font-size:12px;pointer-events:none}</style>
