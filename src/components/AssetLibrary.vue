<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref } from 'vue'
import { Box, Image as ImageIcon, Plus, Search, Trash2, Upload } from '@lucide/vue'
import ModelViewport, { type PreviewLayer } from './ModelViewport.vue'
import { t } from '../i18n'
import { analyzeModelImport, chooseModelDependency, chooseModelFile, chooseTextureFile, deleteAsset, getImageAsset, getModelAsset, importModelAsset, importTextureFile, listAssets, renameAsset, type AssetCatalog, type AssetDefinition } from '../lib/projects'

const props=defineProps<{projectPath:string}>()
const emit=defineEmits<{error:[message:string]}>()
const catalog=ref<AssetCatalog>({models:[],textures:[]})
const tab=ref<'model'|'texture'>('model')
const query=ref('')
const selectedId=ref('')
const selectedModel=ref<AssetDefinition>()
const selectedImage=ref('')
const busy=ref(false)
const editingName=ref('')
const visibleModels=computed(()=>catalog.value.models.filter(item=>item.name.toLowerCase().includes(query.value.toLowerCase())))
const visibleTextures=computed(()=>catalog.value.textures.filter(item=>item.name.toLowerCase().includes(query.value.toLowerCase())))
const selectedModelItem=computed(()=>catalog.value.models.find(item=>item.id===selectedId.value))
const selectedTextureItem=computed(()=>catalog.value.textures.find(item=>item.hash===selectedId.value))
const previewLayers=computed<PreviewLayer[]>(()=>selectedModel.value?[{key:selectedModel.value.id,assetId:selectedModel.value.id,layerId:selectedModel.value.id,carriageId:'asset',visible:true,z:0,reversed:false,bogieOffset:0,flipV:false,hiddenParts:[],bindings:[]}]:[])
function error(cause:unknown){emit('error',cause instanceof Error?cause.message:String(cause))}
async function refresh(){catalog.value=await listAssets(props.projectPath)}
onMounted(()=>{void refresh().catch(error)})
onBeforeUnmount(()=>{if(selectedImage.value)URL.revokeObjectURL(selectedImage.value)})
async function select(kind:'model'|'texture',id:string){
  selectedId.value=id;selectedModel.value=undefined;editingName.value=''
  if(selectedImage.value){URL.revokeObjectURL(selectedImage.value);selectedImage.value=''}
  try{
    if(kind==='model'){selectedModel.value=await getModelAsset(id);editingName.value=selectedModel.value.name}
    else {const item=catalog.value.textures.find(entry=>entry.hash===id);editingName.value=item?.name||'';selectedImage.value=URL.createObjectURL(new Blob([await getImageAsset(id)],{type:'image/png'}))}
  }catch(cause){error(cause)}
}
async function upload(){
  if(busy.value)return
  try{
    busy.value=true
    if(tab.value==='model'){
      const path=await chooseModelFile();if(!path)return
      const overrides:Record<string,string>={};let analysis=await analyzeModelImport(path,overrides)
      while(analysis.missingDependencies.length){for(const missing of analysis.missingDependencies){const name=missing.split(/[\\/]/).pop()||missing;const located=await chooseModelDependency(name);if(!located)return;overrides[name]=located}analysis=await analyzeModelImport(path,overrides)}
      const asset=await importModelAsset(path,overrides);await refresh();await select('model',asset.id)
    }else{
      const path=await chooseTextureFile();if(!path)return
      const hash=await importTextureFile(path);await refresh();await select('texture',hash)
    }
  }catch(cause){error(cause)}finally{busy.value=false}
}
async function saveName(){
  const name=editingName.value.trim();if(!selectedId.value||!name)return
  try{await renameAsset(tab.value,selectedId.value,name);await refresh();if(selectedModel.value)selectedModel.value.name=name}catch(cause){error(cause)}
}
async function remove(){
  if(!selectedId.value)return
  try{await deleteAsset(tab.value,selectedId.value);selectedId.value='';selectedModel.value=undefined;if(selectedImage.value){URL.revokeObjectURL(selectedImage.value);selectedImage.value=''}await refresh()}
  catch(cause){error(cause)}
}
</script>

<template>
  <div class="asset-library">
    <div class="library-tools"><div class="library-tabs"><button :class="{active:tab==='model'}" @click="tab='model';selectedId='';selectedModel=undefined">{{ t('assetModels') }} <span>{{ catalog.models.length }}</span></button><button :class="{active:tab==='texture'}" @click="tab='texture';selectedId='';selectedModel=undefined">{{ t('assetTextures') }} <span>{{ catalog.textures.length }}</span></button></div><label class="library-search"><Search :size="16" /><input v-model="query" :placeholder="t('searchAssets')" /></label><button class="library-upload" :disabled="busy" @click="upload"><Upload :size="15" />{{ tab==='model'?t('importModelAsset'):t('importTextureAsset') }}</button></div>
    <div class="library-body"><div class="asset-grid">
      <template v-if="tab==='model'"><button v-for="item in visibleModels" :key="item.id" :class="['asset-card',{active:selectedId===item.id}]" @click="select('model',item.id)"><span class="asset-icon"><Box :size="30" /></span><strong>{{ item.name }}</strong><small>{{ item.sourceFormat.toUpperCase() }} · {{ item.partCount }} {{ t('parts') }} · {{ item.triangleCount.toLocaleString() }} {{ t('triangles') }}</small><small>{{ item.references.length }} {{ t('assetUses') }}</small></button></template>
      <template v-else><button v-for="item in visibleTextures" :key="item.hash" :class="['asset-card',{active:selectedId===item.hash}]" @click="select('texture',item.hash)"><span class="asset-icon"><ImageIcon :size="30" /></span><strong>{{ item.name }}</strong><small>{{ item.width }} × {{ item.height }}</small><small>{{ item.references.length }} {{ t('assetUses') }}</small></button></template>
      <div v-if="!(tab==='model'?visibleModels.length:visibleTextures.length)" class="asset-empty"><Plus :size="26" /><p>{{ t('assetEmpty') }}</p></div>
    </div><aside v-if="selectedModelItem||selectedTextureItem" class="asset-detail"><div v-if="selectedModelItem" class="asset-preview"><ModelViewport :assets="previewLayers" :guides="[]" @error="error" /></div><img v-if="selectedTextureItem&&selectedImage" class="texture-preview" :src="selectedImage" alt="" /><label>{{ t('fieldName') }}<input v-model="editingName" maxlength="80" @change="saveName" /></label><p v-if="selectedModelItem">{{ selectedModelItem.partCount }} {{ t('parts') }} · {{ selectedModelItem.triangleCount.toLocaleString() }} {{ t('triangles') }}</p><p v-if="selectedTextureItem">{{ selectedTextureItem.width }} × {{ selectedTextureItem.height }}</p><h3>{{ t('assetUsedBy') }}</h3><ul v-if="(selectedModelItem||selectedTextureItem)?.references.length"><li v-for="reference in (selectedModelItem||selectedTextureItem)?.references" :key="reference">{{ reference }}</li></ul><p v-else>{{ t('assetUnused') }}</p><button class="asset-delete" :disabled="!!(selectedModelItem||selectedTextureItem)?.references.length" @click="remove"><Trash2 :size="15" />{{ t('delete') }}</button></aside></div>
  </div>
</template>

<style scoped>
.asset-library{display:flex;flex-direction:column;min-height:400px;height:calc(100% - 95px);border:1px solid #414850;border-radius:8px;overflow:hidden;background:#20252a;color:#e8edf2}.library-tools{display:flex;align-items:center;gap:14px;padding:14px;border-bottom:1px solid #3b4249}.library-tabs{display:flex;gap:5px}.library-tabs button,.library-upload,.asset-delete{border:1px solid #4b545d;border-radius:5px;background:#2a3036;color:inherit;padding:8px 11px;font:inherit;cursor:pointer}.library-tabs button.active{background:#e6edf5;color:#1a2026}.library-tabs span{opacity:.65;margin-left:4px}.library-search{display:flex;align-items:center;gap:8px;margin-left:auto;padding:7px 9px;border:1px solid #4b545d;border-radius:5px;color:#abb5c0}.library-search input{border:0;outline:0;background:none;color:inherit;width:150px}.library-upload,.asset-delete{display:flex;align-items:center;gap:7px}.library-upload:disabled,.asset-delete:disabled{opacity:.45;cursor:default}.library-body{display:flex;min-height:0;flex:1}.asset-grid{flex:1;min-width:0;overflow:auto;padding:18px;display:grid;grid-template-columns:repeat(auto-fill,minmax(190px,1fr));align-content:start;gap:14px}.asset-card{display:flex;flex-direction:column;gap:6px;text-align:left;min-width:0;padding:10px;border:1px solid #434b53;border-radius:7px;background:#282e34;color:inherit;cursor:pointer}.asset-card:hover,.asset-card.active{border-color:#c6d4e2}.asset-card strong,.asset-card small{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.asset-card small{color:#aab4be;font-size:11px}.asset-icon{height:104px;display:grid;place-items:center;background:#1c2125;color:#a9bacb;border-radius:4px}.asset-empty{grid-column:1/-1;display:grid;justify-items:center;padding:55px;color:#aab4be}.asset-detail{width:310px;flex:none;border-left:1px solid #3b4249;overflow:auto;padding:16px}.asset-preview{height:190px;margin-bottom:16px}.texture-preview{display:block;max-width:100%;max-height:190px;object-fit:contain;margin:0 auto 16px}.asset-detail label{display:grid;gap:6px;font-size:12px}.asset-detail input{padding:8px;border:1px solid #4b545d;border-radius:4px;background:#171b1f;color:inherit}.asset-detail p,.asset-detail li{color:#aeb8c1;font-size:12px;overflow-wrap:anywhere}.asset-detail h3{font-size:13px;margin-top:24px}.asset-detail ul{padding-left:18px}.asset-delete{margin-top:20px}@media(max-width:900px){.library-tools{flex-wrap:wrap}.library-search{margin-left:0}.asset-detail{width:240px}}
</style>
