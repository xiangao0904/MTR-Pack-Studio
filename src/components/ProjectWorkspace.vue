<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { AlertTriangle, ArrowRight, Box, Database, Folder, Home, Monitor, MoreHorizontal, Plus, Search, Settings2, TrainFront, TreePine, Upload, X } from '@lucide/vue'
import { t } from '../i18n'
import ProjectArtwork from './ProjectArtwork.vue'
const TrainEditor = defineAsyncComponent(() => import('./TrainEditor.vue'))
import { chooseExportPath, createTrain, exportResourcePack, getProject, saveProject, setProjectCover, updateProjectSettings, validateExport, type ContentEntry, type ExportOptions, type ProjectData, type ProjectSummary, type ValidationIssue } from '../lib/projects'

const props = defineProps<{ project: ProjectSummary }>()
const emit = defineEmits<{ back: [] }>()
type Section = 'overview' | 'all' | 'trains' | 'objects' | 'pids' | 'assets' | 'settings'
const section = ref<Section>('overview')
const data = ref<ProjectData | null>(null)
const loading = ref(true)
const error = ref('')
const query = ref('')
const creating = ref(false)
const trainName = ref('')
const busy = ref(false)
const selectedTrain = ref<ContentEntry | null>(null)
const notice = ref('')
const saveStatus = ref<'saving' | 'saved' | 'failed'>('saved')
const trainEditor = ref<{ flush: () => Promise<void>; focusIssue: (issue: ValidationIssue) => Promise<void> }>()
const pendingIssue = ref<ValidationIssue | null>(null)
const settingsNamespace = ref('')
const settingsDescription = ref('')
const coverRevision = ref(0)
const coverInput = ref<HTMLInputElement>()
const coverBusy = ref(false)
let coverWrite: Promise<void> | null = null
let pendingCover: { file: File | null } | null = null
let settingsTimer: ReturnType<typeof setTimeout> | undefined
let settingsWrite: Promise<void> | null = null
const exportOpen = ref(false)
const exportBusy = ref(false)
const exportIssues = ref<ValidationIssue[]>([])
const exportOptions = ref<ExportOptions>(JSON.parse(localStorage.getItem('mtr-pack-studio:last-export') || 'null') || { target: 'mtr4', minecraftVersion: '1.20.4', modelFormat: 'obj' })
const mtr3Versions = ['1.16.5', '1.17.1', '1.18.2', '1.19.2', '1.19.3', '1.19.4', '1.20.1']

const allItems = computed(() => [...(data.value?.content || [])].sort((a, b) => b.updatedAt - a.updatedAt))
const visibleItems = computed(() => allItems.value.filter(item => (section.value !== 'trains' || item.kind === 'train') && item.name.toLowerCase().includes(query.value.toLowerCase())))
const recentItems = computed(() => allItems.value.slice(0, 4))
const sectionTitle = computed(() => ({ all: t('allContentHeading'), trains: t('trainsHeading'), objects: t('decorativeObjects'), pids: t('pids'), assets: t('assetLibrary'), settings: t('projectSettings') } as Partial<Record<Section, string>>)[section.value] || '')

async function load() {
  loading.value = true
  try { data.value = await getProject(props.project.path); settingsNamespace.value = data.value.namespace; settingsDescription.value = data.value.description; error.value = '' }
  catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) }
  finally { loading.value = false }
}
onMounted(() => { load(); window.addEventListener('keydown', saveShortcut) })
onBeforeUnmount(() => { clearTimeout(settingsTimer); window.removeEventListener('keydown', saveShortcut) })
watch(() => props.project.path, load)
watch([settingsNamespace, settingsDescription], () => {
  clearTimeout(settingsTimer)
  if (!data.value || (settingsNamespace.value === data.value.namespace && settingsDescription.value === data.value.description)) return
  saveStatus.value = 'saving'
  settingsTimer = setTimeout(() => { void flushSave().catch(() => {}) }, 800)
})

async function flushSave() {
  saveStatus.value = 'saving'
  clearTimeout(settingsTimer)
  try { await trainEditor.value?.flush(); await persistCover(); await persistSettings(); await saveProject(); saveStatus.value = 'saved'; error.value = '' }
  catch (cause) { saveStatus.value = 'failed'; error.value = cause instanceof Error ? cause.message : String(cause); throw cause }
}
defineExpose({ flush: flushSave })
async function navigate(next: Section) {
  try { await flushSave(); selectedTrain.value = null; section.value = next } catch { /* The current editor stays open so the user can retry. */ }
}
async function leaveTrain() { await navigate('trains') }
async function openTrain(entry: ContentEntry) {
  try { await flushSave(); selectedTrain.value = entry; section.value = 'trains' } catch { /* Keep unsaved edits visible. */ }
}
function updateEntry(entry: ContentEntry) {
  if (!data.value) return
  const index = data.value.content.findIndex(item => item.id === entry.id); if (index >= 0) data.value.content[index] = entry
  selectedTrain.value = entry
}
function openExport() { exportIssues.value = []; exportOpen.value = true }
function changeTarget() { exportOptions.value.minecraftVersion = exportOptions.value.target === 'mtr4' ? '1.20.4' : '1.20.1'; exportIssues.value = [] }
async function submitExport() {
  if (!data.value) return
  try {
    exportBusy.value = true; await flushSave(); exportIssues.value = await validateExport(exportOptions.value)
    if (exportIssues.value.some(issue => issue.severity === 'error')) return
    const path = await chooseExportPath(data.value.name, exportOptions.value.target); if (!path) return
    const report = await exportResourcePack(path, exportOptions.value); localStorage.setItem('mtr-pack-studio:last-export', JSON.stringify(exportOptions.value)); exportOpen.value = false; notice.value = `${t('exportComplete')} ${report.fileCount} ${t('files')}`
  } catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) } finally { exportBusy.value = false }
}
async function openIssue(issue: ValidationIssue) {
  if (!data.value) return
  if (!issue.trainId) { exportOpen.value = false; await navigate('settings'); return }
  const entry = data.value.content.find(item => item.id === issue.trainId)
  if (!entry) return
  try {
    await flushSave(); pendingIssue.value = issue; exportOpen.value = false
    if (selectedTrain.value?.id === entry.id && trainEditor.value) await applyPendingIssue()
    else { selectedTrain.value = entry; section.value = 'trains' }
  } catch { /* Retain the current document when saving fails. */ }
}
async function applyPendingIssue() {
  if (!pendingIssue.value || !trainEditor.value) return
  try { await trainEditor.value.focusIssue(pendingIssue.value); pendingIssue.value = null }
  catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) }
}
async function persistSettings() {
  if (settingsWrite) await settingsWrite
  if (!data.value || (settingsNamespace.value === data.value.namespace && settingsDescription.value === data.value.description)) return
  settingsWrite = (async () => {
    while (data.value && (settingsNamespace.value !== data.value.namespace || settingsDescription.value !== data.value.description)) {
      const saved = await updateProjectSettings(props.project.path, settingsNamespace.value, settingsDescription.value)
      data.value.namespace = saved.namespace; data.value.description = saved.description
    }
  })()
  try { await settingsWrite } finally { settingsWrite = null }
}
async function saveSettings() { try { await flushSave(); notice.value = t('settingsSaved') } catch { /* Show the error from flushSave. */ } }
async function changeCover(file: File | null) {
  if (coverBusy.value) return
  coverBusy.value = true; saveStatus.value = 'saving'
  pendingCover = { file }
  try { await persistCover(); saveStatus.value = 'saved' }
  catch (cause) { saveStatus.value = 'failed'; error.value = cause instanceof Error ? cause.message : String(cause) }
  finally { coverBusy.value = false; if (coverInput.value) coverInput.value.value = '' }
}
async function persistCover() {
  if (coverWrite) { await coverWrite; return }
  if (!pendingCover) return
  const request = pendingCover
  coverWrite = setProjectCover(props.project.path, request.file)
  try { await coverWrite; if (pendingCover === request) pendingCover = null; coverRevision.value++ }
  finally { coverWrite = null }
}
function coverSelected(event: Event) { const file = (event.target as HTMLInputElement).files?.[0]; if (file) void changeCover(file) }
function saveShortcut(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') { event.preventDefault(); void flushSave().catch(() => {}) }
}

async function submitTrain() {
  if (!trainName.value.trim()) return
  busy.value = true
  saveStatus.value = 'saving'
  try {
    await flushSave()
    await createTrain(props.project.path, trainName.value.trim())
    creating.value = false
    trainName.value = ''
    section.value = 'trains'
    await load()
    saveStatus.value = 'saved'
  } catch (cause) { saveStatus.value = 'failed'; error.value = cause instanceof Error ? cause.message : String(cause) }
  finally { busy.value = false }
}
function formatDate(timestamp: number) {
  const date = new Date(timestamp)
  const time = date.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false })
  return `${date.toDateString() === new Date().toDateString() ? t('today') : date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })} ${time}`
}
</script>

<template>
  <div class="workbench">
    <div class="workbench-toolbar">
      <div class="breadcrumbs"><button @click="emit('back')"><Home :size="18" fill="currentColor" />{{ t('home') }}</button><span>/</span><strong>{{ data?.name || project.name }}</strong></div>
      <div class="toolbar-right"><span :class="['save-state', saveStatus]">{{ saveStatus === 'saving' ? t('saving') : saveStatus === 'failed' ? t('saveFailed') : t('saved') }}</span><button class="export-button" @click="openExport"><Upload :size="18" />{{ t('exportPack') }}</button></div>
    </div>
    <div class="workbench-body">
      <aside v-if="!selectedTrain" class="workbench-sidebar">
        <div class="project-identity"><div class="project-image"><ProjectArtwork :path="project.path" :revision="coverRevision" /></div><div><strong>{{ data?.name || project.name }}</strong><span>{{ t('packEyebrow') }}</span></div></div>
        <nav :aria-label="t('allContent')">
          <button :class="['work-nav', { active: section === 'overview' }]" @click="navigate('overview')"><Home :size="19" fill="currentColor" />{{ t('overview') }}</button>
          <span class="nav-caption">{{ t('contentGroup') }}</span>
          <button :class="['work-nav', { active: section === 'all' }]" @click="navigate('all')"><Box :size="19" />{{ t('allContent') }}</button>
          <button :class="['work-nav', { active: section === 'trains' }]" @click="navigate('trains')"><TrainFront :size="19" />{{ t('trains') }}</button>
          <button :class="['work-nav', { active: section === 'objects' }]" @click="navigate('objects')"><TreePine :size="19" />{{ t('decorativeObjects') }}<small>{{ t('planned') }}</small></button>
          <button :class="['work-nav', { active: section === 'pids' }]" @click="navigate('pids')"><Monitor :size="19" />{{ t('pids') }}<small>{{ t('planned') }}</small></button>
          <div class="work-nav-rule"></div><span class="nav-caption">{{ t('projectGroup') }}</span>
          <button :class="['work-nav', { active: section === 'assets' }]" @click="navigate('assets')"><Database :size="19" />{{ t('assetLibrary') }}</button>
          <button :class="['work-nav', { active: section === 'settings' }]" @click="navigate('settings')"><Settings2 :size="19" />{{ t('projectSettings') }}</button>
        </nav>
      </aside>
      <main :class="['workbench-main',{editing:selectedTrain}]">
        <div v-if="error" class="work-error">{{ error }}<button @click="data ? saveSettings() : load()">{{ t('retry') }}</button></div>
        <div v-if="data?.recovered" class="recovery-banner">{{ t('recoveredProject') }}</div>
        <template v-if="!loading && data">
          <template v-if="section === 'overview'">
            <div class="workspace-heading"><div><h1>{{ t('projectOverview') }}</h1><p>{{ t('overviewSubtitle') }}</p></div><button class="new-train" @click="creating = true"><Plus :size="22" />{{ t('newTrain') }}</button></div>
            <section class="summary-card"><span class="summary-label">{{ t('projectLabel') }}</span><h2>{{ data.name }}</h2><p>{{ data.description || t('projectSummary') }}</p><span class="summary-target">{{ data.namespace }}</span></section>
            <section class="content-types"><h2>{{ t('contentTypes') }}</h2><p>{{ t('contentTypesHint') }}</p><div class="type-cards">
              <button class="type-card" @click="navigate('trains')"><TrainFront :size="30" /><span><strong>{{ t('trains') }}</strong><small>{{ t('trainTypeHint') }}</small></span><ArrowRight :size="18" /></button>
              <button class="type-card planned-card" @click="navigate('objects')"><TreePine :size="30" /><span><strong>{{ t('decorativeObjects') }}</strong><small>{{ t('objectTypeHint') }}</small></span><em>{{ t('planned') }}</em></button>
              <button class="type-card planned-card" @click="navigate('pids')"><Monitor :size="30" /><span><strong>{{ t('pids') }}</strong><small>{{ t('pidsTypeHint') }}</small></span><em>{{ t('planned') }}</em></button>
            </div></section>
            <section class="recent-content"><div class="recent-heading"><div><h2>{{ t('recentlyEdited') }}</h2><p>{{ t('recentlyEditedHint') }}</p></div><button @click="navigate('all')">{{ t('viewAll') }}<ArrowRight :size="16" /></button></div><div class="content-table"><div class="content-table-head"><span>{{ t('itemName') }}</span><span>{{ t('itemType') }}</span><span>{{ t('itemLastEdited') }}</span></div><button v-for="item in recentItems" :key="item.id" class="content-row" @click="openTrain(item)"><span class="item-name"><span class="item-icon"><TrainFront :size="22" /></span><span><strong>{{ item.name }}</strong><small>{{ t('trains') }}</small></span></span><span class="item-kind"><TrainFront :size="16" />{{ t('trains') }}</span><span>{{ formatDate(item.updatedAt) }}</span><MoreHorizontal :size="18" /></button><div v-if="!recentItems.length" class="content-empty"><TrainFront :size="30" /><strong>{{ t('noContent') }}</strong><span>{{ t('noContentHint') }}</span></div></div></section>
          </template>
          <template v-else>
            <div class="workspace-heading"><div><h1>{{ selectedTrain?.name || sectionTitle }}</h1><p>{{ selectedTrain ? t('trainEditorHint') : section === 'trains' ? t('trainTypeHint') : section === 'all' ? t('contentTypesHint') : section === 'assets' ? t('assetsHint') : section === 'settings' ? t('projectSettingsHint') : t('plannedHint') }}</p></div><button v-if="section === 'trains' || section === 'all'" class="new-train" @click="creating = true"><Plus :size="22" />{{ t('newTrain') }}</button></div>
            <template v-if="section === 'trains' || section === 'all'"><TrainEditor v-if="selectedTrain" ref="trainEditor" :key="selectedTrain.id" :project-path="project.path" :entry="selectedTrain" @back="leaveTrain" @ready="applyPendingIssue" @changed="updateEntry" @status="saveStatus=$event" @error="error=$event" /><template v-else><label class="content-search"><Search :size="18" /><input v-model="query" :placeholder="t('contentSearch')" /></label><div class="content-table list-table"><div class="content-table-head"><span>{{ t('itemName') }}</span><span>{{ t('itemType') }}</span><span>{{ t('itemLastEdited') }}</span></div><button v-for="item in visibleItems" :key="item.id" class="content-row" @click="openTrain(item)"><span class="item-name"><span class="item-icon"><TrainFront :size="22" /></span><span><strong>{{ item.name }}</strong><small>{{ t('trains') }}</small></span></span><span class="item-kind"><TrainFront :size="16" />{{ t('trains') }}</span><span>{{ formatDate(item.updatedAt) }}</span><MoreHorizontal :size="18" /></button><div v-if="!visibleItems.length" class="content-empty"><TrainFront :size="30" /><strong>{{ section === 'trains' ? t('noTrains') : t('noContent') }}</strong><span>{{ section === 'trains' ? t('noTrainsHint') : t('noContentHint') }}</span></div></div></template></template>
            <form v-else-if="section === 'settings'" class="settings-panel" @submit.prevent="saveSettings"><Settings2 :size="28" /><h2>{{ t('projectSettings') }}</h2><section class="cover-settings"><div class="cover-preview"><ProjectArtwork :path="project.path" :revision="coverRevision" /></div><div><h3>{{ t('projectCover') }}</h3><p>{{ t('projectCoverHint') }}</p><div class="cover-actions"><button type="button" :disabled="coverBusy" @click="coverInput?.click()"><Upload :size="15" />{{ t('uploadCover') }}</button><button type="button" :disabled="coverBusy" @click="changeCover(null)">{{ t('removeCover') }}</button></div><input ref="coverInput" type="file" accept="image/png,image/jpeg,image/webp" hidden @change="coverSelected" /></div></section><label>{{ t('namespace') }}<input v-model="settingsNamespace" pattern="[a-z0-9_.-]+" required /><small>{{ t('namespaceHint') }}</small></label><label>{{ t('description') }}<textarea v-model="settingsDescription" rows="4" /></label><button class="new-train" type="submit">{{ t('saveSettings') }}</button></form>
            <div v-else class="placeholder-panel"><component :is="section === 'objects' ? TreePine : section === 'pids' ? Monitor : Folder" :size="42" /><h2>{{ sectionTitle }}</h2><p>{{ section === 'assets' ? t('assetsHint') : t('plannedHint') }}</p></div>
          </template>
        </template>
      </main>
    </div>
    <div v-if="creating" class="work-modal-scrim" @click.self="creating = false"><form class="work-modal" @submit.prevent="submitTrain"><div><h2>{{ t('newTrain') }}</h2><button type="button" :aria-label="t('close')" @click="creating = false"><X :size="18" /></button></div><label>{{ t('trainName') }}<input v-model="trainName" autofocus maxlength="80" :placeholder="t('trainNamePlaceholder')" /></label><footer><button type="button" @click="creating = false">{{ t('cancel') }}</button><button type="submit" :disabled="busy || !trainName.trim()">{{ t('createTrain') }}</button></footer></form></div>
    <div v-if="exportOpen" class="work-modal-scrim" @click.self="exportOpen = false"><form class="work-modal export-modal" @submit.prevent="submitExport"><div><h2>{{ t('exportPack') }}</h2><button type="button" :aria-label="t('close')" @click="exportOpen=false"><X :size="18" /></button></div><label>{{ t('exportTarget') }}<select v-model="exportOptions.target" @change="changeTarget"><option value="mtr4">MTR 4</option><option value="mtr3_nte">MTR 3 + NTE</option></select></label><label>{{ t('minecraftVersion') }}<select v-model="exportOptions.minecraftVersion"><option v-for="version in exportOptions.target === 'mtr4' ? ['1.20.4'] : mtr3Versions" :key="version">{{ version }}</option></select></label><label v-if="exportOptions.target==='mtr4'">{{ t('modelFormat') }}<select v-model="exportOptions.modelFormat"><option value="obj">OBJ</option><option value="mqo">MQO</option></select></label><div v-if="exportIssues.length" class="export-issues"><button v-for="(issue,index) in exportIssues" :key="index" type="button" :class="issue.severity" @click="openIssue(issue)"><AlertTriangle :size="15" /><span>{{ issue.message }}</span></button></div><footer><button type="button" @click="exportOpen=false">{{ t('cancel') }}</button><button type="submit" :disabled="exportBusy">{{ exportBusy ? t('validating') : t('continueExport') }}</button></footer></form></div>
    <div v-if="notice" class="work-notice" role="status">{{ notice }}<button @click="notice = ''"><X :size="16" /></button></div>
  </div>
</template>

<style scoped>
.workbench{flex:1;min-height:0;display:flex;flex-direction:column;background:#191e22;color:#f3f5f8}.workbench-toolbar{height:50px;flex:none;display:flex;align-items:center;justify-content:space-between;padding:0 26px;border-bottom:1px solid #353d45;background:#1d2227}.breadcrumbs,.breadcrumbs button,.toolbar-right{display:flex;align-items:center;gap:14px}.breadcrumbs{font-size:13px;color:#d4dce5}.breadcrumbs button{border:0;background:none;color:#d4dce5;padding:0}.breadcrumbs span{color:#7e8993}.breadcrumbs strong{font-weight:500}.pack-chip,.export-button{height:34px;border:1px solid #3f4953;border-radius:7px;background:#282f36;color:#dce3eb;display:flex;align-items:center;gap:10px;padding:0 12px;font-size:12px}.pack-chip{font-size:11px;letter-spacing:.04em}.export-button{background:#444d59;border-color:#444d59;font-weight:600}.export-button:hover{background:#56616e}.workbench-body{min-height:0;flex:1;display:flex}.workbench-sidebar{width:250px;flex:none;border-right:1px solid #353d45;background:linear-gradient(150deg,#22282e,#1b2024);padding:22px 15px;overflow:auto}.project-identity{display:flex;align-items:center;gap:12px;margin:0 8px 20px;min-width:0}.project-image{width:68px;height:68px;display:grid;place-items:center;flex:none;border:1px solid #58636d;border-radius:7px;background:linear-gradient(135deg,#77899a,#303c47 50%,#151a1e);color:#f2f5f9}.project-identity div:last-child{min-width:0}.project-identity strong,.project-identity span{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.project-identity strong{font-size:13px}.project-identity span{font-size:10px;color:#aeb8c4;margin-top:7px;letter-spacing:.06em}.workbench-sidebar nav{display:flex;flex-direction:column;gap:4px}.work-nav{min-height:42px;width:100%;display:flex;align-items:center;gap:14px;border:0;border-radius:7px;padding:0 13px;text-align:left;background:transparent;color:#d6dde5;font-size:13px;white-space:nowrap}.work-nav:hover{background:#313941}.work-nav.active{background:#3b434d;color:#fff;box-shadow:inset 2px 0 #f0f4f9}.work-nav small,.type-card em{margin-left:auto;font-size:10px;font-style:normal;color:#c0c8d2;background:#343c45;border-radius:20px;padding:4px 8px}.nav-caption{font-size:12px;color:#8995a1;margin:17px 13px 7px}.work-nav-rule{height:1px;background:#3e4750;margin:14px 8px 0}.workbench-main{min-width:0;flex:1;overflow:auto;padding:23px clamp(25px,3vw,42px) 45px}.workspace-heading{display:flex;align-items:center;justify-content:space-between;gap:20px;margin-bottom:21px}.workspace-heading h1{font:600 34px/1.15 Outfit,'Segoe UI',sans-serif;margin:0 0 4px}.workspace-heading p{margin:0;color:#c6ced8;font-size:15px}.new-train{height:39px;display:flex;align-items:center;gap:9px;white-space:nowrap;border:1px solid #f8fbff;border-radius:8px;padding:0 17px;background:linear-gradient(120deg,#f8fbff,#dfe8f6);color:#1e2730;font-weight:700;font-size:13px}.new-train:hover{filter:brightness(.93)}.summary-card{min-height:166px;position:relative;border:1px solid #454e57;border-radius:10px;padding:26px 31px;background:linear-gradient(120deg,#262e35,#1d2429);overflow:hidden}.summary-card:after{content:'';position:absolute;right:-35px;bottom:-100px;width:280px;height:280px;border:1px solid #ffffff0b;border-radius:50%;box-shadow:0 0 0 48px #ffffff05,0 0 0 96px #ffffff03;pointer-events:none}.summary-label{color:#b7c2cd;font-size:10px;letter-spacing:.2em}.summary-card h2{font:600 31px Outfit,'Segoe UI',sans-serif;margin:13px 0 5px}.summary-card p{color:#c0c9d2;font-size:13px;margin:0}.summary-target{display:block;color:#8e9aa6;font-size:10px;letter-spacing:.14em;margin-top:15px}.content-types,.recent-content{margin-top:22px}.content-types h2,.recent-content h2{font:600 18px Outfit,'Segoe UI',sans-serif;margin:0 0 3px}.content-types>p,.recent-content p{color:#aab4be;font-size:12px;margin:0 0 13px}.type-cards{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:12px}.type-card{min-height:90px;border:1px solid #59636d;border-radius:9px;background:linear-gradient(125deg,#283038,#22282e);color:#ecf0f5;display:flex;align-items:center;gap:16px;padding:15px 17px;text-align:left}.type-card:hover{border-color:#bdc9d6}.type-card>svg{flex:none}.type-card span{display:flex;flex-direction:column;gap:7px;min-width:0}.type-card strong{font-size:13px}.type-card small{font-size:11px;line-height:1.4;color:#aeb8c3}.type-card>svg:last-child{margin-left:auto}.planned-card{color:#a7b0ba;border-color:#414a53;background:#20262b}.planned-card em{align-self:flex-start;white-space:nowrap}.recent-heading{display:flex;justify-content:space-between;align-items:start}.recent-heading button{display:flex;align-items:center;gap:8px;border:0;background:none;color:#b9c3ce;font-size:12px}.content-table{border:1px solid #39434c;border-radius:9px;overflow:hidden;background:#1e2529}.content-table-head,.content-row{display:grid;grid-template-columns:minmax(180px,2fr) minmax(100px,1fr) minmax(130px,1fr) 22px;align-items:center;column-gap:12px;padding:0 17px}.content-table-head{height:32px;background:#293138;color:#b3bec9;font-size:10px;letter-spacing:.04em}.content-row{width:100%;min-height:58px;border:0;border-top:1px solid #354049;background:transparent;color:#c7d0da;text-align:left;font-size:12px}.content-row:hover{background:#29323a}.item-name,.item-kind{display:flex;align-items:center;gap:12px}.item-name strong,.item-name small{display:block}.item-name strong{color:#f3f5f8;font-size:13px}.item-name small{color:#9ca8b3;font-size:10px;margin-top:3px}.item-icon{width:39px;height:39px;display:grid;place-items:center;border:1px solid #64717e;border-radius:6px;background:#303d48}.content-empty{min-height:145px;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:7px;color:#9facb8;font-size:12px}.content-empty strong{color:#e8edf2;font-size:13px}.content-search{height:40px;max-width:330px;display:flex;align-items:center;gap:10px;border:1px solid #4b5661;border-radius:8px;padding:0 12px;color:#b9c4ce;margin:12px 0 18px}.content-search input{width:100%;border:0;outline:0;background:transparent;color:#fff;font-size:12px}.placeholder-panel{min-height:250px;border:1px solid #414b54;border-radius:10px;background:#22292f;display:flex;flex-direction:column;align-items:center;justify-content:center;color:#a9b5c0;margin-top:18px;text-align:center;padding:30px}.placeholder-panel h2{color:#f1f4f7;margin:15px 0 3px;font-size:18px}.placeholder-panel p{font-size:13px}.placeholder-panel button{border:1px solid #5c6975;border-radius:6px;background:#35404a;color:#e9eff5;padding:8px 12px}.work-error{border:1px solid #a26363;border-radius:7px;padding:12px;margin-bottom:12px;color:#ffc5c5}.work-error button{margin-left:10px;background:none;border:0;color:inherit;text-decoration:underline}.work-modal-scrim{position:fixed;z-index:20;inset:0;background:#080b0dc9;display:grid;place-items:center}.work-modal{width:min(430px,calc(100vw - 32px));background:#282f36;border:1px solid #5c6874;border-radius:12px;padding:25px;box-shadow:0 20px 80px #0009}.work-modal>div,.work-modal footer{display:flex;align-items:center;justify-content:space-between}.work-modal h2{font:600 23px Outfit,'Segoe UI',sans-serif;margin:0}.work-modal button{border:0;background:none;color:#dce4ec}.work-modal label{display:flex;flex-direction:column;gap:8px;margin-top:28px;font-size:12px}.work-modal input{height:40px;border:1px solid #5c6874;background:#1c2329;color:#fff;border-radius:7px;padding:0 11px;outline:0}.work-modal footer{justify-content:flex-end;gap:10px;margin-top:27px}.work-modal footer button{border:1px solid #5c6874;border-radius:7px;padding:9px 13px}.work-modal footer button:last-child{background:#eaf2fb;border-color:#fff;color:#1b232c;font-weight:700}.work-modal footer button:disabled{opacity:.5}.work-notice{position:fixed;z-index:22;right:20px;bottom:20px;background:#39434d;border:1px solid #6b7885;border-radius:8px;padding:11px 14px;display:flex;align-items:center;gap:14px;font-size:12px}.work-notice button{background:none;border:0;color:#fff}@media(max-width:1100px){.workbench-sidebar{width:205px}.type-cards{grid-template-columns:1fr}.type-card{min-height:75px}}@media(max-width:800px){.workbench-sidebar{width:65px;padding:18px 8px}.project-identity,.nav-caption,.work-nav-rule{display:none}.work-nav{font-size:0;justify-content:center;padding:0}.work-nav small{display:none}.workspace-heading h1{font-size:27px}.pack-chip{display:none}}
.save-state{font-size:11px;color:#9faab5;white-space:nowrap}.save-state.saving{color:#d8e3ee}.save-state.failed{color:#ff9e9e}.recovery-banner{margin-bottom:16px;border:1px solid #8a7041;border-radius:8px;background:#3b3325;color:#f3d9a4;padding:11px 14px;font-size:12px}
.workbench-main.editing{overflow:hidden;padding:0}.workbench-main.editing>.workspace-heading{display:none}.workbench-main.editing>.work-error,.workbench-main.editing>.recovery-banner{position:absolute;z-index:8;left:20px;right:20px}.workbench-main.editing .train-editor{height:100%}
.work-modal select,.settings-panel input,.settings-panel textarea{height:40px;border:1px solid #5c6874;background:#1c2329;color:#fff;border-radius:7px;padding:0 11px;outline:0}.export-modal label{margin-top:16px}.export-issues{max-height:170px;overflow:auto;margin-top:16px;display:flex!important;flex-direction:column;align-items:stretch!important;gap:6px}.export-issues button{display:flex;align-items:flex-start;gap:8px;text-align:left;border:1px solid #7b6262;border-radius:6px;background:#39292b;color:#ffc3c3;padding:8px;font-size:10px}.export-issues button.warning{border-color:#79683f;background:#383226;color:#efd49b}.settings-panel{max-width:620px;border:1px solid #414b54;border-radius:10px;background:#22292f;padding:28px}.settings-panel h2{font:600 22px Outfit,sans-serif}.settings-panel label{display:flex;flex-direction:column;gap:7px;color:#c4cdd6;font-size:12px;margin:18px 0}.settings-panel textarea{height:auto;padding:10px;resize:vertical}.settings-panel small{color:#8995a1}.settings-panel .new-train{margin-top:24px}
.work-nav>svg{flex-shrink:0}.cover-settings{display:flex;align-items:center;gap:20px;padding:20px 0;border-bottom:1px solid #3d4851}.cover-preview{width:104px;height:104px;border:1px solid #586674;border-radius:12px;flex:none;overflow:hidden}.cover-settings h3{font-size:14px;margin:0 0 7px}.cover-settings p{color:#a7b5c1;font-size:12px;line-height:1.5;margin:0 0 12px}.cover-actions{display:flex;flex-wrap:wrap;gap:8px}.cover-actions button{display:flex;align-items:center;gap:7px;border:1px solid #586775;background:#35424e;color:#e7edf4;border-radius:6px;padding:7px 10px;font-size:11px}.cover-actions button:disabled{opacity:.5}.cover-settings input[hidden]{display:none}.project-image{overflow:hidden}
</style>

