<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { AlertTriangle, ArrowDown, ArrowLeft, ArrowUp, Box, BoxSelect, ChevronDown, ChevronRight, Copy, Eye, EyeOff, FileBox, Grid3X3, Maximize, Plus, RotateCw, Search, Trash2, Upload } from '@lucide/vue'
import ModelViewport, { type PreviewLayer } from './ModelViewport.vue'
import { t } from '../i18n'
import { analyzeModelImport, chooseModelDependency, chooseModelFile, chooseTextureFile, importTextureFile, storeImageBytes, getImageAsset, getModelAsset, getTrain, importModel, updateTrain, type AssetDefinition, type CarPlacementRule, type CarriageDefinition, type ContentEntry, type ModelLayer, type TrainDefinition, type ValidationIssue } from '../lib/projects'
import { arrangeConsist, matchesPlacement } from '../lib/train-preview'
import { SaveQueue } from '../lib/save-queue'

const props = defineProps<{ projectPath: string; entry: ContentEntry }>()
const emit = defineEmits<{ back: []; changed: [entry: ContentEntry]; status: [value: 'saving' | 'saved' | 'failed']; error: [message: string]; ready: [] }>()
type Tab = 'general' | 'models' | 'placement' | 'mtr3'
const train = ref<TrainDefinition>()
const selectedCarriageId = ref('')
const selectedLayerId = ref('')
const selectedPartId = ref('')
const tab = ref<Tab>('general')
const assets = ref<Record<string, AssetDefinition>>({})
const importing = ref(false)
const loading = ref(true)
const carriageQuery = ref('')
const partQuery = ref('')
const treeTab = ref<'model' | 'materials'>('model')
const showGrid = ref(true)
const wireframe = ref(false)
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
    key:`${instance.index}:${layer.id}`,assetId:layer.assetId,layerId:layer.id,carriageId:car.id,visible:layer.visible,z:instance.z,reversed:instance.reversed,bogieOffset:group.offset,flipV:layer.flipTextureV,bindings:layer.materialBindings,
    hiddenParts:simulateRules.value ? (assets.value[layer.assetId]?.parts||[]).filter(part=>!matchesPlacement(layer.partRules[part.id]||car.placement,position,count)).map(part=>part.id) : [],
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
    await loadAssets();await nextTick();emit('status','saved');emit('ready')
  } catch(cause){emit('error',message(cause));emit('status','failed')}
  finally{hydrating=false;loading.value=false}
}
async function loadAssets() {
  const all=train.value?.carriages.flatMap(car=>[...car.bodyModels,...car.bogie1Models,...car.bogie2Models])||[]
  await Promise.all([...new Set(all.map(layer=>layer.assetId))].filter(id=>!assets.value[id]).map(async id=>{try{assets.value[id]=await getModelAsset(id)}catch(cause){emit('error',message(cause))}}))
  await Promise.all((train.value?.carriages||[]).filter(car=>car.thumbnailHash&&!thumbnails.value[car.id]).map(async car=>{try{const bytes=await getImageAsset(car.thumbnailHash!);thumbnails.value[car.id]=URL.createObjectURL(new Blob([bytes],{type:'image/png'}))}catch(cause){emit('error',message(cause))}}))
}
watch(train,()=>{if(!hydrating)scheduleSave()},{deep:true,flush:'sync'})
watch(selectedCarriageId,()=>{selectedLayerId.value='';selectedPartId.value=''},{flush:'sync'})
watch(selectedLayerId,()=>{selectedPartId.value=''},{flush:'sync'})
onMounted(load)
onBeforeUnmount(()=>{alive=false;if(timer)window.clearTimeout(timer);Object.values(thumbnails.value).forEach(url=>URL.revokeObjectURL(url))})
function scheduleSave(){queue.markDirty();emit('status','saving');if(timer)window.clearTimeout(timer);timer=window.setTimeout(()=>void flush().catch(()=>{}),800)}
async function flush(){ if(operationPromise)await operationPromise; await flushEdits() }
async function flushEdits(){
  if(timer)window.clearTimeout(timer);timer=undefined
  try{await Promise.all([...thumbnailJobs]);await queue.flush();emit('status','saved')}
  catch(cause){emit('status','failed');emit('error',message(cause));throw cause}
}
async function immediate(action:()=>void){action();await flush()}
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
function selectLayer(layer:ModelLayer){selectedLayerId.value=layer.id;tab.value='models'}
function selectPart(partId:string,layerId=selectedLayerId.value,carriageId=selectedCarriageId.value){selectedCarriageId.value=carriageId;selectedLayerId.value=layerId;selectedPartId.value=partId}
function partsFor(layer:ModelLayer){return(assets.value[layer.assetId]?.parts||[]).filter(part=>part.name.toLowerCase().includes(partQuery.value.toLowerCase()))}
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
    if(replace){const index=group.findIndex(item=>item.id===replace.id);added.id=replace.id;added.name=replace.name;added.visible=replace.visible;added.flipTextureV=replace.flipTextureV;const materialIds=new Set(result.asset.materials.map(item=>item.id));const partIds=new Set(result.asset.parts.map(item=>item.id));added.materialBindings=replace.materialBindings.filter(item=>materialIds.has(item.materialId));added.partRules=Object.fromEntries(Object.entries(replace.partRules).filter(([id])=>partIds.has(id)));if(index>=0){group.pop();group.splice(index,1,added)}}
    selectedLayerId.value=added.id;await loadAssets();await flushEdits();emit('changed',{...props.entry,updatedAt:Date.now()})
  }catch(cause){emit('error',message(cause));emit('status','failed')}finally{importing.value=false;hydrating=false}
}
function replaceTexture(materialId:string){operationPromise=replaceTextureImpl(materialId).finally(()=>{operationPromise=undefined});return operationPromise}
async function replaceTextureImpl(materialId:string){
  if(!selectedLayer.value||importing.value)return
  const layer=selectedLayer.value;importing.value=true
  try{const path=await chooseTextureFile();if(!path)return;await flushEdits();const hash=await importTextureFile(path);const binding=layer.materialBindings.find(item=>item.materialId===materialId);if(binding)binding.textureAssetId=hash;else layer.materialBindings.push({materialId,textureAssetId:hash});await flushEdits()}
  catch(cause){emit('error',message(cause))}finally{importing.value=false}
}
function resetTexture(materialId:string){perform(()=>{if(selectedLayer.value)selectedLayer.value.materialBindings=selectedLayer.value.materialBindings.filter(item=>item.materialId!==materialId)})}
function addInstance(){perform(()=>{if(train.value&&carriage.value){train.value.previewConsist.push({carriageId:carriage.value.id,reversed:false});selectedInstance.value=train.value.previewConsist.length-1;viewMode.value='consist'}})}
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
      <div class="tool-group"><button class="back-button" @click="back"><ArrowLeft :size="16" />{{ t('trains') }}</button></div>
      <div class="tool-group"><button :class="{active:viewMode==='single'}" @click="viewMode='single'">{{ t('singleCarriage') }}</button><button :class="{active:viewMode==='consist'}" @click="viewMode='consist'">{{ t('previewConsist') }}</button><button :class="{active:simulateRules}" @click="simulateRules=!simulateRules">{{ t('simulateRules') }}</button></div>
      <div class="tool-spacer" />
      <div class="tool-group view-tools"><button :class="{active:showGrid}" @click="showGrid=!showGrid"><Grid3X3 :size="16" />{{ t('grid') }}</button><button :class="{active:wireframe}" @click="wireframe=!wireframe"><BoxSelect :size="16" />{{ t('wireframe') }}</button><button @click="viewport?.fitView()"><Maximize :size="16" />{{ t('fitView') }}</button></div>
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

      <main class="editor-center">
        <section class="viewport-panel"><ModelViewport ref="viewport" :assets="viewportAssets" :guides="viewportGuides" :selected-part="selectedPartId" :selected-layer="selectedLayerId" :show-grid="showGrid" :wireframe="wireframe" :thumbnail-carriage-id="viewMode==='single' && !simulateRules && !importing ? selectedCarriageId : undefined" @select="selection=>selectPart(selection.partId,selection.layerId,selection.carriageId)" @error="emit('error',$event)" @thumbnail="onThumbnail" /><div class="viewport-badge"><Box :size="14" />{{ carriage?.name }} · {{ carriage?.length }} × {{ carriage?.width }} m</div><div v-if="selectedAsset?.warnings.length" class="model-warning"><AlertTriangle :size="15" />{{ selectedAsset.warnings.join(' ') }}</div></section>
        <section class="consist-panel"><div class="consist-heading"><strong>{{ t('previewConsist') }}</strong><button @click="addInstance"><Plus :size="13" />{{ t('addSelectedCarriage') }}</button><span class="tool-spacer" /><button :disabled="!train.previewConsist[selectedInstance]" :title="t('reverseCarriage')" @click="reverseInstance"><RotateCw :size="14" /></button><button :disabled="selectedInstance===0" :title="t('moveUp')" @click="moveInstance(selectedInstance,selectedInstance-1)"><ArrowLeft :size="14" /></button><button :disabled="selectedInstance>=train.previewConsist.length-1" :title="t('moveDown')" @click="moveInstance(selectedInstance,selectedInstance+1)"><ChevronRight :size="14" /></button><button :disabled="!train.previewConsist.length" :title="t('delete')" @click="removeInstance"><Trash2 :size="14" /></button></div><div class="consist-items"><button v-for="(instance,index) in train.previewConsist" :key="index" draggable="true" :class="{active:selectedInstance===index}" @dragstart="dragIndex=index" @dragover.prevent @drop.prevent="dropInstance(index)" @click="selectedInstance=index;selectedCarriageId=instance.carriageId"><span>{{ index+1 }}</span>{{ train.carriages.find(item=>item.id===instance.carriageId)?.name }}<span>{{ instance.reversed ? '←' : '→' }}</span></button><small v-if="!train.previewConsist.length">{{ t('emptyConsist') }}</small></div></section>
        <section class="hierarchy-panel">
          <div class="hierarchy-tabs"><button :class="{active:treeTab==='model'}" @click="treeTab='model'">{{ t('modelTree') }}</button><button :class="{active:treeTab==='materials'}" @click="treeTab='materials'">{{ t('materials') }}</button></div>
          <label class="tree-search"><Search :size="14" /><input v-model="partQuery" :placeholder="t('searchParts')" /></label>
          <div v-if="treeTab==='model'" class="tree-table">
            <div class="tree-head"><span>{{ t('fieldName') }}</span><span>{{ t('type') }}</span><span>{{ t('visible') }}</span></div>
            <div class="tree-row root"><span><ChevronDown :size="14" /><Box :size="14" />{{ carriage?.name }}</span><small>{{ t('carriage') }}</small><Eye :size="14" /></div>
            <template v-for="layer in layers" :key="layer.id"><button :class="['tree-row','layer',{active:layer.id===selectedLayerId}]" @click="selectLayer(layer)"><span><ChevronDown :size="14" /><FileBox :size="14" />{{ layer.name }}</span><small>{{ t('modelLayer') }}</small><component :is="layer.visible?Eye:EyeOff" :size="14" @click.stop="layer.visible=!layer.visible" /></button><button v-for="part in partsFor(layer)" :key="part.id" :class="['tree-row','part',{active:part.id===selectedPartId && layer.id===selectedLayerId}]" @click="selectPart(part.id,layer.id)"><span><ChevronRight :size="13" /><Box :size="13" />{{ part.name }}</span><small>{{ part.triangleCount.toLocaleString() }} △</small><Eye :size="13" /></button></template>
          </div>
          <div v-else class="materials-list"><div v-if="!selectedAsset" class="tree-empty">{{ t('selectModelLayer') }}</div><template v-else><div v-for="material in selectedAsset.materials" :key="material.id" class="material-row"><span class="material-swatch" :style="{background:`rgba(${material.color.slice(0,3).map(value=>Math.round(value*255)).join(',')},${material.color[3]})`}" /><span><strong>{{ material.name }}</strong><small>{{ selectedLayer?.materialBindings.some(item=>item.materialId===material.id&&item.textureAssetId) ? t('customTexture') : material.texture || t('solidColor') }}</small></span><button @click="replaceTexture(material.id)">{{ t('replaceTexture') }}</button><button v-if="selectedLayer?.materialBindings.some(item=>item.materialId===material.id)" @click="resetTexture(material.id)">{{ t('resetTexture') }}</button></div></template></div>
        </section>
        <footer class="editor-status"><span>{{ layers.length }} {{ t('modelLayers') }}</span><i /> <span>{{ triangleCount.toLocaleString() }} {{ t('triangles') }}</span><i /><span>{{ t('unitsMetres') }}</span><i /><span>+Z {{ t('forward') }}</span><span class="status-right"><Grid3X3 :size="13" />{{ t('gridSize') }}</span></footer>
      </main>

      <aside class="inspector">
        <nav class="inspector-tabs"><button v-for="item in (['general','models','placement','mtr3'] as Tab[])" :key="item" :class="{active:tab===item}" @click="tab=item">{{ t(item === 'mtr3' ? 'mtr3Short' : item) }}</button></nav>
        <div class="inspector-content">
          <template v-if="tab==='general'"><section class="inspector-section"><h2>{{ t('train') }}</h2><label>{{ t('trainName') }}<input v-model="train.name" maxlength="80" /></label><label>{{ t('exportId') }}<input data-field="exportId" v-model="train.exportId" /></label><label>{{ t('description') }}<textarea v-model="train.description" rows="3" /></label><label>{{ t('tags') }}<input :value="train.tags.join(', ')" @change="updateTags" /></label></section><section v-if="carriage" class="inspector-section"><h2>{{ t('selectedCarriage') }}</h2><label>{{ t('fieldName') }}<input v-model="carriage.name" /></label><label>{{ t('exportId') }}<input v-model="carriage.exportId" /></label><div class="field-grid"><label>{{ t('length') }}<input data-field="length" min="0.1" v-model.number="carriage.length" type="number" step="0.1" /></label><label>{{ t('width') }}<input data-field="width" min="0.1" v-model.number="carriage.width" type="number" step="0.1" /></label><label>{{ t('bogie1') }}<input v-model.number="carriage.bogie1Position" type="number" step="0.1" /></label><label>{{ t('bogie2') }}<input v-model.number="carriage.bogie2Position" type="number" step="0.1" /></label><label>{{ t('frontCoupling') }}<input v-model.number="carriage.couplingPadding1" type="number" min="0" step="0.1" /></label><label>{{ t('rearCoupling') }}<input v-model.number="carriage.couplingPadding2" type="number" min="0" step="0.1" /></label></div><label class="switch-row"><span>{{ t('frontGangway') }}</span><input v-model="carriage.end1.gangway" type="checkbox" /></label><label class="switch-row"><span>{{ t('rearGangway') }}</span><input v-model="carriage.end2.gangway" type="checkbox" /></label><label class="switch-row"><span>{{ t('frontBarrier') }}</span><input v-model="carriage.end1.barrier" type="checkbox" /></label><label class="switch-row"><span>{{ t('rearBarrier') }}</span><input v-model="carriage.end2.barrier" type="checkbox" /></label></section></template>
          <template v-else-if="tab==='models' && carriage"><section v-for="group in slotGroups" :key="group.key" class="model-card"><div class="model-card-head"><h2>{{ group.label }}</h2><button :disabled="importing" @click="addModel(group.key)"><Upload :size="14" />{{ group.layers.length ? t('addLayer') : t('import') }}</button></div><div v-if="!group.layers.length" class="empty-slot"><FileBox :size="22" /><span>{{ t('noModelAssigned') }}</span></div><div v-for="layer in group.layers" :key="layer.id" :class="['model-layer-card',{active:layer.id===selectedLayerId}]" @click="selectLayer(layer)"><div class="layer-title"><button @click.stop="layer.visible=!layer.visible"><component :is="layer.visible?Eye:EyeOff" :size="15" /></button><span><input v-model="layer.name" :aria-label="t('fieldName')" @click.stop /><small>{{ assets[layer.assetId]?.sourceFormat?.toUpperCase() }}</small></span><ChevronRight :size="15" /></div><label class="switch-row"><span>{{ t('visible') }}</span><input v-model="layer.visible" type="checkbox" /></label><label class="switch-row"><span>{{ t('uvFlip') }}</span><input v-model="layer.flipTextureV" type="checkbox" /></label><div class="layer-actions"><button @click.stop="addModel(group.key,layer)">{{ t('replaceModel') }}</button><button :title="t('delete')" @click.stop="removeLayer(layer)"><Trash2 :size="13" /></button></div><button class="binding-row" @click.stop="selectedLayerId=layer.id;treeTab='materials'"><span>{{ t('materialBindings') }}</span><span>{{ assets[layer.assetId]?.materials?.length || 0 }} <ChevronRight :size="14" /></span></button><div v-if="assets[layer.assetId]?.warnings.length" class="inline-warning"><AlertTriangle :size="14" />{{ assets[layer.assetId]?.warnings[0] }}</div></div></section></template>
          <template v-else-if="tab==='placement'"><label v-if="selectedPartId" class="switch-row"><span>{{ t('overridePlacement') }}</span><input type="checkbox" :checked="!!placementRule" @change="setPartOverride(($event.target as HTMLInputElement).checked)" /></label><p v-if="selectedPartId&&!placementRule" class="inspector-hint">{{ t('inheritsPlacement') }}</p><section v-if="placementRule" class="inspector-section"><h2>{{ selectedPartId ? t('partPlacement') : t('carriagePlacement') }}</h2><p class="inspector-hint">{{ selectedPartId ? t('partPlacementHint') : t('carriagePlacementHint') }}</p><label>{{ t('preset') }}<select v-model="placementRule.preset"><option value="all">{{ t('allCars') }}</option><option value="first">{{ t('firstCar') }}</option><option value="last">{{ t('lastCar') }}</option><option value="odd">{{ t('oddCars') }}</option><option value="even">{{ t('evenCars') }}</option><option value="every">{{ t('everyCars') }}</option><option value="custom">{{ t('custom') }}</option></select></label><div v-if="placementRule.preset==='every'" class="field-grid"><label>{{ t('interval') }}<input v-model.number="placementRule.every" type="number" min="1" /></label><label>{{ t('offset') }}<input v-model.number="placementRule.offset" type="number" /></label></div><template v-if="placementRule.preset==='custom'"><p class="inspector-hint">{{ t('advancedPlacementHint') }}</p><label>{{ t('whitelist') }}<input v-model="placementRule.whitelist" placeholder="1,-1,%2" /></label><label>{{ t('blacklist') }}<input v-model="placementRule.blacklist" placeholder="2,%4" /></label></template></section></template>
          <template v-else-if="tab==='mtr3'"><section class="inspector-section"><h2>{{ t('mtr3Compatibility') }}</h2><p class="inspector-hint">{{ t('mtr3Hint') }}</p><label>{{ t('baseTrainType') }}<input data-field="mtr3BaseTrainType" v-model="train.mtr3BaseTrainType" placeholder="sp1900" /></label><div class="compat-note"><AlertTriangle :size="17" /><span>{{ t('mtr3LengthHint') }}</span></div></section></template>
        </div>
      </aside>
    </div>
  </fieldset>
  <div v-else class="editor-loading">{{ t('loading') }}</div>
</template>

<style scoped>
.train-editor{padding:0;margin:0;border:0;min-width:0;height:100%;min-height:0;display:flex;flex-direction:column;overflow:hidden;background:#171c21;color:#eef3f8}.editor-toolbar{height:52px;flex:none;display:flex;align-items:center;gap:10px;padding:0 10px;border-bottom:1px solid #39434d;background:#1b2127}.tool-group{display:flex;align-items:center;gap:4px;padding-right:10px;border-right:1px solid #3b454f}.tool-group:last-child{border:0;padding:0}.tool-group button{height:34px;display:flex;align-items:center;gap:7px;border:1px solid transparent;border-radius:6px;background:transparent;color:#cbd4de;padding:0 10px;font-size:11px}.tool-group button:hover:not(:disabled),.tool-group button.active{border-color:#6f8294;background:#303b45;color:#f5f8fb}.tool-group button:disabled{opacity:.35}.tool-group .back-button{border-color:#46525d}.tool-spacer{flex:1}.editor-body{flex:1;min-height:0;display:grid;grid-template-columns:255px minmax(380px,1fr) 326px}.carriage-panel{min-height:0;display:flex;flex-direction:column;border-right:1px solid #39434d;background:#1d2329;padding:10px}.train-identity{height:68px;display:flex;align-items:center;gap:10px;padding:8px;border-bottom:1px solid #39434d;margin:-10px -10px 10px}.train-thumbnail{width:60px;height:48px;display:grid;place-items:center;flex:none;border:1px solid #4b5864;border-radius:6px;background:linear-gradient(145deg,#43515e,#202830);color:#dce7f1}.train-identity>div:nth-child(2){min-width:0;flex:1}.train-identity strong,.train-identity span{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.train-identity strong{font-size:12px}.train-identity span{color:#8996a2;font-size:9px;margin-top:5px}.carriage-search,.tree-search{height:35px;display:flex;align-items:center;gap:8px;border:1px solid #46525e;border-radius:6px;background:#1a2025;color:#9ba8b4;padding:0 10px}.carriage-search input,.tree-search input{min-width:0;flex:1;border:0;background:transparent;color:#e8edf2;outline:0;font-size:10px}.panel-heading{display:flex;align-items:center;justify-content:space-between;margin:14px 3px 9px}.panel-heading strong{font-size:12px}.panel-heading button{display:flex;align-items:center;gap:5px;border:1px solid #4b5864;border-radius:5px;background:#293139;color:#dce4ec;padding:5px 8px;font-size:10px}.carriage-cards{min-height:0;overflow:auto;display:flex;flex-direction:column;gap:7px}.carriage-card{width:100%;height:72px;display:flex;align-items:center;gap:10px;border:1px solid #3c4650;border-radius:7px;background:#232a31;color:#dce3ea;padding:7px;text-align:left}.carriage-card:hover{border-color:#667584}.carriage-card.active{border-color:#9cc7ed;background:#303d49;box-shadow:inset 0 0 0 1px #75a6d0}.carriage-preview{width:76px;height:51px;display:flex;align-items:center;justify-content:center;flex:none;border-radius:5px;background:linear-gradient(#51606c,#29343d);overflow:hidden}.mini-car{width:66px;height:25px;display:flex;gap:5px;align-items:center;border:2px solid #c8d1d8;border-radius:3px;background:#707d87;box-shadow:0 6px 0 #232a30}.mini-car i{width:13px;height:12px;background:#202d38;border:1px solid #a6b1ba}.carriage-copy{min-width:0;flex:1}.carriage-copy strong,.carriage-copy small{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.carriage-copy strong{font-size:11px}.carriage-copy small{font-size:8px;color:#9aa6b1;margin-top:5px}.carriage-number{align-self:flex-start;color:#697784;font-size:9px}.carriage-actions{display:grid;grid-template-columns:35px 35px 1fr 1fr;gap:5px;margin-top:9px}.carriage-actions button{height:34px;display:flex;align-items:center;justify-content:center;gap:5px;border:1px solid #46525d;border-radius:5px;background:#252d34;color:#cbd4dd;font-size:9px}.carriage-actions button:disabled{opacity:.35}.editor-center{min-width:0;min-height:0;display:grid;grid-template-rows:minmax(220px,1fr) 88px 215px 28px;background:#151a1f}.viewport-panel{min-width:0;min-height:0;position:relative;border-bottom:1px solid #39434d}.viewport-badge{position:absolute;top:10px;left:10px;display:flex;align-items:center;gap:6px;border:1px solid #495661;border-radius:5px;background:#1b2228d9;color:#cbd6df;padding:6px 9px;font-size:9px}.model-warning{position:absolute;left:12px;right:12px;bottom:12px;display:flex;gap:8px;align-items:center;padding:8px 10px;border:1px solid #80683c;border-radius:6px;background:#3b3325e8;color:#f1d59c;font-size:9px}.hierarchy-panel{min-height:0;display:flex;flex-direction:column;background:#1b2127}.hierarchy-tabs{height:36px;display:flex;align-items:end;border-bottom:1px solid #39434d;padding-left:10px}.hierarchy-tabs button{height:36px;border:0;border-bottom:2px solid transparent;background:none;color:#99a6b2;padding:0 13px;font-size:10px}.hierarchy-tabs button.active{color:#dfefff;border-color:#9dc9f0}.tree-search{height:29px;margin:7px 9px}.tree-table,.materials-list{min-height:0;overflow:auto;padding:0 9px 7px}.tree-head,.tree-row{display:grid;grid-template-columns:minmax(180px,1fr) 110px 45px;align-items:center;min-height:24px;padding:0 8px;color:#b9c4ce;font-size:9px}.tree-head{color:#7f8c98;border-bottom:1px solid #303943}.tree-row{width:100%;border:0;border-radius:3px;background:none;text-align:left}.tree-row:hover,.tree-row.active{background:#35485a;color:#f2f7fb}.tree-row>span{display:flex;align-items:center;gap:6px;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.tree-row small{color:#84919d;font-size:8px}.tree-row>svg{justify-self:center}.tree-row.layer{padding-left:23px}.tree-row.part{padding-left:49px}.materials-list{padding-top:4px}.material-row{display:flex;align-items:center;gap:10px;border-bottom:1px solid #303943;padding:8px;color:#c7d1da}.material-row span{min-width:0}.material-row strong,.material-row small{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.material-row strong{font-size:10px}.material-row small{font-size:8px;color:#83909c;margin-top:3px}.tree-empty{display:grid;place-items:center;height:110px;color:#7f8c98;font-size:10px}.editor-status{display:flex;align-items:center;gap:9px;border-top:1px solid #39434d;background:#1a2025;color:#8e9aa6;padding:0 10px;font-size:8px}.editor-status i{width:2px;height:2px;border-radius:50%;background:#687580}.status-right{margin-left:auto;display:flex;align-items:center;gap:5px}.inspector{min-height:0;border-left:1px solid #39434d;background:#1d2329;overflow:hidden;display:flex;flex-direction:column}.inspector-tabs{height:48px;flex:none;display:grid;grid-template-columns:repeat(4,1fr);border-bottom:1px solid #39434d}.inspector-tabs button{border:0;border-bottom:2px solid transparent;background:transparent;color:#a3afba;font-size:10px}.inspector-tabs button.active{color:#dceeff;border-color:#9ecbf1;background:#222a31}.inspector-content{min-height:0;overflow:auto;padding:12px}.inspector-section,.model-card{border:1px solid #3c4852;border-radius:7px;background:#222a31;padding:12px;margin-bottom:10px}.inspector h2{font:600 13px Outfit,sans-serif;margin:0 0 12px}.inspector label{display:flex;flex-direction:column;gap:5px;color:#9daab6;font-size:9px;margin-bottom:10px}.inspector input,.inspector textarea,.inspector select{width:100%;border:1px solid #46535f;border-radius:5px;background:#171d22;color:#eef2f6;padding:7px 8px;outline:0;font-size:10px}.inspector textarea{resize:vertical}.field-grid{display:grid;grid-template-columns:1fr 1fr;gap:7px}.model-card-head{display:flex;align-items:center;justify-content:space-between;margin-bottom:9px}.model-card-head h2{margin:0}.model-card-head button{display:flex;align-items:center;gap:5px;border:1px solid #4b5965;border-radius:5px;background:#2e3841;color:#d8e1e9;padding:5px 7px;font-size:8px}.empty-slot{height:48px;display:flex;align-items:center;justify-content:center;gap:8px;color:#778590;font-size:9px;border:1px dashed #46515b;border-radius:5px}.model-layer-card{border:1px solid #3b4751;border-radius:6px;background:#1c2329;padding:8px;margin-top:7px}.model-layer-card.active{border-color:#79a8d1}.layer-title{display:flex;align-items:center;gap:7px;margin-bottom:8px}.layer-title>button{border:0;background:none;color:#bcc7d0;padding:0}.layer-title>span{min-width:0;flex:1}.layer-title strong,.layer-title small{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.layer-title strong{font-size:10px}.layer-title small{font-size:7px;color:#82909b;margin-top:2px}.switch-row{height:28px!important;display:flex!important;flex-direction:row!important;align-items:center;justify-content:space-between;margin:0!important;border-top:1px solid #303a43}.switch-row input{width:auto}.binding-row{width:100%;height:29px;display:flex;align-items:center;justify-content:space-between;border:0;border-top:1px solid #303a43;background:none;color:#aeb9c3;font-size:9px;padding:0}.binding-row span:last-child{display:flex;align-items:center}.inline-warning,.compat-note{display:flex;gap:7px;color:#e4c98f;background:#373025;border-radius:5px;padding:8px;font-size:8px;line-height:1.35;margin-top:6px}.inspector-hint{color:#8f9ca7;font-size:9px;line-height:1.5}.editor-loading{height:400px;display:grid;place-items:center;color:#99a5af}@media(max-width:1200px){.editor-body{grid-template-columns:210px minmax(330px,1fr) 280px}.edit-tools button span,.carriage-actions button span{display:none}}@media(max-width:950px){.editor-body{grid-template-columns:170px minmax(300px,1fr) 245px}.tool-group button{padding:0 7px}.view-tools button{font-size:0}.editor-center{grid-template-rows:minmax(220px,1fr) 88px 180px 28px}}
.train-thumbnail img{width:100%;height:100%;object-fit:contain}.carriage-preview img{width:100%;height:100%;object-fit:contain}.consist-panel{min-width:0;border-bottom:1px solid #39434d;padding:7px 10px;background:#1c2329}.consist-heading{display:flex;align-items:center;gap:5px;font-size:10px}.consist-heading strong{margin-right:10px}.consist-heading button,.material-row button,.layer-actions button{display:flex;align-items:center;gap:4px;border:1px solid #485866;border-radius:4px;background:#29343e;color:#d6e2eb;padding:4px 6px;font-size:9px}.consist-items{display:flex;gap:6px;overflow-x:auto;margin-top:6px}.consist-items button{display:flex;align-items:center;gap:8px;white-space:nowrap;border:1px solid #485866;border-radius:4px;background:#27333d;color:#cbd8e3;padding:6px 9px;font-size:10px}.consist-items button.active{border-color:#99c5e9;background:#364c5f}.consist-items button span,.consist-items small{color:#8fa1b0}.layer-actions{display:flex;justify-content:space-between;padding:4px 0 8px}.material-swatch{width:22px;height:22px;border:1px solid #596877;border-radius:4px;flex:none}.material-row>span:nth-child(2){flex:1}.material-row button{flex:none}.train-editor:disabled .editor-body{opacity:.7}.train-editor button:disabled{opacity:.35;cursor:default}</style>
