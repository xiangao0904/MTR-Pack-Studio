<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { getModelPreview, type MaterialBinding } from '../lib/projects'
import { t } from '../i18n'

export interface PreviewLayer { key: string; assetId: string; layerId: string; carriageId: string; visible: boolean; z: number; reversed: boolean; bogieOffset: number; flipV: boolean; hiddenParts: string[]; bindings: MaterialBinding[] }
export interface PreviewGuide { key: string; length: number; width: number; z: number; reversed: boolean }
const props = withDefaults(defineProps<{ assets: PreviewLayer[]; guides: PreviewGuide[]; selectedPart?: string; selectedLayer?: string; showGrid?: boolean; wireframe?: boolean; thumbnailCarriageId?: string }>(), { showGrid: true, wireframe: false })
const emit = defineEmits<{ select: [selection: { partId: string; layerId: string; carriageId: string }]; error: [message: string]; thumbnail: [carriageId: string, bytes: Uint8Array] }>()
const host = ref<HTMLDivElement>()
let renderer: THREE.WebGLRenderer | undefined
let scene: THREE.Scene | undefined
let camera: THREE.PerspectiveCamera | undefined
let controls: OrbitControls | undefined
let resize: ResizeObserver | undefined
let frame = 0
let content: THREE.Group | undefined
let guides: THREE.Group | undefined
let selection: THREE.BoxHelper | undefined
let grid: THREE.GridHelper | undefined
let generation = 0
let disposed = false
let previousKeys = ''
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
      const instance = new THREE.Group(); instance.position.z = asset.z; instance.rotation.y = asset.reversed ? Math.PI : 0
      gltf.scene.position.z = asset.bogieOffset
      gltf.scene.visible = asset.visible
      const flipped = new Set<THREE.BufferGeometry>()
      gltf.scene.traverse(object => {
        object.userData.layerId = asset.layerId; object.userData.carriageId = asset.carriageId
        if (asset.hiddenParts.includes(object.userData.partId)) object.visible = false
        if (object instanceof THREE.Mesh) {
          if (asset.flipV && !flipped.has(object.geometry)) { flipped.add(object.geometry); const uv = object.geometry.getAttribute('uv'); if (uv) { for (let index = 0; index < uv.count; index++) uv.setY(index, 1 - uv.getY(index)); uv.needsUpdate = true } }
          for (const material of Array.isArray(object.material) ? object.material : [object.material]) material.wireframe = props.wireframe
        }
      })
      instance.add(gltf.scene); group.add(instance)
    }))
    const failure = results.find(result => result.status === 'rejected')
    if (failure?.status === 'rejected') throw failure.reason
  } catch (cause) { complete = false; if (version === generation) emit('error', cause instanceof Error ? cause.message : String(cause)) }
  if (disposed || version !== generation) { disposeObject(group); return }
  if (content) { scene.remove(content); disposeObject(content) }
  content = group; scene.add(content); rebuildGuides(); updateSelection()
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
    const arrow = new THREE.ArrowHelper(new THREE.Vector3(0,0,guide.reversed?-1:1),new THREE.Vector3(guide.width/2+.5,.15,guide.z),Math.max(1,guide.length*.25),0x9bc8ed,.45,.25); guides.add(arrow)
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
  target.position.copy(center).add(new THREE.Vector3(size*.65,size*.4,size*.75)); target.near=Math.max(.01,size/1000); target.far=Math.max(100,size*20); target.lookAt(center); target.updateProjectionMatrix(); return center
}
function frameContent() { if (!camera || !controls) return; const box=contentBounds(); if(box.isEmpty())return; controls.target.copy(positionCamera(camera,box));controls.update() }
function captureThumbnail(carriageId: string) {
  if (!renderer || !scene || !content) return
  const box = contentBounds(); if(box.isEmpty())return
  const thumbCamera = new THREE.PerspectiveCamera(45, 1.5, .01, 10000); positionCamera(thumbCamera,box)
  const size = renderer.getSize(new THREE.Vector2()); const ratio=renderer.getPixelRatio(); const gridVisible=grid?.visible; const guidesVisible=guides?.visible; const selectionVisible=selection?.visible
  if(grid)grid.visible=false;if(guides)guides.visible=false;if(selection)selection.visible=false
  renderer.setPixelRatio(1);renderer.setSize(240,160,false);renderer.render(scene,thumbCamera)
  const data=renderer.domElement.toDataURL('image/png')
  renderer.setPixelRatio(ratio);renderer.setSize(size.x,size.y,false)
  if(grid)grid.visible=!!gridVisible;if(guides)guides.visible=!!guidesVisible;if(selection)selection.visible=!!selectionVisible
  emit('thumbnail', carriageId, Uint8Array.from(atob(data.split(',')[1]!), character => character.charCodeAt(0)))
}
function updateWireframe() { content?.traverse(object => { if(object instanceof THREE.Mesh)for(const material of Array.isArray(object.material)?object.material:[object.material])material.wireframe=props.wireframe }) }
function updateSelection() {
  if(!scene)return
  if(selection){scene.remove(selection);disposeObject(selection)}selection=undefined
  if(!props.selectedPart)return
  let selected:THREE.Object3D|undefined
  content?.traverse(object=>{if(!selected && object.userData.partId===props.selectedPart && object.userData.layerId===props.selectedLayer && isVisible(object))selected=object})
  if(selected){selection=new THREE.BoxHelper(selected,0xc6e2ff);scene.add(selection)}
}
function isVisible(object: THREE.Object3D): boolean { return object.visible && (!object.parent || isVisible(object.parent)) }
function pointerDown(event:PointerEvent){pointerStart.set(event.clientX,event.clientY)}
function click(event: MouseEvent) {
  if (!renderer || !camera || !content || pointerStart.distanceTo(new THREE.Vector2(event.clientX,event.clientY))>4) return
  const bounds=renderer.domElement.getBoundingClientRect();const pointer=new THREE.Vector2((event.clientX-bounds.left)/bounds.width*2-1,-(event.clientY-bounds.top)/bounds.height*2+1)
  const raycaster=new THREE.Raycaster();raycaster.setFromCamera(pointer,camera)
  let current:THREE.Object3D|null|undefined=raycaster.intersectObject(content,true).find(hit=>isVisible(hit.object))?.object
  while(current&&!current.userData.partId)current=current.parent
  if(current?.userData.partId)emit('select',{partId:current.userData.partId,layerId:current.userData.layerId,carriageId:current.userData.carriageId})
}
onMounted(()=>{
  if(!host.value)return
  scene=new THREE.Scene();scene.background=new THREE.Color(0x151a1f);camera=new THREE.PerspectiveCamera(45,1,.01,10000);camera.position.set(8,5,12)
  try { renderer=new THREE.WebGLRenderer({antialias:true}) } catch(cause) { emit('error',String(cause)); return }
  renderer.setPixelRatio(Math.min(devicePixelRatio,2));renderer.outputColorSpace=THREE.SRGBColorSpace;host.value.append(renderer.domElement)
  controls=new OrbitControls(camera,renderer.domElement);controls.enableDamping=true
  scene.add(new THREE.HemisphereLight(0xdcecff,0x30353a,2.4));const sun=new THREE.DirectionalLight(0xffffff,2.2);sun.position.set(8,12,9);scene.add(sun)
  grid=new THREE.GridHelper(100,100,0x52606c,0x28323a);grid.visible=props.showGrid;scene.add(grid);scene.add(new THREE.AxesHelper(2))
  renderer.domElement.addEventListener('click',click);renderer.domElement.addEventListener('pointerdown',pointerDown)
  resize=new ResizeObserver(()=>{if(!host.value||!renderer||!camera)return;const{clientWidth,clientHeight}=host.value;renderer.setSize(clientWidth,clientHeight,false);camera.aspect=clientWidth/Math.max(clientHeight,1);camera.updateProjectionMatrix()});resize.observe(host.value)
  const animate=()=>{frame=requestAnimationFrame(animate);controls?.update();selection?.update();if(scene&&camera)renderer?.render(scene,camera)};animate();void rebuild()
})
watch(()=>JSON.stringify([props.assets,props.guides,props.thumbnailCarriageId]),()=>void rebuild())
watch(()=>[props.selectedPart,props.selectedLayer],updateSelection)
watch(()=>props.showGrid,value=>{if(grid)grid.visible=value;if(guides)guides.visible=value})
watch(()=>props.wireframe,updateWireframe)
onBeforeUnmount(()=>{disposed=true;generation++;cancelAnimationFrame(frame);resize?.disconnect();controls?.dispose();if(scene)disposeObject(scene);byteCache.clear();renderer?.dispose();renderer?.domElement.remove()})
defineExpose({fitView:frameContent})
</script>
<template><div ref="host" class="model-viewport"><div v-if="!assets.length" class="viewport-empty">{{ t('previewEmpty') }}</div></div></template>
<style scoped>.model-viewport{position:relative;width:100%;height:100%;min-height:220px;overflow:hidden;background:#151a1f}.model-viewport :deep(canvas){display:block;width:100%;height:100%}.viewport-empty{position:absolute;z-index:1;inset:0;display:grid;place-items:center;color:#77838e;font-size:12px;pointer-events:none}</style>
