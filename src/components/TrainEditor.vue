<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { AlertTriangle, ArrowDown, ArrowLeft, ArrowUp, Box, BoxSelect, ChevronRight, Copy, Eye, EyeOff, FileBox, Grid3X3, Maximize, Plus, RotateCw, Search, Trash2, Upload, Undo2, Redo2 } from '@lucide/vue'
import ModelViewport, { type PreviewLayer } from './ModelViewport.vue'
import MaterialEditor from './MaterialEditor.vue'
import type { MaterialProperties, TextureChannel } from '../lib/projects'
import ModelHierarchy from './ModelHierarchy.vue'
import ViewportModeControls from './ViewportModeControls.vue'
import { loadViewportPreferences, type PreviewRenderMode } from '../lib/viewport-settings'
import { EditorHistory, trainHistorySnapshot } from '../lib/editor-history'
import { t } from '../i18n'
import { analyzeModelImport, chooseModelDependency, chooseModelFile, chooseTextureFile, importTextureFile, storeImageBytes, getImageAsset, getModelAsset, getTrain, importModel, updateTrain, type AssetDefinition, type CarPlacementRule, type CarriageDefinition, type ContentEntry, type ModelLayer, type TrainDefinition, type ValidationIssue } from '../lib/projects'
import { arrangeConsist, matchesPlacement } from '../lib/train-preview'
import { SaveQueue } from '../lib/save-queue'

const props = defineProps<{ projectPath: string; entry: ContentEntry }>()
const emit = defineEmits<{ back: []; export: []; changed: [entry: ContentEntry]; status: [value: 'saving' | 'saved' | 'failed']; error: [message: string]; ready: [] }>()
type Tab = 'general' | 'models' | 'placement' | 'mtr3'
const train = ref<TrainDefinition>()
const selectedCarriageId = ref('')
const selectedLayerId = ref('')
const selectedPartId = ref('')
const selectedInstanceKey = ref('')
const storedRenderMode = localStorage.getItem('mtr-pack-studio:render-mode')
const renderMode = ref<PreviewRenderMode>(storedRenderMode === 'unlit' ? 'material' : storedRenderMode === 'material' || storedRenderMode === 'minecraft' ? storedRenderMode : 'studio')
watch(renderMode, value => localStorage.setItem('mtr-pack-studio:render-mode', value))
const viewportSettings = ref(loadViewportPreferences())
watch(viewportSettings, value => localStorage.setItem('mtr-pack-studio:viewport-settings', JSON.stringify(value)), {deep:true})
const cameraView = ref<'perspective'|'front'|'back'|'left'|'right'|'top'>('perspective')
const showConsist = ref(false)
const history = new EditorHistory<TrainDefinition>({ normalize: trainHistorySnapshot })
const historyTick = ref(0)
const canUndo = computed(() => { void historyTick.value; return !!train.value && (history.canUndo || history.hasChanges(train.value)) })
const canRedo = computed(() => { void historyTick.value; return !!train.value && history.canRedo && !history.hasChanges(train.value) })
const tab = ref<Tab>('general')
const assets = ref<Record<string, AssetDefinition>>({})
const importing = ref(false)
const loading = ref(true)
const carriageQuery = ref('')
const partQuery = ref('')
const treeTab = ref<'model' | 'materials'>('model')
function loadViewFlags() {
  try { const saved = JSON.parse(localStorage.getItem('mtr-pack-studio:view-flags') || '{}'); return Object.fromEntries((['studio','material','minecraft'] as const).map(mode=>[mode,{grid:typeof saved?.[mode]?.grid==='boolean'?saved[mode].grid:mode==='studio',wireframe:saved?.[mode]?.wireframe===true}])) as Record<PreviewRenderMode,{grid:boolean;wireframe:boolean}> } catch { return {studio:{grid:true,wireframe:false},material:{grid:false,wireframe:false},minecraft:{grid:false,wireframe:false}} }
}
const viewFlags = ref(loadViewFlags())
watch(viewFlags, value=>localStorage.setItem('mtr-pack-studio:view-flags',JSON.stringify(value)),{deep:true})
const showGrid = computed({get:()=>viewFlags.value[renderMode.value].grid,set:value=>{viewFlags.value[renderMode.value].grid=value}})
const wireframe = computed({get:()=>viewFlags.value[renderMode.value].wireframe,set:value=>{viewFlags.value[renderMode.value].wireframe=value}})
const viewMode = ref<'single'|'consist'>('single')
const simulateRules = ref(false)
const selectedInstance = ref(0)
const dragIndex = ref<number>()
const thumbnails = ref<Record<string,string>>({})
const viewport = ref<{ fitView: () => void }>()
const editorRoot = ref<HTMLElement>()
let timer: number | undefined
let hydrating = true
let alive = true
let operationPromise: Promise<void> | undefined
const thumbnailJobs = new Set<Promise<void>>()
const queue = new SaveQueue(async () => {
  if (!train.value) return
  const snapshot = JSON.parse(JSON.stringify(train.value)) as TrainDefinition
  const updated = await updateTrain(props.projectPath,snapshot,snapshot.revision)
  hydrating = true; train.value.revision = updated.revision; hydrating = false
  emit('changed',{...props.entry,name:updated.name,updatedAt:Date.now()})
})
const carriage = computed(() => train.value?.carriages.find(item => item.id === selectedCarriageId.value))
const layers = computed(() => carriage.value ? [...carriage.value.bodyModels, ...carriage.value.bogie1Models, ...carriage.value.bogie2Models] : [])
const selectedLayer = computed(() => layers.value.find(item => item.id === selectedLayerId.value))
const selectedAsset = computed(() => selectedLayer.value ? assets.value[selectedLayer.value.assetId] : undefined)
const previewInstances = computed(() => !train.value ? [] : viewMode.value==='consist' ? arrangeConsist(train.value) : carriage.value ? [{carriage:carriage.value,index:0,z:0,reversed:false}] : [])
const viewportAssets = computed<PreviewLayer[]>(() => previewInstances.value.flatMap(instance => {
  const car=instance.carriage; const count=previewInstances.value.length; const position=instance.index+1
  return ([{models:car.bodyModels,offset:0},{models:car.bogie1Models,offset:car.bogie1Position},{models:car.bogie2Models,offset:car.bogie2Position}]).flatMap(group=>group.models.map(layer=>({
    key:`${instance.index}:${layer.id}`,assetId:layer.assetId,layerId:layer.id,carriageId:car.id,visible:layer.visible,z:instance.z,reversed:instance.reversed,bogieOffset:group.offset,flipV:layer.flipTextureV,legacyUvCorrection:assets.value[layer.assetId]?.legacyUvCorrection,bindings:layer.materialBindings,
    transform: layer.transform, partTransforms: layer.partTransforms,
    hiddenParts:[...(layer.hiddenParts || []), ...(simulateRules.value ? (assets.value[layer.assetId]?.parts||[]).filter(part=>!matchesPlacement(layer.partRules[part.id]||car.placement,position,count)).map(part=>part.id) : [])],
  })))
}))
const viewportGuides=computed(()=>previewInstances.value.map(item=>({key:`${item.index}:${item.carriage.id}`,length:item.carriage.length,width:item.carriage.width,z:item.z,reversed:item.reversed})))
const filteredCarriages = computed(() => train.value?.carriages.filter(item => item.name.toLowerCase().includes(carriageQuery.value.toLowerCase())) || [])
const triangleCount = computed(() => layers.value.reduce((sum,layer) => sum+(assets.value[layer.assetId]?.parts.reduce((count,part)=>count+part.triangleCount,0)||0),0))
const slotGroups = computed(() => carriage.value ? [
  { key: 'body' as const, label: t('body'), layers: carriage.value.bodyModels },
  { key: 'bogie1' as const, label: t('frontBogie'), layers: carriage.value.bogie1Models },
  { key: 'bogie2' as const, label: t('rearBogie'), layers: carriage.value.bogie2Models },
] : [])
const placementRule = computed<CarPlacementRule | undefined>(() => !carriage.value ? undefined : selectedPartId.value && selectedLayer.value ? selectedLayer.value.partRules[selectedPartId.value] : carriage.value.placement)
async function load() {
  try {
    hydrating=true;train.value=await getTrain(props.projectPath,props.entry.id);selectedCarriageId.value=train.value.carriages[0]?.id||''
    await loadAssets();history.reset(train.value);historyTick.value++;await nextTick();emit('status','saved');emit('ready')
  } catch(cause){emit('error',message(cause));emit('status','failed')}
  finally{hydrating=false;loading.value=false}
}
async function loadAssets() {
  const all=train.value?.carriages.flatMap(car=>[...car.bodyModels,...car.bogie1Models,...car.bogie2Models])||[]
  await Promise.all([...new Set(all.map(layer=>layer.assetId))].filter(id=>!assets.value[id]).map(async id=>{try{assets.value[id]=await getModelAsset(id)}catch(cause){emit('error',message(cause))}}))
  await Promise.all((train.value?.carriages||[]).filter(car=>car.thumbnailHash&&!thumbnails.value[car.id]).map(async car=>{try{const bytes=await getImageAsset(car.thumbnailHash!);thumbnails.value[car.id]=URL.createObjectURL(new Blob([bytes],{type:'image/png'}))}catch(cause){emit('error',message(cause))}}))
}
watch(train,()=>{if(!hydrating)scheduleSave()},{deep:true,flush:'sync'})
watch(selectedCarriageId,()=>{selectedLayerId.value='';selectedPartId.value='';selectedInstanceKey.value=''},{flush:'sync'})
watch(selectedLayerId,()=>{selectedPartId.value=''},{flush:'sync'})
onMounted(()=>{void load();window.addEventListener('keydown',editorShortcut)})
onBeforeUnmount(()=>{alive=false;window.removeEventListener('keydown',editorShortcut);if(timer)window.clearTimeout(timer);Object.values(thumbnails.value).forEach(url=>URL.revokeObjectURL(url))})
function scheduleSave(){queue.markDirty();emit('status','saving');if(timer)window.clearTimeout(timer);timer=window.setTimeout(()=>void flush().catch(()=>{}),800)}
async function flush(){ if(operationPromise)await operationPromise; await flushEdits() }
async function flushEdits(){
  if(timer)window.clearTimeout(timer);timer=undefined
  try{await Promise.all([...thumbnailJobs]);if(train.value){history.record(train.value);historyTick.value++}await queue.flush();emit('status','saved')}
  catch(cause){emit('status','failed');emit('error',message(cause));throw cause}
}
async function immediate(action:()=>void){if(train.value)history.record(train.value);action();await flush()}
function perform(action:()=>void){if(importing.value)return;void immediate(action).catch(()=>{})}
async function back(){try{await flush();emit('back')}catch{/* keep the editor open */}}
async function focusIssue(issue:ValidationIssue){
  if(issue.carriageId)selectedCarriageId.value=issue.carriageId
  const field=issue.field||'';tab.value=field.includes('placement')||field.includes('partRules')?'placement':/model|asset|texture|material/i.test(field)?'models':field.includes('mtr3')?'mtr3':'general'
  const matching=layers.value.find(layer=>layer.id===issue.layerId||field.includes(layer.id));if(matching)selectedLayerId.value=matching.id
  if(field.startsWith('partRules.'))selectedPartId.value=field.slice('partRules.'.length)
  if(field.includes('material'))treeTab.value='materials'
  await nextTick();const input=editorRoot.value?.querySelector<HTMLInputElement>(`[data-field="${CSS.escape(field)}"]`);input?.focus();input?.scrollIntoView({block:'nearest'})
}
defineExpose({flush,focusIssue})
function uniqueId(base:string){const used=new Set(train.value?.carriages.map(car=>car.exportId));let value=base;let n=2;while(used.has(value))value=`${base}_${n++}`;return value}
function addCarriage(){perform(()=>{if(!train.value)return;const number=train.value.carriages.length+1;const item:CarriageDefinition={id:crypto.randomUUID(),exportId:uniqueId(`carriage_${number}`),name:`${t('carriage')} ${number}`,length:20,width:3,bogie1Position:7,bogie2Position:-7,couplingPadding1:0,couplingPadding2:0,end1:{gangway:false,barrier:false},end2:{gangway:false,barrier:false},placement:{preset:'all',offset:0,whitelist:'',blacklist:''},bodyModels:[],bogie1Models:[],bogie2Models:[]};train.value.carriages.push(item);selectedCarriageId.value=item.id})}
function duplicateCarriage(){perform(()=>{if(!train.value||!carriage.value)return;const copy=JSON.parse(JSON.stringify(carriage.value)) as CarriageDefinition;copy.id=crypto.randomUUID();copy.name+=` ${t('copySuffix')}`;copy.exportId=uniqueId(`${copy.exportId}_copy`);for(const layer of [...copy.bodyModels,...copy.bogie1Models,...copy.bogie2Models])layer.id=crypto.randomUUID();train.value.carriages.push(copy);selectedCarriageId.value=copy.id;void loadAssets()})}
function removeCarriage(){perform(()=>{if(!train.value||!carriage.value||train.value.carriages.length===1)return;const id=carriage.value.id;train.value.carriages=train.value.carriages.filter(item=>item.id!==id);train.value.previewConsist=train.value.previewConsist.filter(item=>item.carriageId!==id);selectedCarriageId.value=train.value.carriages[0]!.id})}
function moveCarriage(direction:-1|1){perform(()=>{if(!train.value||!carriage.value)return;const index=train.value.carriages.indexOf(carriage.value);const next=index+direction;if(next<0||next>=train.value.carriages.length)return;[train.value.carriages[index],train.value.carriages[next]]=[train.value.carriages[next]!,train.value.carriages[index]!]})}
function selectLayer(layer:ModelLayer){selectedLayerId.value=layer.id;selectedPartId.value='';selectedInstanceKey.value='';tab.value='models'}
function selectPart(partId:string,layerId=selectedLayerId.value,carriageId=selectedCarriageId.value,instanceKey=''){selectedCarriageId.value=carriageId;selectedLayerId.value=layerId;selectedPartId.value=partId;selectedInstanceKey.value=instanceKey}
function clearSelection(){selectedLayerId.value='';selectedPartId.value='';selectedInstanceKey.value=''}
function selectTree(layerId:string,partId?:string){const layer=layers.value.find(item=>item.id===layerId);if(!layer)return;if(partId)selectPart(partId,layerId);else selectLayer(layer)}
function toggleVisibility(layerId:string|null,partIds?:string[]){perform(()=>{
  if(!layerId){const visible=!layers.value.some(layer=>layer.visible);for(const layer of layers.value)layer.visible=visible;return}
  const layer=layers.value.find(item=>item.id===layerId);if(!layer)return
  if(!partIds){layer.visible=!layer.visible;return}
  const hidden=new Set(layer.hiddenParts||[]);const hide=layer.visible&&partIds.some(id=>!hidden.has(id));if(!hide)layer.visible=true
  for(const id of partIds)hide?hidden.add(id):hidden.delete(id);layer.hiddenParts=[...hidden]
})}
async function travelHistory(direction:'undo'|'redo'){
  if(!train.value||importing.value)return
  if(timer)window.clearTimeout(timer);timer=undefined
  try{await Promise.all([...thumbnailJobs]);await queue.flush();const restored=history[direction](train.value);historyTick.value++;if(!restored)return
    restored.revision=train.value.revision;for(const car of restored.carriages)car.thumbnailHash=train.value.carriages.find(item=>item.id===car.id)?.thumbnailHash
    hydrating=true;train.value=restored;hydrating=false;if(!restored.carriages.some(car=>car.id===selectedCarriageId.value))selectedCarriageId.value=restored.carriages[0]?.id||''
    if(!layers.value.some(layer=>layer.id===selectedLayerId.value))selectedLayerId.value=''
    queue.markDirty();emit('status','saving');await loadAssets();await queue.flush();emit('status','saved')
  }catch(cause){emit('error',message(cause));emit('status','failed')}
}
function editorShortcut(event:KeyboardEvent){
  if(importing.value||loading.value)return
  const input=event.target instanceof HTMLElement&&!!event.target.closest('input,textarea,select,[contenteditable=true]')
  if(input)return
  const key=event.key.toLowerCase()
  if((event.ctrlKey||event.metaKey)&&(key==='z'||key==='y')){event.preventDefault();void travelHistory(key==='y'||event.shiftKey?'redo':'undo');return}
  if(event.ctrlKey||event.metaKey||event.altKey)return
  if(key==='f')viewport.value?.fitView()
}
function updateTags(event:Event){if(train.value)train.value.tags=(event.target as HTMLInputElement).value.split(',').map(value=>value.trim()).filter(Boolean)}
function setPartOverride(enabled:boolean){if(!selectedLayer.value||!selectedPartId.value||!carriage.value)return;if(enabled)selectedLayer.value.partRules[selectedPartId.value]=JSON.parse(JSON.stringify(carriage.value.placement));else delete selectedLayer.value.partRules[selectedPartId.value]}
function removeLayer(layer:ModelLayer){perform(()=>{for(const group of slotGroups.value){const index=group.layers.indexOf(layer);if(index>=0)group.layers.splice(index,1)}if(selectedLayerId.value===layer.id)selectedLayerId.value=''})}
function addModel(slot:'body'|'bogie1'|'bogie2',replace?:ModelLayer){
  operationPromise=addModelImpl(slot,replace).finally(()=>{operationPromise=undefined});return operationPromise
}
async function addModelImpl(slot:'body'|'bogie1'|'bogie2',replace?:ModelLayer){
  if(!train.value||!carriage.value||importing.value)return
  importing.value=true
  try{
    const path=await chooseModelFile();if(!path)return
    await flushEdits();const trainId=train.value.id;const carriageId=carriage.value.id
    const overrides:Record<string,string>={};let analysis=await analyzeModelImport(path,overrides)
    while(analysis.missingDependencies.length){for(const missing of analysis.missingDependencies){const name=missing.split(/[\\/]/).pop()||missing;const resolved=await chooseModelDependency(name);if(!resolved)throw new Error(`${t('missingDependencies')}: ${name}`);overrides[name]=resolved}analysis=await analyzeModelImport(path,overrides)}
    const result=await importModel(trainId,carriageId,slot,path,overrides,train.value.revision)
    hydrating=true;train.value=result.train;assets.value[result.asset.id]=result.asset;hydrating=false
    const group=slotGroups.value.find(item=>item.key===slot)!.layers;const added=group.at(-1)!
    if(replace){const index=group.findIndex(item=>item.id===replace.id);added.id=replace.id;added.name=replace.name;added.visible=replace.visible;added.flipTextureV=replace.flipTextureV;added.transform=replace.transform;const materialIds=new Set(result.asset.materials.map(item=>item.id));const partIds=new Set(result.asset.parts.map(item=>item.id));added.hiddenParts=(replace.hiddenParts||[]).filter(id=>partIds.has(id));added.partTransforms=Object.fromEntries(Object.entries(replace.partTransforms||{}).filter(([id])=>partIds.has(id)));added.materialBindings=replace.materialBindings.filter(item=>materialIds.has(item.materialId));added.partRules=Object.fromEntries(Object.entries(replace.partRules).filter(([id])=>partIds.has(id)));if(index>=0){group.pop();group.splice(index,1,added)}}
    selectedLayerId.value=added.id;await loadAssets();await flushEdits();emit('changed',{...props.entry,updatedAt:Date.now()})
  }catch(cause){emit('error',message(cause));emit('status','failed')}finally{importing.value=false;hydrating=false}
}
function replaceTexture(materialId:string,channel?:TextureChannel){operationPromise=replaceTextureImpl(materialId,channel).finally(()=>{operationPromise=undefined});return operationPromise}
async function replaceTextureImpl(materialId:string,channel?:TextureChannel){
  if(!selectedLayer.value||importing.value)return
  const layer=selectedLayer.value;importing.value=true
  try{const path=await chooseTextureFile();if(!path)return;await flushEdits();const hash=await importTextureFile(path);let binding=layer.materialBindings.find(item=>item.materialId===materialId);if(!binding){binding={materialId};layer.materialBindings.push(binding)}
    if(channel){binding.properties ||= {};binding.properties.maps ||= {};binding.properties.maps[channel]=hash;
      if(channel==='metalness'||channel==='roughness')binding.properties[channel]=1;
      if(channel==='emissive')binding.properties.emissive=[1,1,1];
    }else binding.textureAssetId=hash;await flushEdits()}
  catch(cause){emit('error',message(cause))}finally{importing.value=false}
}
function editMaterial(materialId:string,properties:MaterialProperties){perform(()=>{const layer=selectedLayer.value;if(!layer)return;const binding=layer.materialBindings.find(item=>item.materialId===materialId);if(binding)binding.properties=properties;else layer.materialBindings.push({materialId,properties})})}
function resetTexture(materialId:string){perform(()=>{if(selectedLayer.value)selectedLayer.value.materialBindings=selectedLayer.value.materialBindings.filter(item=>item.materialId!==materialId)})}
function addInstance(){showConsist.value=true;perform(()=>{if(train.value&&carriage.value){train.value.previewConsist.push({carriageId:carriage.value.id,reversed:false});selectedInstance.value=train.value.previewConsist.length-1;viewMode.value='consist'}})}
function moveInstance(from:number,to:number){perform(()=>{if(!train.value||to<0||to>=train.value.previewConsist.length||from===to)return;const[item]=train.value.previewConsist.splice(from,1);train.value.previewConsist.splice(to,0,item!);selectedInstance.value=to})}
function removeInstance(){perform(()=>{train.value?.previewConsist.splice(selectedInstance.value,1);selectedInstance.value=Math.max(0,selectedInstance.value-1)})}
function reverseInstance(){perform(()=>{const item=train.value?.previewConsist[selectedInstance.value];if(item)item.reversed=!item.reversed})}
function dropInstance(index:number){if(dragIndex.value!==undefined)moveInstance(dragIndex.value,index);dragIndex.value=undefined}
function onThumbnail(id:string,bytes:Uint8Array){
  const job=(async()=>{try{const hash=await storeImageBytes(bytes);if(!alive)return;const car=train.value?.carriages.find(item=>item.id===id);if(car&&car.thumbnailHash!==hash){car.thumbnailHash=hash;if(thumbnails.value[id])URL.revokeObjectURL(thumbnails.value[id]);thumbnails.value[id]=URL.createObjectURL(new Blob([bytes.slice().buffer],{type:'image/png'}))}}catch(cause){emit('error',message(cause))}})()
  thumbnailJobs.add(job);void job.finally(()=>thumbnailJobs.delete(job))
}
function message(cause:unknown){return cause instanceof Error?cause.message:String(cause)}
</script>
<template>
  <fieldset v-if="train" ref="editorRoot" class="train-editor" :disabled="importing || loading">
    <header class="editor-toolbar">
      <div class="tool-group history-tools"><button :disabled="!canUndo" :title="`${t('undo')} (Ctrl+Z)`" @click="travelHistory('undo')"><Undo2 :size="17" /><span>{{ t('undo') }}</span></button><button :disabled="!canRedo" :title="`${t('redo')} (Ctrl+Shift+Z)`" @click="travelHistory('redo')"><Redo2 :size="17" /><span>{{ t('redo') }}</span></button></div>

      <div class="tool-group view-tools"><label class="camera-picker"><Box :size="17" /><select v-model="cameraView" :aria-label="t('perspective')"><option v-for="view in (['perspective','front','back','left','right','top'] as const)" :key="view" :value="view">{{ t(view==='back'?'viewBack':view) }}</option></select></label><button :class="{active:showGrid}" @click="showGrid=!showGrid"><Grid3X3 :size="17" /><span>{{ t('grid') }}</span></button><button :class="{active:wireframe}" @click="wireframe=!wireframe"><BoxSelect :size="17" /><span>{{ t('wireframe') }}</span></button><button @click="viewport?.fitView()"><Maximize :size="17" /><span>{{ t('fitView') }}</span></button></div>
      <ViewportModeControls v-model:mode="renderMode" v-model:settings="viewportSettings[renderMode]" v-model:show-grid="showGrid" v-model:wireframe="wireframe" />
      <div class="tool-spacer" /><button class="editor-export" @click="emit('export')"><Upload :size="18" />{{ t('exportPack') }}</button>
    </header>
    <div class="editor-body">
      <aside class="carriage-panel">
        <div class="train-identity"><div class="train-thumbnail"><img :src="thumbnails[selectedCarriageId] || '/images/train-placeholder.svg'" alt="" /></div><div><strong>{{ train.name }}</strong><span>{{ train.exportId }}</span></div></div>
        <label class="carriage-search"><Search :size="15" /><input v-model="carriageQuery" :placeholder="t('searchCarriages')" /></label>
        <div class="panel-heading"><strong>{{ t('carriages') }}</strong><button @click="addCarriage"><Plus :size="15" />{{ t('add') }}</button></div>
        <div class="carriage-cards">
          <button v-for="item in filteredCarriages" :key="item.id" :class="['carriage-card',{active:item.id===selectedCarriageId}]" @click="selectedCarriageId=item.id">
            <span class="carriage-preview"><img :src="thumbnails[item.id] || '/images/train-placeholder.svg'" alt="" /></span><span class="carriage-copy"><strong>{{ item.name }}</strong><small>{{ item.exportId }} · {{ item.length }} m</small></span><span class="carriage-number">{{ train.carriages.indexOf(item) + 1 }}</span>
          </button>
        </div>
        <div class="carriage-actions"><button :title="t('moveUp')" @click="moveCarriage(-1)"><ArrowUp :size="16" /></button><button :title="t('moveDown')" @click="moveCarriage(1)"><ArrowDown :size="16" /></button><button :title="t('duplicate')" @click="duplicateCarriage"><Copy :size="16" /><span>{{ t('duplicate') }}</span></button><button :title="t('delete')" :disabled="train.carriages.length===1" @click="removeCarriage"><Trash2 :size="16" /><span>{{ t('delete') }}</span></button></div>
      </aside>

      <main :class="['editor-center',{'with-consist':showConsist}]">
        <section class="viewport-panel"><ModelViewport ref="viewport" :assets="viewportAssets" :guides="viewportGuides" :selected-part="selectedPartId" :selected-layer="selectedLayerId" :selected-instance-key="selectedInstanceKey" :render-mode="renderMode" :settings="viewportSettings[renderMode]" :camera-view="cameraView" :show-grid="showGrid" :wireframe="wireframe" :thumbnail-carriage-id="viewMode==='single' && !simulateRules && !importing ? selectedCarriageId : undefined" @select="selection=>selectPart(selection.partId,selection.layerId,selection.carriageId,selection.instanceKey)" @clear-selection="clearSelection" @error="emit('error',$event)" @thumbnail="onThumbnail" /><div v-if="renderMode==='minecraft'" :class="['environment-caption',{'with-warning':selectedAsset?.warnings.length}]" :title="t('minecraftPreviewHint')">{{ t('minecraftScale') }}</div><div class="viewport-badge"><Box :size="14" />{{ carriage?.name }} · {{ carriage?.length }} × {{ carriage?.width }} m</div><div v-if="selectedAsset?.warnings.length" class="model-warning"><AlertTriangle :size="15" />{{ selectedAsset.warnings.join(' ') }}</div></section>
        <section v-if="showConsist" class="consist-panel"><div class="consist-heading"><select v-model="viewMode" :aria-label="t('previewConsist')"><option value="single">{{ t('singleCarriage') }}</option><option value="consist">{{ t('previewConsist') }}</option></select><button :class="{active:simulateRules}" @click="simulateRules=!simulateRules">{{ t('simulateRules') }}</button><button @click="addInstance"><Plus :size="13" />{{ t('addSelectedCarriage') }}</button><span class="tool-spacer" /><button :disabled="!train.previewConsist[selectedInstance]" :title="t('reverseCarriage')" @click="reverseInstance"><RotateCw :size="14" /></button><button :disabled="selectedInstance===0" :title="t('moveUp')" @click="moveInstance(selectedInstance,selectedInstance-1)"><ArrowLeft :size="14" /></button><button :disabled="selectedInstance>=train.previewConsist.length-1" :title="t('moveDown')" @click="moveInstance(selectedInstance,selectedInstance+1)"><ChevronRight :size="14" /></button><button :disabled="!train.previewConsist.length" :title="t('delete')" @click="removeInstance"><Trash2 :size="14" /></button></div><div class="consist-items"><button v-for="(instance,index) in train.previewConsist" :key="index" draggable="true" :class="{active:selectedInstance===index}" @dragstart="dragIndex=index" @dragover.prevent @drop.prevent="dropInstance(index)" @click="selectedInstance=index;selectedCarriageId=instance.carriageId"><span>{{ index+1 }}</span>{{ train.carriages.find(item=>item.id===instance.carriageId)?.name }}<span>{{ instance.reversed ? '←' : '→' }}</span></button><small v-if="!train.previewConsist.length">{{ t('emptyConsist') }}</small></div></section>
        <section class="hierarchy-panel">
          <div class="hierarchy-tabs"><button :class="{active:showConsist}" @click="showConsist=!showConsist">{{ t('showConsist') }}</button><button :class="{active:treeTab==='model'}" @click="treeTab='model'">{{ t('modelTree') }}</button><button :class="{active:treeTab==='materials'}" @click="treeTab='materials'">{{ t('materials') }}</button></div>
          <label class="tree-search"><Search :size="14" /><input v-model="partQuery" :placeholder="t('searchParts')" /></label>
          <ModelHierarchy v-if="treeTab==='model'" :storage-key="`mtr-pack-studio:tree:${projectPath}:${entry.id}:${selectedCarriageId}`" :name="carriage?.name || ''" :layers="layers" :assets="assets" :query="partQuery" :selected-layer="selectedLayerId" :selected-part="selectedPartId" @select="selectTree" @visibility="toggleVisibility" />
          <div v-else class="materials-list"><div v-if="!selectedAsset" class="tree-empty">{{ t('selectModelLayer') }}</div><template v-else><MaterialEditor v-for="material in selectedAsset.materials" :key="material.id" :material="material" :binding="selectedLayer?.materialBindings.find(item=>item.materialId===material.id)" @change="editMaterial(material.id,$event)" @texture="replaceTexture(material.id,$event)" @reset="resetTexture(material.id)" /></template></div>
        </section>
        <footer class="editor-status"><span>{{ layers.length }} {{ t('modelLayers') }}</span><i /> <span>{{ triangleCount.toLocaleString() }} {{ t('triangles') }}</span><i /><span>{{ t('unitsMetres') }}</span><i /><span>+Z {{ t('forward') }}</span><button class="status-back" @click="back"><ArrowLeft :size="13" />{{ t('trains') }}</button><span class="status-right"><Grid3X3 :size="13" />{{ t('gridSize') }}</span></footer>
      </main>

      <aside class="inspector">
        <nav class="inspector-tabs"><button v-for="item in (['general','models','placement','mtr3'] as Tab[])" :key="item" :class="{active:tab===item}" @click="tab=item">{{ t(item === 'mtr3' ? 'mtr3Short' : item === 'models' ? 'modelsShort' : item === 'placement' ? 'placementShort' : item) }}</button></nav>
        <div class="inspector-content">
          <template v-if="tab==='general'"><section class="inspector-section"><h2>{{ t('train') }}</h2><label>{{ t('trainName') }}<input v-model="train.name" maxlength="80" /></label><label class="color-field">{{ t('color') }}<input type="color" :value="`#${train.color}`" @input="train.color=($event.target as HTMLInputElement).value.slice(1)" /><span class="color-value">#{{ train.color }}</span></label><label>{{ t('exportId') }}<input data-field="exportId" v-model="train.exportId" /></label><label>{{ t('description') }}<textarea v-model="train.description" rows="3" /></label><label>{{ t('tags') }}<input :value="train.tags.join(', ')" @change="updateTags" /></label></section><section v-if="carriage" class="inspector-section"><h2>{{ t('selectedCarriage') }}</h2><label>{{ t('fieldName') }}<input v-model="carriage.name" /></label><label>{{ t('exportId') }}<input v-model="carriage.exportId" /></label><div class="field-grid"><label>{{ t('length') }}<input data-field="length" min="0.1" v-model.number="carriage.length" type="number" step="0.1" /></label><label>{{ t('width') }}<input data-field="width" min="0.1" v-model.number="carriage.width" type="number" step="0.1" /></label><label>{{ t('bogie1') }}<input v-model.number="carriage.bogie1Position" type="number" step="0.1" /></label><label>{{ t('bogie2') }}<input v-model.number="carriage.bogie2Position" type="number" step="0.1" /></label><label>{{ t('frontCoupling') }}<input v-model.number="carriage.couplingPadding1" type="number" min="0" step="0.1" /></label><label>{{ t('rearCoupling') }}<input v-model.number="carriage.couplingPadding2" type="number" min="0" step="0.1" /></label></div><label class="switch-row"><span>{{ t('frontGangway') }}</span><input v-model="carriage.end1.gangway" type="checkbox" /></label><label class="switch-row"><span>{{ t('rearGangway') }}</span><input v-model="carriage.end2.gangway" type="checkbox" /></label><label class="switch-row"><span>{{ t('frontBarrier') }}</span><input v-model="carriage.end1.barrier" type="checkbox" /></label><label class="switch-row"><span>{{ t('rearBarrier') }}</span><input v-model="carriage.end2.barrier" type="checkbox" /></label></section></template>
          <template v-else-if="tab==='models' && carriage"><section v-for="group in slotGroups" :key="group.key" class="model-card"><div class="model-card-head"><h2>{{ group.label }}</h2><button :disabled="importing" @click="addModel(group.key)"><Upload :size="14" />{{ group.layers.length ? t('addLayer') : t('import') }}</button></div><div v-if="!group.layers.length" class="empty-slot"><FileBox :size="22" /><span>{{ t('noModelAssigned') }}</span></div><div v-for="layer in group.layers" :key="layer.id" :class="['model-layer-card',{active:layer.id===selectedLayerId}]" @click="selectLayer(layer)"><div class="layer-title"><button :aria-label="`${t('toggleVisibility')} ${layer.name}`" @click.stop="toggleVisibility(layer.id)"><component :is="layer.visible?Eye:EyeOff" :size="15" /></button><span><input v-model="layer.name" :aria-label="t('fieldName')" @click.stop /><small>{{ assets[layer.assetId]?.sourceFormat?.toUpperCase() }}</small></span><ChevronRight :size="15" /></div><label class="switch-row"><span><Eye :size="15" />{{ t('visible') }}</span><input :checked="layer.visible" type="checkbox" @change="toggleVisibility(layer.id)" /></label><label class="model-file">{{ t('modelFile') }}<input readonly :value="assets[layer.assetId]?.name || layer.name" /></label><label class="switch-row"><span>{{ t('flipV') }}</span><input v-model="layer.flipTextureV" type="checkbox" /></label><div class="layer-actions"><button @click.stop="addModel(group.key,layer)">{{ t('replaceModel') }}</button><button :title="t('delete')" @click.stop="removeLayer(layer)"><Trash2 :size="13" /></button></div><button class="binding-row" @click.stop="selectedLayerId=layer.id;treeTab='materials'"><span>{{ t('materialBindings') }}</span><span>{{ assets[layer.assetId]?.materials?.length || 0 }} <ChevronRight :size="14" /></span></button><div v-if="assets[layer.assetId]?.warnings.length" class="inline-warning"><AlertTriangle :size="14" />{{ assets[layer.assetId]?.warnings[0] }}</div></div></section></template>
          <template v-else-if="tab==='placement'"><label v-if="selectedPartId" class="switch-row"><span>{{ t('overridePlacement') }}</span><input type="checkbox" :checked="!!placementRule" @change="setPartOverride(($event.target as HTMLInputElement).checked)" /></label><p v-if="selectedPartId&&!placementRule" class="inspector-hint">{{ t('inheritsPlacement') }}</p><section v-if="placementRule" class="inspector-section"><h2>{{ selectedPartId ? t('partPlacement') : t('carriagePlacement') }}</h2><p class="inspector-hint">{{ selectedPartId ? t('partPlacementHint') : t('carriagePlacementHint') }}</p><label>{{ t('preset') }}<select v-model="placementRule.preset"><option value="all">{{ t('allCars') }}</option><option value="first">{{ t('firstCar') }}</option><option value="last">{{ t('lastCar') }}</option><option value="odd">{{ t('oddCars') }}</option><option value="even">{{ t('evenCars') }}</option><option value="every">{{ t('everyCars') }}</option><option value="custom">{{ t('custom') }}</option></select></label><div v-if="placementRule.preset==='every'" class="field-grid"><label>{{ t('interval') }}<input v-model.number="placementRule.every" type="number" min="1" /></label><label>{{ t('offset') }}<input v-model.number="placementRule.offset" type="number" /></label></div><template v-if="placementRule.preset==='custom'"><p class="inspector-hint">{{ t('advancedPlacementHint') }}</p><label>{{ t('whitelist') }}<input v-model="placementRule.whitelist" placeholder="1,-1,%2" /></label><label>{{ t('blacklist') }}<input v-model="placementRule.blacklist" placeholder="2,%4" /></label></template></section></template>
          <template v-else-if="tab==='mtr3'"><section class="inspector-section"><h2>{{ t('mtr3Compatibility') }}</h2><p class="inspector-hint">{{ t('mtr3Hint') }}</p><label>{{ t('baseTrainType') }}<input data-field="mtr3BaseTrainType" v-model="train.mtr3BaseTrainType" placeholder="sp1900" /></label><div class="compat-note"><AlertTriangle :size="17" /><span>{{ t('mtr3LengthHint') }}</span></div></section></template>
        </div>
      </aside>
    </div>
  </fieldset>
  <div v-else class="editor-loading">{{ t('loading') }}</div>
</template>

<style scoped>
.train-editor{--e-bg:var(--studio-bg,#141517);--e-surface:var(--studio-surface,#1b1c1f);--e-raised:var(--studio-raised,#232428);--e-border:var(--studio-border,#303136);--e-text:var(--studio-text,#e8e8ec);--e-muted:var(--studio-muted,#a4a5ad);--e-quiet:var(--studio-quiet,#73757e);--e-hover:var(--studio-hover,#28292e);--e-selected:var(--studio-selected,#33343a);position:relative;display:flex;flex-direction:column;min-width:0;min-height:0;height:100%;margin:0;padding:0;border:0;overflow:hidden;background:var(--e-bg);color:var(--e-text);font:12px var(--studio-font,'Segoe UI Variable','Segoe UI',sans-serif)}
.train-editor button,.train-editor input,.train-editor select,.train-editor textarea{font-family:inherit;box-sizing:border-box}
.train-editor button{cursor:pointer;transition:background .12s,color .12s}.train-editor button:disabled{opacity:.32;cursor:default}
.train-editor button:focus-visible,.train-editor input:focus-visible,.train-editor select:focus-visible,.train-editor textarea:focus-visible{outline:1px solid #b8bac3;outline-offset:1px}.train-editor:disabled .editor-body{opacity:.65}
.editor-toolbar{height:40px;flex:none;display:flex;align-items:center;gap:10px;padding:0 12px;border-bottom:1px solid var(--e-border);background:var(--e-surface)}
.tool-group{display:flex;align-items:center;gap:2px;padding-right:10px;border-right:1px solid var(--e-border);flex:none}
.tool-group button{height:28px;display:flex;align-items:center;gap:6px;padding:0 8px;border:0;border-radius:4px;background:transparent;color:var(--e-muted);font-size:12px;white-space:nowrap}
.tool-group button:hover:not(:disabled),.tool-group button.active{background:var(--e-hover);color:var(--e-text)}.tool-group svg,.editor-export svg{width:15px;height:15px;stroke-width:1.65}.tool-spacer{flex:1;min-width:0}
.camera-picker{height:28px;display:flex;align-items:center;gap:6px;padding:0 6px;color:var(--e-muted)}
.camera-picker select{min-width:0;max-width:116px;border:0;outline:0;background:var(--e-surface);color:var(--e-text);font-size:12px;padding:3px 2px}
.editor-export{height:28px;display:flex;align-items:center;gap:7px;flex:none;padding:0 11px;border:1px solid #d8d9df;border-radius:4px;background:#e4e5e9;color:#202126;font-size:12px;font-weight:600;white-space:nowrap}.editor-export:hover{background:#f4f4f6}
.editor-body{flex:1;display:grid;grid-template-columns:220px minmax(280px,1fr) 310px;min-height:0;padding-bottom:25px}
.carriage-panel{display:flex;flex-direction:column;min-height:0;padding:0 8px 8px;border-right:1px solid var(--e-border);background:var(--e-surface)}
.train-identity{height:64px;flex:none;display:flex;align-items:center;gap:9px;padding:10px 3px;border-bottom:1px solid var(--e-border);margin-bottom:12px}
.train-thumbnail{width:54px;height:40px;flex:none;overflow:hidden;border-radius:4px;background:var(--e-raised)}.train-thumbnail img,.carriage-preview img{width:100%;height:100%;object-fit:contain}
.train-identity>div:last-child{min-width:0;flex:1}.train-identity strong,.train-identity span{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.train-identity strong{font-size:12px;font-weight:600;line-height:1.4}.train-identity span{margin-top:4px;color:var(--e-quiet);font-size:10px}
.carriage-search,.tree-search{height:30px;flex:none;display:flex;align-items:center;gap:7px;padding:0 8px;border:1px solid var(--e-border);border-radius:4px;background:var(--e-bg);color:var(--e-quiet)}
.carriage-search input,.tree-search input{width:100%;min-width:0;padding:0;border:0;outline:0;background:transparent;color:var(--e-text);font-size:12px}.carriage-search input::placeholder,.tree-search input::placeholder{color:var(--e-quiet)}
.panel-heading{height:42px;flex:none;display:flex;align-items:center;justify-content:space-between;padding:0 3px}.panel-heading strong{font-size:12px;font-weight:600;color:var(--e-muted)}
.panel-heading button{display:flex;align-items:center;gap:4px;padding:4px 5px;border:0;border-radius:4px;background:transparent;color:var(--e-muted);font-size:10px}.panel-heading button:hover{background:var(--e-hover);color:var(--e-text)}
.carriage-cards{display:flex;flex-direction:column;gap:4px;flex:1;min-height:0;overflow:auto}
.carriage-card{width:100%;height:72px;min-height:72px;display:flex;align-items:center;gap:9px;padding:8px;border:1px solid transparent;border-radius:5px;background:transparent;color:var(--e-text);text-align:left}.carriage-card:hover{background:var(--e-hover)}
.carriage-card.active{background:var(--e-selected);border-color:#44454d;box-shadow:inset 2px 0 0 #c0c2cb}.carriage-preview{width:72px;height:46px;flex:none;overflow:hidden;border-radius:3px;background:var(--e-bg)}
.carriage-copy{flex:1;min-width:0}.carriage-copy strong,.carriage-copy small{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.carriage-copy strong{font-size:12px;font-weight:500;line-height:1.5}.carriage-copy small{margin-top:4px;font-size:9px;color:var(--e-quiet)}.carriage-card.active .carriage-copy small{color:var(--e-muted)}.carriage-number{align-self:flex-start;margin-top:2px;color:var(--e-quiet);font-size:9px}
.carriage-actions{display:grid;grid-template-columns:28px 28px 1fr 1fr;gap:3px;flex:none;margin-top:8px;padding-top:8px;border-top:1px solid var(--e-border)}
.carriage-actions button{height:29px;display:flex;align-items:center;justify-content:center;gap:4px;padding:0 3px;border:0;border-radius:4px;background:transparent;color:var(--e-muted);font-size:9px}.carriage-actions svg{width:14px;height:14px;flex:none}.carriage-actions button:hover:not(:disabled){background:var(--e-hover);color:var(--e-text)}
.editor-center{min-width:0;min-height:0;display:grid;grid-template-rows:minmax(200px,1.86fr) minmax(155px,1fr);background:var(--e-bg)}.editor-center.with-consist{grid-template-rows:minmax(190px,1.86fr) 82px minmax(145px,1fr)}
.viewport-panel{position:relative;min-width:0;min-height:0;border-bottom:1px solid var(--e-border)}
.viewport-badge{position:absolute;top:10px;left:12px;display:flex;align-items:center;gap:6px;padding:5px 7px;border-radius:3px;background:#1b1c1fcc;color:#bfc0c8;font-size:10px;pointer-events:none}
.model-warning{position:absolute;left:12px;right:12px;bottom:10px;display:flex;align-items:flex-start;gap:7px;padding:7px 9px;border:1px solid #665a42;border-radius:4px;background:#302b23ef;color:#cfba91;font-size:10px;line-height:1.5}.model-warning svg,.inline-warning svg,.compat-note svg{flex:none;margin-top:1px}
.environment-caption{position:absolute;right:12px;bottom:10px;padding:5px 8px;border-radius:3px;background:#1b1c1fcc;color:#d0d1d6;font-size:10px}.environment-caption.with-warning{bottom:65px}
.hierarchy-panel{display:flex;flex-direction:column;min-height:0;background:var(--e-surface)}.hierarchy-tabs{height:35px;flex:none;display:flex;align-items:stretch;padding:0 10px;border-bottom:1px solid var(--e-border)}
.hierarchy-tabs button{border:0;border-bottom:1px solid transparent;background:transparent;color:var(--e-quiet);padding:0 11px;font-size:12px}.hierarchy-tabs button.active{color:var(--e-text);border-bottom-color:#c2c4cb}.hierarchy-tabs button:hover{color:var(--e-text)}.hierarchy-tabs button:first-child{order:2;margin-left:auto;font-size:10px}.tree-search{height:28px;margin:8px 10px 6px}
.materials-list{min-height:0;overflow:auto;padding:0 10px 8px}.material-row{display:flex;align-items:center;gap:9px;padding:8px 3px;border-bottom:1px solid var(--e-border);color:var(--e-text)}.material-row>span:nth-child(2){flex:1;min-width:0}.material-row strong,.material-row small{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.material-row strong{font-size:12px;font-weight:500}.material-row small{font-size:10px;color:var(--e-quiet);margin-top:4px}.material-swatch{width:24px;height:24px;flex:none;border:1px solid var(--e-border);border-radius:3px}
.material-row button,.layer-actions button{display:flex;align-items:center;justify-content:center;gap:4px;flex:none;min-height:27px;padding:0 7px;border:1px solid var(--e-border);border-radius:4px;background:var(--e-raised);color:var(--e-muted);font-size:10px}.material-row button:hover,.layer-actions button:hover{background:var(--e-hover);color:var(--e-text)}.tree-empty{display:grid;place-items:center;min-height:90px;color:var(--e-quiet);font-size:12px}
.consist-panel{min-width:0;overflow:hidden;padding:7px 10px;border-bottom:1px solid var(--e-border);background:var(--e-surface)}.consist-heading{display:flex;align-items:center;gap:4px;font-size:10px}.consist-heading select{max-width:128px;border:0;background:var(--e-surface);color:var(--e-text);font:inherit;padding:3px}
.consist-heading button{height:25px;display:flex;align-items:center;gap:4px;padding:0 5px;border:0;border-radius:3px;background:transparent;color:var(--e-muted);font-size:10px;white-space:nowrap}.consist-heading button:hover,.consist-heading button.active{background:var(--e-hover);color:var(--e-text)}.consist-items{display:flex;align-items:center;gap:5px;overflow-x:auto;margin-top:5px}.consist-items button{display:flex;align-items:center;gap:8px;white-space:nowrap;padding:6px 8px;border:1px solid var(--e-border);border-radius:3px;background:var(--e-raised);color:var(--e-muted);font-size:10px}.consist-items button.active{border-color:#777983;background:var(--e-selected);color:var(--e-text)}.consist-items button span,.consist-items small{color:var(--e-quiet)}
.editor-status{position:absolute;left:0;right:0;bottom:0;height:25px;display:flex;align-items:center;gap:9px;padding:0 12px;border-top:1px solid var(--e-border);background:var(--e-surface);color:var(--e-quiet);font-size:10px}.editor-status i{width:2px;height:2px;border-radius:50%;background:var(--e-quiet)}.status-back{display:flex;align-items:center;gap:5px;margin-left:auto;padding:2px 6px;border:0;border-radius:3px;background:transparent;color:var(--e-muted);font-size:10px}.status-back:hover{background:var(--e-hover)}.status-right{display:flex;align-items:center;gap:5px;margin-left:8px}
.inspector{display:flex;flex-direction:column;min-height:0;overflow:hidden;border-left:1px solid var(--e-border);background:var(--e-surface)}.inspector-tabs{height:38px;flex:none;display:grid;grid-template-columns:repeat(4,1fr);padding:0 6px;border-bottom:1px solid var(--e-border)}
.inspector-tabs button{border:0;border-bottom:1px solid transparent;background:transparent;color:var(--e-quiet);font-size:12px}.inspector-tabs button.active{color:var(--e-text);border-bottom-color:#c2c4cb}.inspector-tabs button:hover{color:var(--e-text)}.inspector-content{min-height:0;overflow:auto;padding:0 13px 14px}
.inspector-section,.model-card{padding:16px 0;margin:0;border:0;border-bottom:1px solid var(--e-border);background:none}.inspector-section:last-child,.model-card:last-child{border-bottom:0}.inspector h2{margin:0 0 14px;font:600 12px var(--studio-font,'Segoe UI Variable','Segoe UI',sans-serif);color:var(--e-text)}
.inspector label{display:grid;grid-template-columns:95px minmax(0,1fr);align-items:center;gap:7px;margin:0 0 9px;color:var(--e-muted);font-size:12px;line-height:1.4}
.inspector input,.inspector textarea,.inspector select{width:100%;min-width:0;min-height:32px;padding:6px 8px;border:1px solid var(--e-border);border-radius:4px;outline:0;background:var(--e-bg);color:var(--e-text);font-size:12px;line-height:1.4}.inspector input:hover,.inspector select:hover{border-color:#45464e}.inspector input[readonly]{color:var(--e-muted)}.inspector .color-field{grid-template-columns:95px 32px minmax(0,1fr)}.inspector input[type=color]{width:32px;height:28px;min-height:28px;padding:3px;cursor:pointer}.color-value{font-size:10px;color:var(--e-quiet);font-variant-numeric:tabular-nums}.inspector textarea{min-height:72px;resize:vertical}.inspector label:has(textarea){align-items:start}
.field-grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:11px 12px;margin:14px 0}.inspector .field-grid label{display:flex;flex-direction:column;align-items:stretch;gap:6px;margin:0;font-size:10px;color:var(--e-muted)}
.model-card-head{display:flex;align-items:center;justify-content:space-between;gap:8px;margin-bottom:12px}.model-card-head h2{margin:0}.model-card-head button{display:flex;align-items:center;gap:5px;padding:4px 5px;border:0;border-radius:3px;background:transparent;color:var(--e-muted);font-size:10px}.model-card-head button:hover{background:var(--e-hover);color:var(--e-text)}
.empty-slot{height:58px;display:flex;align-items:center;justify-content:center;gap:8px;border:1px dashed var(--e-border);border-radius:4px;color:var(--e-quiet);font-size:10px}.model-layer-card{padding:10px 0 0;margin-top:10px;border-top:1px solid var(--e-border)}.model-layer-card:first-of-type{margin-top:0}
.layer-title{display:flex;align-items:center;gap:7px;margin-bottom:10px;color:var(--e-quiet)}.layer-title>button{display:grid;place-items:center;flex:none;width:24px;height:28px;padding:0;border:0;border-radius:3px;background:transparent;color:var(--e-muted)}.layer-title>button:hover{background:var(--e-hover)}.layer-title>span{flex:1;min-width:0}.layer-title input{height:28px;min-height:28px;padding:3px 5px;border-color:transparent;background:transparent;font-size:12px;font-weight:500}.layer-title input:hover,.layer-title input:focus{border-color:var(--e-border);background:var(--e-bg)}.layer-title small{display:block;padding-left:5px;margin-top:2px;font-size:9px;color:var(--e-quiet)}.model-layer-card.active .layer-title{color:var(--e-text)}.model-layer-card.active .layer-title>span{box-shadow:inset 2px 0 0 #a8aab3}
.inspector .switch-row{min-height:34px;display:flex;align-items:center;justify-content:space-between;gap:10px;margin:0;border:0}.switch-row span{display:flex;align-items:center;gap:7px}.inspector .switch-row input[type=checkbox]{appearance:none;position:relative;flex:none;width:29px;min-height:16px;height:16px;padding:0;border:1px solid #4b4d55;border-radius:9px;background:#303138;cursor:pointer}.switch-row input[type=checkbox]:before{content:'';position:absolute;left:2px;top:2px;width:10px;height:10px;border-radius:50%;background:#858791;transition:transform .12s}.inspector .switch-row input[type=checkbox]:checked{border-color:#c1c3cb;background:#bfc1ca}.switch-row input[type=checkbox]:checked:before{transform:translateX(13px);background:#303138}
.inspector .model-file{margin-top:8px;font-size:10px}.layer-actions{display:flex;justify-content:space-between;gap:7px;padding:6px 0 10px}.binding-row{width:100%;min-height:33px;display:flex;align-items:center;justify-content:space-between;gap:8px;padding:0;border:0;border-top:1px solid var(--e-border);background:none;color:var(--e-muted);font-size:10px;text-align:left}.binding-row:hover{color:var(--e-text)}.binding-row span:last-child{display:flex;align-items:center;gap:3px;color:var(--e-quiet)}
.inline-warning,.compat-note{display:flex;align-items:flex-start;gap:7px;margin-top:8px;padding:8px;border-radius:3px;background:#302b23;color:#c7b48c;font-size:10px;line-height:1.5}.inspector-hint{margin:0 0 12px;color:var(--e-quiet);font-size:12px;line-height:1.6}.editor-loading{height:400px;display:grid;place-items:center;color:var(--studio-muted,#a4a5ad);font:12px var(--studio-font,'Segoe UI Variable','Segoe UI',sans-serif)}
@media(min-width:1600px){.editor-body{grid-template-columns:220px minmax(280px,1fr) 320px}}
@media(max-width:1180px){.editor-toolbar{gap:7px}.tool-group{padding-right:7px}.tool-group button span{display:none}.editor-body{grid-template-columns:210px minmax(260px,1fr) 300px}.carriage-actions button span{display:none}}
@media(max-width:950px){.editor-body{grid-template-columns:190px minmax(250px,1fr) 280px}.editor-toolbar{padding:0 8px;gap:5px}.tool-group button{padding:0 5px}.camera-picker{max-width:112px}.camera-picker select{max-width:84px}.carriage-card{gap:7px;padding:7px 5px}.carriage-preview{width:64px}.inspector-content{padding:0 10px 12px}.inspector label{grid-template-columns:85px minmax(0,1fr)}.inspector .color-field{grid-template-columns:85px 32px minmax(0,1fr)}.consist-heading button{font-size:9px}.consist-heading select{max-width:108px}}
</style>
