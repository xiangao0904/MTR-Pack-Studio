<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { AlertTriangle, ArrowDown, ArrowLeft, ArrowUp, Box, BoxSelect, ChevronDown, ChevronRight, Copy, Eye, EyeOff, FileBox, FolderOpen, Grid3X3, Maximize, MoreHorizontal, MousePointer2, Move, Plus, Redo2, RotateCw, Scaling, Search, Trash2, Undo2, Upload } from '@lucide/vue'
import ModelViewport from './ModelViewport.vue'
import { t } from '../i18n'
import { analyzeModelImport, chooseModelDependency, chooseModelFile, getModelAsset, getTrain, importModel, updateTrain, type AssetDefinition, type CarPlacementRule, type CarriageDefinition, type ContentEntry, type ModelLayer, type TrainDefinition } from '../lib/projects'

const props = defineProps<{ projectPath: string; entry: ContentEntry }>()
const emit = defineEmits<{ back: []; changed: [entry: ContentEntry]; status: [value: 'saving' | 'saved' | 'failed']; error: [message: string] }>()
type Tab = 'general' | 'models' | 'placement' | 'mtr3'
const train = ref<TrainDefinition>()
const selectedCarriageId = ref('')
const selectedLayerId = ref('')
const selectedPartId = ref('')
const tab = ref<Tab>('general')
const assets = ref<Record<string, AssetDefinition>>({})
const importing = ref(false)
const carriageQuery = ref('')
const partQuery = ref('')
const treeTab = ref<'model' | 'materials'>('model')
const showGrid = ref(true)
const wireframe = ref(false)
const viewport = ref<{ fitView: () => void }>()
let timer: number | undefined
let changeSequence = 0
let hydrating = true
let saving = false

const carriage = computed(() => train.value?.carriages.find(item => item.id === selectedCarriageId.value))
const layers = computed(() => carriage.value ? [...carriage.value.bodyModels, ...carriage.value.bogie1Models, ...carriage.value.bogie2Models] : [])
const selectedLayer = computed(() => layers.value.find(item => item.id === selectedLayerId.value))
const selectedAsset = computed(() => selectedLayer.value ? assets.value[selectedLayer.value.assetId] : undefined)
const viewportAssets = computed(() => layers.value.map(layer => ({ id: layer.assetId, visible: layer.visible })))
const filteredCarriages = computed(() => train.value?.carriages.filter(item => item.name.toLowerCase().includes(carriageQuery.value.toLowerCase())) || [])
const triangleCount = computed(() => [...new Set(layers.value.map(layer => layer.assetId))].reduce((sum, id) => sum + (assets.value[id]?.parts.reduce((partSum, part) => partSum + part.triangleCount, 0) || 0), 0))
const slotGroups = computed(() => carriage.value ? [
  { key: 'body' as const, label: t('body'), layers: carriage.value.bodyModels },
  { key: 'bogie1' as const, label: t('frontBogie'), layers: carriage.value.bogie1Models },
  { key: 'bogie2' as const, label: t('rearBogie'), layers: carriage.value.bogie2Models },
] : [])
const placementRule = computed<CarPlacementRule | undefined>(() => {
  if (!carriage.value) return undefined
  if (selectedPartId.value && selectedLayer.value) return selectedLayer.value.partRules[selectedPartId.value] ||= { preset: 'all', offset: 0, whitelist: '', blacklist: '' }
  return carriage.value.placement
})

async function load() {
  try {
    hydrating = true; train.value = await getTrain(props.projectPath, props.entry.id); selectedCarriageId.value = train.value.carriages[0]?.id || ''
    await nextTick(); await loadAssets(); emit('status', 'saved')
  } catch (cause) { emit('error', message(cause)); emit('status', 'failed') }
  finally { hydrating = false }
}
async function loadAssets() {
  const ids = new Set(layers.value.map(layer => layer.assetId))
  await Promise.all([...ids].filter(id => !assets.value[id]).map(async id => { try { assets.value[id] = await getModelAsset(id) } catch { /* surfaced when selecting the model */ } }))
}
watch(train, () => { if (!hydrating) scheduleSave() }, { deep: true })
watch(selectedCarriageId, () => { selectedLayerId.value = ''; selectedPartId.value = ''; void loadAssets() })
watch(selectedLayerId, () => { selectedPartId.value = ''; void loadAssets() })
onMounted(load)
onBeforeUnmount(() => { if (timer) window.clearTimeout(timer) })

function scheduleSave() { changeSequence += 1; if (timer) window.clearTimeout(timer); timer = window.setTimeout(() => void flush(), 800) }
async function flush() {
  if (!train.value || saving) { if (saving) scheduleSave(); return }
  if (timer) window.clearTimeout(timer); timer = undefined; saving = true; emit('status', 'saving')
  const sequence = changeSequence; const snapshot = JSON.parse(JSON.stringify(train.value)) as TrainDefinition
  try {
    const updated = await updateTrain(props.projectPath, snapshot, snapshot.revision)
    hydrating = true; if (train.value) train.value.revision = updated.revision; await nextTick(); hydrating = false
    emit('changed', { ...props.entry, name: updated.name, updatedAt: Date.now() }); emit('status', 'saved')
    if (sequence !== changeSequence) scheduleSave()
  } catch (cause) { emit('status', 'failed'); emit('error', message(cause)) }
  finally { saving = false }
}
defineExpose({ flush })

function addCarriage() {
  if (!train.value) return
  const number = train.value.carriages.length + 1; const item: CarriageDefinition = { id: crypto.randomUUID(), exportId: `carriage_${number}`, name: `Carriage ${number}`, length: 20, width: 3, bogie1Position: 7, bogie2Position: -7, couplingPadding1: 0, couplingPadding2: 0, end1: { gangway: false, barrier: false }, end2: { gangway: false, barrier: false }, placement: { preset: 'all', offset: 0, whitelist: '', blacklist: '' }, bodyModels: [], bogie1Models: [], bogie2Models: [] }
  train.value.carriages.push(item); train.value.previewConsist.push({ carriageId: item.id, reversed: false }); selectedCarriageId.value = item.id
}
function duplicateCarriage() { if (!train.value || !carriage.value) return; const copy = JSON.parse(JSON.stringify(carriage.value)) as CarriageDefinition; copy.id = crypto.randomUUID(); copy.name += ' Copy'; copy.exportId += '_copy'; for (const layer of [...copy.bodyModels, ...copy.bogie1Models, ...copy.bogie2Models]) layer.id = crypto.randomUUID(); train.value.carriages.push(copy); selectedCarriageId.value = copy.id }
function removeCarriage() { if (!train.value || !carriage.value || train.value.carriages.length === 1) return; const id = carriage.value.id; train.value.carriages = train.value.carriages.filter(item => item.id !== id); train.value.previewConsist = train.value.previewConsist.filter(item => item.carriageId !== id); selectedCarriageId.value = train.value.carriages[0].id }
function moveCarriage(direction: -1 | 1) { if (!train.value || !carriage.value) return; const index = train.value.carriages.indexOf(carriage.value); const next = index + direction; if (next < 0 || next >= train.value.carriages.length) return; [train.value.carriages[index], train.value.carriages[next]] = [train.value.carriages[next], train.value.carriages[index]] }
function selectLayer(layer: ModelLayer) { selectedLayerId.value = layer.id; tab.value = 'models' }
function selectPart(partId: string) { selectedPartId.value = partId }
function partsFor(layer: ModelLayer) { const query = partQuery.value.toLowerCase(); return (assets.value[layer.assetId]?.parts || []).filter(part => part.name.toLowerCase().includes(query)) }
function updateTags(event: Event) { if (train.value) train.value.tags = (event.target as HTMLInputElement).value.split(',').map(value => value.trim()).filter(Boolean) }
async function addModel(slot: 'body' | 'bogie1' | 'bogie2') {
  if (!train.value || !carriage.value) return
  try {
    const path = await chooseModelFile(); if (!path) return; importing.value = true
    const dependencyOverrides: Record<string,string> = {}; let analysis = await analyzeModelImport(path, dependencyOverrides)
    while (analysis.missingDependencies.length) { for (const missing of analysis.missingDependencies) { const name = missing.split(/[\\/]/).pop() || missing; const resolved = await chooseModelDependency(name); if (!resolved) throw new Error(`${t('missingDependencies')}: ${name}`); dependencyOverrides[name] = resolved } analysis = await analyzeModelImport(path, dependencyOverrides) }
    const result = await importModel(train.value.id, carriage.value.id, slot, path, dependencyOverrides); hydrating = true; train.value = result.train; assets.value[result.asset.id] = result.asset; selectedLayerId.value = [...carriage.value.bodyModels, ...carriage.value.bogie1Models, ...carriage.value.bogie2Models].at(-1)?.id || ''; await nextTick(); hydrating = false; emit('status', 'saved')
  } catch (cause) { emit('error', message(cause)); emit('status', 'failed') } finally { importing.value = false }
}
function message(cause: unknown) { return cause instanceof Error ? cause.message : String(cause) }
</script>

<template>
  <div v-if="train" class="train-editor">
    <header class="editor-toolbar">
      <div class="tool-group"><button class="back-button" @click="emit('back')"><ArrowLeft :size="16" />{{ t('trains') }}</button></div>
      <div class="tool-group"><button disabled><Undo2 :size="16" />{{ t('undo') }}</button><button disabled><Redo2 :size="16" />{{ t('redo') }}</button></div>
      <div class="tool-group edit-tools"><button class="active"><MousePointer2 :size="16" />{{ t('select') }}</button><button disabled><Move :size="16" />{{ t('move') }}</button><button disabled><RotateCw :size="16" />{{ t('rotate') }}</button><button disabled><Scaling :size="16" />{{ t('scale') }}</button></div>
      <div class="tool-spacer" />
      <div class="tool-group view-tools"><button :class="{active:showGrid}" @click="showGrid=!showGrid"><Grid3X3 :size="16" />{{ t('grid') }}</button><button :class="{active:wireframe}" @click="wireframe=!wireframe"><BoxSelect :size="16" />{{ t('wireframe') }}</button><button @click="viewport?.fitView()"><Maximize :size="16" />{{ t('fitView') }}</button></div>
    </header>
    <div class="editor-body">
      <aside class="carriage-panel">
        <div class="train-identity"><div class="train-thumbnail"><Box :size="27" /></div><div><strong>{{ train.name }}</strong><span>{{ train.exportId }}</span></div><MoreHorizontal :size="17" /></div>
        <label class="carriage-search"><Search :size="15" /><input v-model="carriageQuery" :placeholder="t('searchCarriages')" /></label>
        <div class="panel-heading"><strong>{{ t('carriages') }}</strong><button @click="addCarriage"><Plus :size="15" />{{ t('add') }}</button></div>
        <div class="carriage-cards">
          <button v-for="item in filteredCarriages" :key="item.id" :class="['carriage-card',{active:item.id===selectedCarriageId}]" @click="selectedCarriageId=item.id">
            <span class="carriage-preview"><span class="mini-car"><i /><i /><i /></span></span><span class="carriage-copy"><strong>{{ item.name }}</strong><small>{{ item.exportId }} · {{ item.length }} m</small></span><span class="carriage-number">{{ train.carriages.indexOf(item) + 1 }}</span>
          </button>
        </div>
        <div class="carriage-actions"><button :title="t('moveUp')" @click="moveCarriage(-1)"><ArrowUp :size="16" /></button><button :title="t('moveDown')" @click="moveCarriage(1)"><ArrowDown :size="16" /></button><button :title="t('duplicate')" @click="duplicateCarriage"><Copy :size="16" /><span>{{ t('duplicate') }}</span></button><button :title="t('delete')" :disabled="train.carriages.length===1" @click="removeCarriage"><Trash2 :size="16" /><span>{{ t('delete') }}</span></button></div>
      </aside>

      <main class="editor-center">
        <section class="viewport-panel"><ModelViewport ref="viewport" :assets="viewportAssets" :selected-part="selectedPartId" :show-grid="showGrid" :wireframe="wireframe" @select="selectPart" /><div class="viewport-badge"><Box :size="14" />{{ carriage?.name }}</div><div v-if="selectedAsset?.warnings.length" class="model-warning"><AlertTriangle :size="15" />{{ selectedAsset.warnings.join(' ') }}</div></section>
        <section class="hierarchy-panel">
          <div class="hierarchy-tabs"><button :class="{active:treeTab==='model'}" @click="treeTab='model'">{{ t('modelTree') }}</button><button :class="{active:treeTab==='materials'}" @click="treeTab='materials'">{{ t('materials') }}</button></div>
          <label class="tree-search"><Search :size="14" /><input v-model="partQuery" :placeholder="t('searchParts')" /></label>
          <div v-if="treeTab==='model'" class="tree-table">
            <div class="tree-head"><span>{{ t('fieldName') }}</span><span>{{ t('type') }}</span><span>{{ t('visible') }}</span></div>
            <div class="tree-row root"><span><ChevronDown :size="14" /><Box :size="14" />{{ carriage?.name }}</span><small>{{ t('carriage') }}</small><Eye :size="14" /></div>
            <template v-for="layer in layers" :key="layer.id"><button :class="['tree-row','layer',{active:layer.id===selectedLayerId}]" @click="selectLayer(layer)"><span><ChevronDown :size="14" /><FileBox :size="14" />{{ layer.name }}</span><small>{{ t('modelLayer') }}</small><component :is="layer.visible?Eye:EyeOff" :size="14" /></button><button v-for="part in partsFor(layer)" :key="part.id" :class="['tree-row','part',{active:part.id===selectedPartId}]" @click="selectedLayerId=layer.id;selectPart(part.id)"><span><ChevronRight :size="13" /><Box :size="13" />{{ part.name }}</span><small>{{ part.triangleCount.toLocaleString() }} △</small><Eye :size="13" /></button></template>
          </div>
          <div v-else class="materials-list"><div v-if="!selectedAsset" class="tree-empty">{{ t('selectModelLayer') }}</div><template v-else><div class="material-row"><FileBox :size="16" /><span><strong>{{ selectedAsset.name }}</strong><small>{{ selectedAsset.sourceFormat.toUpperCase() }} · {{ selectedAsset.dependencies.length }} {{ t('dependencies') }}</small></span></div><div v-for="dependency in selectedAsset.dependencies" :key="dependency.hash" class="material-row"><FolderOpen :size="15" /><span><strong>{{ dependency.name }}</strong><small>{{ dependency.mediaType }}</small></span></div></template></div>
        </section>
        <footer class="editor-status"><span>{{ layers.length }} {{ t('modelLayers') }}</span><i /> <span>{{ triangleCount.toLocaleString() }} {{ t('triangles') }}</span><i /><span>{{ t('unitsMetres') }}</span><i /><span>+Z {{ t('forward') }}</span><span class="status-right"><Grid3X3 :size="13" />{{ t('gridSize') }}</span></footer>
      </main>

      <aside class="inspector">
        <nav class="inspector-tabs"><button v-for="item in (['general','models','placement','mtr3'] as Tab[])" :key="item" :class="{active:tab===item}" @click="tab=item">{{ t(item === 'mtr3' ? 'mtr3Short' : item) }}</button></nav>
        <div class="inspector-content">
          <template v-if="tab==='general'"><section class="inspector-section"><h2>{{ t('train') }}</h2><label>{{ t('trainName') }}<input v-model="train.name" maxlength="80" /></label><label>{{ t('exportId') }}<input v-model="train.exportId" /></label><label>{{ t('description') }}<textarea v-model="train.description" rows="3" /></label><label>{{ t('tags') }}<input :value="train.tags.join(', ')" @change="updateTags" /></label></section><section v-if="carriage" class="inspector-section"><h2>{{ t('selectedCarriage') }}</h2><label>{{ t('fieldName') }}<input v-model="carriage.name" /></label><label>{{ t('exportId') }}<input v-model="carriage.exportId" /></label><div class="field-grid"><label>{{ t('length') }}<input v-model.number="carriage.length" type="number" step="0.1" /></label><label>{{ t('width') }}<input v-model.number="carriage.width" type="number" step="0.1" /></label><label>{{ t('bogie1') }}<input v-model.number="carriage.bogie1Position" type="number" step="0.1" /></label><label>{{ t('bogie2') }}<input v-model.number="carriage.bogie2Position" type="number" step="0.1" /></label></div></section></template>
          <template v-else-if="tab==='models' && carriage"><section v-for="group in slotGroups" :key="group.key" class="model-card"><div class="model-card-head"><h2>{{ group.label }}</h2><button :disabled="importing" @click="addModel(group.key)"><Upload :size="14" />{{ group.layers.length ? t('addLayer') : t('import') }}</button></div><div v-if="!group.layers.length" class="empty-slot"><FileBox :size="22" /><span>{{ t('noModelAssigned') }}</span></div><div v-for="layer in group.layers" :key="layer.id" :class="['model-layer-card',{active:layer.id===selectedLayerId}]" @click="selectLayer(layer)"><div class="layer-title"><button @click.stop="layer.visible=!layer.visible"><component :is="layer.visible?Eye:EyeOff" :size="15" /></button><span><strong>{{ layer.name }}</strong><small>{{ assets[layer.assetId]?.sourceFormat?.toUpperCase() }}</small></span><ChevronRight :size="15" /></div><label class="switch-row"><span>{{ t('visible') }}</span><input v-model="layer.visible" type="checkbox" /></label><label class="switch-row"><span>{{ t('uvFlip') }}</span><input v-model="layer.flipTextureV" type="checkbox" /></label><button class="binding-row"><span>{{ t('materialBindings') }}</span><span>{{ assets[layer.assetId]?.dependencies.length || 0 }} <ChevronRight :size="14" /></span></button><div v-if="assets[layer.assetId]?.warnings.length" class="inline-warning"><AlertTriangle :size="14" />{{ assets[layer.assetId]?.warnings[0] }}</div></div></section></template>
          <template v-else-if="tab==='placement' && placementRule"><section class="inspector-section"><h2>{{ selectedPartId ? t('partPlacement') : t('carriagePlacement') }}</h2><p class="inspector-hint">{{ selectedPartId ? t('partPlacementHint') : t('carriagePlacementHint') }}</p><label>{{ t('preset') }}<select v-model="placementRule.preset"><option value="all">{{ t('allCars') }}</option><option value="first">{{ t('firstCar') }}</option><option value="last">{{ t('lastCar') }}</option><option value="odd">{{ t('oddCars') }}</option><option value="even">{{ t('evenCars') }}</option><option value="every">{{ t('everyCars') }}</option><option value="custom">{{ t('custom') }}</option></select></label><div v-if="placementRule.preset==='every'" class="field-grid"><label>{{ t('interval') }}<input v-model.number="placementRule.every" type="number" min="1" /></label><label>{{ t('offset') }}<input v-model.number="placementRule.offset" type="number" /></label></div><template v-if="placementRule.preset==='custom'"><label>{{ t('whitelist') }}<input v-model="placementRule.whitelist" placeholder="1,-1,%2" /></label><label>{{ t('blacklist') }}<input v-model="placementRule.blacklist" placeholder="2,%4" /></label></template></section></template>
          <template v-else-if="tab==='mtr3'"><section class="inspector-section"><h2>{{ t('mtr3Compatibility') }}</h2><p class="inspector-hint">{{ t('mtr3Hint') }}</p><label>{{ t('baseTrainType') }}<input v-model="train.mtr3BaseTrainType" placeholder="sp1900" /></label><div class="compat-note"><AlertTriangle :size="17" /><span>{{ t('mtr3LengthHint') }}</span></div></section></template>
        </div>
      </aside>
    </div>
  </div>
  <div v-else class="editor-loading">{{ t('loading') }}</div>
</template>

<style scoped>
.train-editor{height:100%;min-height:0;display:flex;flex-direction:column;overflow:hidden;background:#171c21;color:#eef3f8}.editor-toolbar{height:52px;flex:none;display:flex;align-items:center;gap:10px;padding:0 10px;border-bottom:1px solid #39434d;background:#1b2127}.tool-group{display:flex;align-items:center;gap:4px;padding-right:10px;border-right:1px solid #3b454f}.tool-group:last-child{border:0;padding:0}.tool-group button{height:34px;display:flex;align-items:center;gap:7px;border:1px solid transparent;border-radius:6px;background:transparent;color:#cbd4de;padding:0 10px;font-size:11px}.tool-group button:hover:not(:disabled),.tool-group button.active{border-color:#6f8294;background:#303b45;color:#f5f8fb}.tool-group button:disabled{opacity:.35}.tool-group .back-button{border-color:#46525d}.tool-spacer{flex:1}.editor-body{flex:1;min-height:0;display:grid;grid-template-columns:255px minmax(380px,1fr) 326px}.carriage-panel{min-height:0;display:flex;flex-direction:column;border-right:1px solid #39434d;background:#1d2329;padding:10px}.train-identity{height:68px;display:flex;align-items:center;gap:10px;padding:8px;border-bottom:1px solid #39434d;margin:-10px -10px 10px}.train-thumbnail{width:60px;height:48px;display:grid;place-items:center;flex:none;border:1px solid #4b5864;border-radius:6px;background:linear-gradient(145deg,#43515e,#202830);color:#dce7f1}.train-identity>div:nth-child(2){min-width:0;flex:1}.train-identity strong,.train-identity span{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.train-identity strong{font-size:12px}.train-identity span{color:#8996a2;font-size:9px;margin-top:5px}.carriage-search,.tree-search{height:35px;display:flex;align-items:center;gap:8px;border:1px solid #46525e;border-radius:6px;background:#1a2025;color:#9ba8b4;padding:0 10px}.carriage-search input,.tree-search input{min-width:0;flex:1;border:0;background:transparent;color:#e8edf2;outline:0;font-size:10px}.panel-heading{display:flex;align-items:center;justify-content:space-between;margin:14px 3px 9px}.panel-heading strong{font-size:12px}.panel-heading button{display:flex;align-items:center;gap:5px;border:1px solid #4b5864;border-radius:5px;background:#293139;color:#dce4ec;padding:5px 8px;font-size:10px}.carriage-cards{min-height:0;overflow:auto;display:flex;flex-direction:column;gap:7px}.carriage-card{width:100%;height:72px;display:flex;align-items:center;gap:10px;border:1px solid #3c4650;border-radius:7px;background:#232a31;color:#dce3ea;padding:7px;text-align:left}.carriage-card:hover{border-color:#667584}.carriage-card.active{border-color:#9cc7ed;background:#303d49;box-shadow:inset 0 0 0 1px #75a6d0}.carriage-preview{width:76px;height:51px;display:flex;align-items:center;justify-content:center;flex:none;border-radius:5px;background:linear-gradient(#51606c,#29343d);overflow:hidden}.mini-car{width:66px;height:25px;display:flex;gap:5px;align-items:center;border:2px solid #c8d1d8;border-radius:3px;background:#707d87;box-shadow:0 6px 0 #232a30}.mini-car i{width:13px;height:12px;background:#202d38;border:1px solid #a6b1ba}.carriage-copy{min-width:0;flex:1}.carriage-copy strong,.carriage-copy small{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.carriage-copy strong{font-size:11px}.carriage-copy small{font-size:8px;color:#9aa6b1;margin-top:5px}.carriage-number{align-self:flex-start;color:#697784;font-size:9px}.carriage-actions{display:grid;grid-template-columns:35px 35px 1fr 1fr;gap:5px;margin-top:9px}.carriage-actions button{height:34px;display:flex;align-items:center;justify-content:center;gap:5px;border:1px solid #46525d;border-radius:5px;background:#252d34;color:#cbd4dd;font-size:9px}.carriage-actions button:disabled{opacity:.35}.editor-center{min-width:0;min-height:0;display:grid;grid-template-rows:minmax(260px,1fr) 265px 28px;background:#151a1f}.viewport-panel{min-width:0;min-height:0;position:relative;border-bottom:1px solid #39434d}.viewport-badge{position:absolute;top:10px;left:10px;display:flex;align-items:center;gap:6px;border:1px solid #495661;border-radius:5px;background:#1b2228d9;color:#cbd6df;padding:6px 9px;font-size:9px}.model-warning{position:absolute;left:12px;right:12px;bottom:12px;display:flex;gap:8px;align-items:center;padding:8px 10px;border:1px solid #80683c;border-radius:6px;background:#3b3325e8;color:#f1d59c;font-size:9px}.hierarchy-panel{min-height:0;display:flex;flex-direction:column;background:#1b2127}.hierarchy-tabs{height:36px;display:flex;align-items:end;border-bottom:1px solid #39434d;padding-left:10px}.hierarchy-tabs button{height:36px;border:0;border-bottom:2px solid transparent;background:none;color:#99a6b2;padding:0 13px;font-size:10px}.hierarchy-tabs button.active{color:#dfefff;border-color:#9dc9f0}.tree-search{height:29px;margin:7px 9px}.tree-table,.materials-list{min-height:0;overflow:auto;padding:0 9px 7px}.tree-head,.tree-row{display:grid;grid-template-columns:minmax(180px,1fr) 110px 45px;align-items:center;min-height:24px;padding:0 8px;color:#b9c4ce;font-size:9px}.tree-head{color:#7f8c98;border-bottom:1px solid #303943}.tree-row{width:100%;border:0;border-radius:3px;background:none;text-align:left}.tree-row:hover,.tree-row.active{background:#35485a;color:#f2f7fb}.tree-row>span{display:flex;align-items:center;gap:6px;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.tree-row small{color:#84919d;font-size:8px}.tree-row>svg{justify-self:center}.tree-row.layer{padding-left:23px}.tree-row.part{padding-left:49px}.materials-list{padding-top:4px}.material-row{display:flex;align-items:center;gap:10px;border-bottom:1px solid #303943;padding:8px;color:#c7d1da}.material-row span{min-width:0}.material-row strong,.material-row small{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.material-row strong{font-size:10px}.material-row small{font-size:8px;color:#83909c;margin-top:3px}.tree-empty{display:grid;place-items:center;height:110px;color:#7f8c98;font-size:10px}.editor-status{display:flex;align-items:center;gap:9px;border-top:1px solid #39434d;background:#1a2025;color:#8e9aa6;padding:0 10px;font-size:8px}.editor-status i{width:2px;height:2px;border-radius:50%;background:#687580}.status-right{margin-left:auto;display:flex;align-items:center;gap:5px}.inspector{min-height:0;border-left:1px solid #39434d;background:#1d2329;overflow:hidden;display:flex;flex-direction:column}.inspector-tabs{height:48px;flex:none;display:grid;grid-template-columns:repeat(4,1fr);border-bottom:1px solid #39434d}.inspector-tabs button{border:0;border-bottom:2px solid transparent;background:transparent;color:#a3afba;font-size:10px}.inspector-tabs button.active{color:#dceeff;border-color:#9ecbf1;background:#222a31}.inspector-content{min-height:0;overflow:auto;padding:12px}.inspector-section,.model-card{border:1px solid #3c4852;border-radius:7px;background:#222a31;padding:12px;margin-bottom:10px}.inspector h2{font:600 13px Outfit,sans-serif;margin:0 0 12px}.inspector label{display:flex;flex-direction:column;gap:5px;color:#9daab6;font-size:9px;margin-bottom:10px}.inspector input,.inspector textarea,.inspector select{width:100%;border:1px solid #46535f;border-radius:5px;background:#171d22;color:#eef2f6;padding:7px 8px;outline:0;font-size:10px}.inspector textarea{resize:vertical}.field-grid{display:grid;grid-template-columns:1fr 1fr;gap:7px}.model-card-head{display:flex;align-items:center;justify-content:space-between;margin-bottom:9px}.model-card-head h2{margin:0}.model-card-head button{display:flex;align-items:center;gap:5px;border:1px solid #4b5965;border-radius:5px;background:#2e3841;color:#d8e1e9;padding:5px 7px;font-size:8px}.empty-slot{height:48px;display:flex;align-items:center;justify-content:center;gap:8px;color:#778590;font-size:9px;border:1px dashed #46515b;border-radius:5px}.model-layer-card{border:1px solid #3b4751;border-radius:6px;background:#1c2329;padding:8px;margin-top:7px}.model-layer-card.active{border-color:#79a8d1}.layer-title{display:flex;align-items:center;gap:7px;margin-bottom:8px}.layer-title>button{border:0;background:none;color:#bcc7d0;padding:0}.layer-title>span{min-width:0;flex:1}.layer-title strong,.layer-title small{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.layer-title strong{font-size:10px}.layer-title small{font-size:7px;color:#82909b;margin-top:2px}.switch-row{height:28px!important;display:flex!important;flex-direction:row!important;align-items:center;justify-content:space-between;margin:0!important;border-top:1px solid #303a43}.switch-row input{width:auto}.binding-row{width:100%;height:29px;display:flex;align-items:center;justify-content:space-between;border:0;border-top:1px solid #303a43;background:none;color:#aeb9c3;font-size:9px;padding:0}.binding-row span:last-child{display:flex;align-items:center}.inline-warning,.compat-note{display:flex;gap:7px;color:#e4c98f;background:#373025;border-radius:5px;padding:8px;font-size:8px;line-height:1.35;margin-top:6px}.inspector-hint{color:#8f9ca7;font-size:9px;line-height:1.5}.editor-loading{height:400px;display:grid;place-items:center;color:#99a5af}@media(max-width:1200px){.editor-body{grid-template-columns:210px minmax(330px,1fr) 280px}.edit-tools button span,.carriage-actions button span{display:none}}@media(max-width:950px){.editor-body{grid-template-columns:170px minmax(300px,1fr) 245px}.tool-group button{padding:0 7px}.view-tools button{font-size:0}.editor-center{grid-template-rows:minmax(240px,1fr) 220px 28px}}
</style>
