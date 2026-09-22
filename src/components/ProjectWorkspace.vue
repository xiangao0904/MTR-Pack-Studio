<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { AlertTriangle, ArrowRight, Box, Database, Folder, Home, Monitor, MoreHorizontal, Plus, Search, Settings2, TrainFront, TreePine, Upload, X } from '@lucide/vue'
import { t } from '../i18n'
import ProjectArtwork from './ProjectArtwork.vue'
const TrainEditor = defineAsyncComponent(() => import('./TrainEditor.vue'))
import { chooseExportPath, createTrain, exportResourcePack, getProject, saveProject, setProjectCover, updateProjectSettings, validateExport, type ContentEntry, type ExportOptions, type ProjectData, type ProjectSummary, type ValidationIssue } from '../lib/projects'

const props = defineProps<{ project: ProjectSummary }>()
const emit = defineEmits<{ back: []; editorContext: [context: {name:string;status:'saving'|'saved'|'failed'}] }>()
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
watch([selectedTrain,saveStatus],()=>emit('editorContext',{name:selectedTrain.value?.name || '',status:saveStatus.value}),{immediate:true})
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
defineExpose({ flush: flushSave, overview: () => navigate('overview') })
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
function openExport() { exportOptions.value.onlyVisible = false; exportIssues.value = []; exportOpen.value = true }
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
    <div v-if="!selectedTrain" class="workbench-toolbar">
      <span class="workspace-section-label">{{ section === 'overview' ? t('overview') : sectionTitle }}</span>
      <div class="toolbar-right"><button class="export-button" @click="openExport"><Upload :size="18" />{{ t('exportPack') }}</button></div>
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
            <template v-if="section === 'trains' || section === 'all'"><TrainEditor v-if="selectedTrain" ref="trainEditor" :key="selectedTrain.id" :project-path="project.path" :entry="selectedTrain" @back="leaveTrain" @export="openExport" @ready="applyPendingIssue" @changed="updateEntry" @status="saveStatus=$event" @error="error=$event" /><template v-else><label class="content-search"><Search :size="18" /><input v-model="query" :placeholder="t('contentSearch')" /></label><div class="content-table list-table"><div class="content-table-head"><span>{{ t('itemName') }}</span><span>{{ t('itemType') }}</span><span>{{ t('itemLastEdited') }}</span></div><button v-for="item in visibleItems" :key="item.id" class="content-row" @click="openTrain(item)"><span class="item-name"><span class="item-icon"><TrainFront :size="22" /></span><span><strong>{{ item.name }}</strong><small>{{ t('trains') }}</small></span></span><span class="item-kind"><TrainFront :size="16" />{{ t('trains') }}</span><span>{{ formatDate(item.updatedAt) }}</span><MoreHorizontal :size="18" /></button><div v-if="!visibleItems.length" class="content-empty"><TrainFront :size="30" /><strong>{{ section === 'trains' ? t('noTrains') : t('noContent') }}</strong><span>{{ section === 'trains' ? t('noTrainsHint') : t('noContentHint') }}</span></div></div></template></template>
            <form v-else-if="section === 'settings'" class="settings-panel" @submit.prevent="saveSettings"><section class="cover-settings"><div class="cover-preview"><ProjectArtwork :path="project.path" :revision="coverRevision" /></div><div><h3>{{ t('projectCover') }}</h3><p>{{ t('projectCoverHint') }}</p><div class="cover-actions"><button type="button" :disabled="coverBusy" @click="coverInput?.click()"><Upload :size="15" />{{ t('uploadCover') }}</button><button type="button" :disabled="coverBusy" @click="changeCover(null)">{{ t('removeCover') }}</button></div><input ref="coverInput" type="file" accept="image/png,image/jpeg,image/webp" hidden @change="coverSelected" /></div></section><label>{{ t('namespace') }}<input v-model="settingsNamespace" pattern="[a-z0-9_.-]+" required /><small>{{ t('namespaceHint') }}</small></label><label>{{ t('description') }}<textarea v-model="settingsDescription" rows="4" /></label><button class="new-train" type="submit">{{ t('saveSettings') }}</button></form>
            <div v-else class="placeholder-panel"><component :is="section === 'objects' ? TreePine : section === 'pids' ? Monitor : Folder" :size="42" /><h2>{{ sectionTitle }}</h2><p>{{ section === 'assets' ? t('assetsHint') : t('plannedHint') }}</p></div>
          </template>
        </template>
      </main>
    </div>
    <div v-if="creating" class="work-modal-scrim" @click.self="creating = false"><form class="work-modal" @submit.prevent="submitTrain"><div><h2>{{ t('newTrain') }}</h2><button type="button" :aria-label="t('close')" @click="creating = false"><X :size="18" /></button></div><label>{{ t('trainName') }}<input v-model="trainName" autofocus maxlength="80" :placeholder="t('trainNamePlaceholder')" /></label><footer><button type="button" @click="creating = false">{{ t('cancel') }}</button><button type="submit" :disabled="busy || !trainName.trim()">{{ t('createTrain') }}</button></footer></form></div>
    <div v-if="exportOpen" class="work-modal-scrim" @click.self="exportOpen = false"><form class="work-modal export-modal" @submit.prevent="submitExport"><div><h2>{{ t('exportPack') }}</h2><button type="button" :aria-label="t('close')" @click="exportOpen=false"><X :size="18" /></button></div><label>{{ t('exportTarget') }}<select v-model="exportOptions.target" @change="changeTarget"><option value="mtr4">MTR 4</option><option value="mtr3_nte">MTR 3 + NTE</option></select></label><label>{{ t('minecraftVersion') }}<select v-model="exportOptions.minecraftVersion"><option v-for="version in exportOptions.target === 'mtr4' ? ['1.20.4'] : mtr3Versions" :key="version">{{ version }}</option></select></label><label v-if="exportOptions.target==='mtr4'">{{ t('modelFormat') }}<select v-model="exportOptions.modelFormat"><option value="obj">OBJ</option><option value="mqo">MQO</option></select></label><label class="export-visibility"><input v-model="exportOptions.onlyVisible" type="checkbox" /><span>{{ t('onlyExportVisible') }}<small>{{ t('onlyExportVisibleHint') }}</small></span></label><div v-if="exportIssues.length" class="export-issues"><button v-for="(issue,index) in exportIssues" :key="index" type="button" :class="issue.severity" @click="openIssue(issue)"><AlertTriangle :size="15" /><span>{{ issue.message }}</span></button></div><footer><button type="button" @click="exportOpen=false">{{ t('cancel') }}</button><button type="submit" :disabled="exportBusy">{{ exportBusy ? t('validating') : t('continueExport') }}</button></footer></form></div>
    <div v-if="notice" class="work-notice" role="status">{{ notice }}<button @click="notice = ''"><X :size="16" /></button></div>
  </div>
</template>

<style scoped>
.workbench {
  --workspace-line: var(--studio-border, #ffffff0d);
  --workspace-text: var(--studio-text, #eef0f3);
  --workspace-muted: var(--studio-muted, #92969f);
  --workspace-quiet: var(--studio-quiet, #686d77);
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--studio-bg, #191b1f);
  color: var(--workspace-text);
  font-family: var(--studio-font, 'Inter', 'Segoe UI', sans-serif);
  font-size: 13px;
  -webkit-font-smoothing: antialiased;
}
.workbench button, .workbench input, .workbench select, .workbench textarea { font: inherit; }
.workbench button { cursor: pointer; }
.workbench button:disabled { opacity: .45; cursor: not-allowed; }
.workbench button:focus-visible { outline: 2px solid #a5aab5; outline-offset: 3px; }
.workbench-toolbar {
  height: 40px;
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  padding: 0 24px;
  border-bottom: 1px solid var(--workspace-line);
  background: var(--studio-bg, #191b1f);
}
.workspace-section-label { color: var(--workspace-muted); font-size: 11px; }
.toolbar-right { display: flex; align-items: center; flex: none; }
.export-button, .new-train {
  height: 32px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  flex: none;
  white-space: nowrap;
  padding: 0 12px;
  border: 1px solid #e4e5e8;
  border-radius: 6px;
  background: #e4e5e8;
  color: #25272c;
  font-size: 12px;
  font-weight: 600;
  box-shadow: 0 1px 2px #0002;
}
.export-button:hover, .new-train:hover { background: #f5f5f6; border-color: #f5f5f6; }
.export-button svg, .new-train svg { width: 15px; height: 15px; }
.workbench-body { min-height: 0; flex: 1; display: flex; }
.workbench-sidebar {
  width: 220px;
  box-sizing: border-box;
  flex: none;
  overflow: auto;
  padding: 20px 12px;
  background: var(--studio-surface, #1d1f23);
  border-right: 1px solid var(--workspace-line);
}
.project-identity { display: flex; align-items: center; gap: 10px; margin: 0 5px 25px; min-width: 0; }
.project-image { width: 36px; height: 36px; flex: none; overflow: hidden; border: 1px solid #ffffff12; border-radius: 6px; background: #282b31; }
.project-identity > div:last-child { min-width: 0; }
.project-identity strong, .project-identity span { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.project-identity strong { font-size: 12px; font-weight: 550; }
.project-identity span { margin-top: 4px; font-size: 10px; color: var(--workspace-quiet); }
.workbench-sidebar nav { display: flex; flex-direction: column; gap: 3px; }
.work-nav {
  width: 100%;
  min-height: 32px;
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 0 10px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--workspace-muted);
  text-align: left;
  font-size: 12px;
  white-space: nowrap;
}
.work-nav > svg { width: 15px; height: 15px; flex-shrink: 0; stroke-width: 1.6; fill: none; }
.work-nav:hover { color: var(--workspace-text); background: var(--studio-hover, #ffffff04); }
.work-nav.active { color: var(--workspace-text); background: var(--studio-selected, #ffffff09); box-shadow: inset 2px 0 #aeb2ba; }
.work-nav small, .type-card em { margin-left: auto; color: var(--workspace-quiet); font-size: 9px; font-style: normal; font-weight: 400; }
.work-nav small { padding: 2px 5px; border: 1px solid #ffffff0a; border-radius: 4px; }
.nav-caption { margin: 22px 10px 7px; font-size: 10px; font-weight: 500; color: var(--workspace-quiet); }
.work-nav-rule { height: 1px; background: var(--workspace-line); margin: 19px 10px 0; }
.work-nav-rule + .nav-caption { margin-top: 16px; }
.workbench-main { min-width: 0; flex: 1; overflow: auto; padding: 34px clamp(24px, 4vw, 56px) 48px; }
.workspace-heading { display: flex; justify-content: space-between; align-items: center; gap: 24px; margin-bottom: 30px; }
.workspace-heading h1 { margin: 0; font: 600 28px/1.25 var(--studio-font, 'Inter', 'Segoe UI', sans-serif); letter-spacing: -.7px; }
.workspace-heading p { margin: 9px 0 0; color: var(--workspace-muted); font-size: 12px; line-height: 1.6; }
.summary-card { padding: 0 0 26px; border: 0; border-bottom: 1px solid var(--workspace-line); background: none; }
.summary-label { color: var(--workspace-quiet); font-size: 10px; letter-spacing: .07em; }
.summary-card h2 { margin: 9px 0 7px; font-size: 18px; font-weight: 550; letter-spacing: -.25px; }
.summary-card p { max-width: 720px; margin: 0; color: var(--workspace-muted); font-size: 12px; line-height: 1.7; overflow-wrap: anywhere; }
.summary-target { display: inline-block; margin-top: 12px; color: var(--workspace-quiet); font-family: 'Cascadia Code', Consolas, monospace; font-size: 10px; }
.content-types, .recent-content { margin-top: 28px; }
.content-types h2, .recent-content h2 { margin: 0; font-size: 13px; font-weight: 550; letter-spacing: -.1px; }
.content-types > p, .recent-content p { margin: 5px 0 14px; color: var(--workspace-quiet); font-size: 11px; line-height: 1.6; }
.type-cards { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 10px; }
.type-card { position: relative; min-height: 82px; display: flex; align-items: center; gap: 12px; padding: 14px; border: 1px solid var(--workspace-line); border-radius: 6px; background: var(--studio-surface, #1d1f23); color: #c9ccd3; text-align: left; }
.type-card:hover { background: var(--studio-hover, #ffffff04); border-color: #ffffff19; }
.type-card > svg { width: 21px; height: 21px; flex: none; stroke-width: 1.5; color: var(--workspace-muted); }
.type-card span { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
.type-card strong { font-size: 12px; font-weight: 550; color: var(--workspace-text); }
.type-card small { font-size: 10px; line-height: 1.5; color: var(--workspace-muted); }
.type-card > svg:last-child { margin-left: auto; width: 14px; height: 14px; }
.planned-card strong { color: var(--workspace-muted); }
.planned-card small { color: var(--workspace-quiet); }
.planned-card em { align-self: flex-start; white-space: nowrap; }
.recent-heading { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
.recent-heading button { display: flex; align-items: center; gap: 6px; padding: 0; border: 0; background: none; color: var(--workspace-muted); font-size: 11px; }
.recent-heading button:hover { color: var(--workspace-text); }
.content-table { overflow: hidden; border-top: 1px solid var(--workspace-line); border-bottom: 1px solid var(--workspace-line); }
.content-table-head, .content-row { display: grid; grid-template-columns: minmax(160px, 2fr) minmax(90px, 1fr) minmax(125px, 1fr) 18px; align-items: center; gap: 12px; padding: 0 12px; }
.content-table-head { height: 34px; color: var(--workspace-quiet); font-size: 10px; }
.content-row { width: 100%; min-height: 62px; border: 0; border-top: 1px solid var(--workspace-line); background: transparent; color: var(--workspace-muted); text-align: left; font-size: 11px; }
.content-row:hover { background: var(--studio-hover, #ffffff04); }
.content-row > svg { color: var(--workspace-quiet); }
.item-name, .item-kind { display: flex; align-items: center; gap: 10px; min-width: 0; }
.item-name > span:last-child { min-width: 0; }
.item-name strong, .item-name small { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.item-name strong { color: var(--workspace-text); font-size: 12px; font-weight: 500; }
.item-name small { margin-top: 4px; color: var(--workspace-quiet); font-size: 10px; }
.item-kind { gap: 7px; color: var(--workspace-quiet); }
.item-kind svg { width: 13px; height: 13px; }
.item-icon { width: 32px; height: 32px; display: grid; place-items: center; flex: none; border: 1px solid #ffffff0a; border-radius: 5px; background: var(--studio-raised, #25272c); color: #adb1ba; }
.item-icon svg { width: 18px; height: 18px; stroke-width: 1.5; }
.content-empty { min-height: 180px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 9px; padding: 24px; color: var(--workspace-quiet); text-align: center; font-size: 11px; }
.content-empty > svg { width: 24px; height: 24px; margin-bottom: 4px; stroke-width: 1.3; }
.content-empty strong { color: var(--workspace-muted); font-size: 12px; font-weight: 500; }
.content-search { height: 32px; max-width: 300px; display: flex; align-items: center; gap: 8px; margin: 0 0 18px; padding: 0 10px; border: 1px solid #ffffff12; border-radius: 6px; background: #ffffff02; color: var(--workspace-quiet); }
.content-search > svg { width: 14px; height: 14px; }
.content-search:focus-within { border-color: #ffffff30; }
.content-search input { width: 100%; border: 0; outline: 0; background: none; color: var(--workspace-text); font-size: 12px; }
.content-search input::placeholder { color: var(--workspace-quiet); }
.placeholder-panel { min-height: 300px; display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 32px; border: 1px dashed #ffffff0e; border-radius: 6px; color: var(--workspace-quiet); text-align: center; }
.placeholder-panel > svg { width: 30px; height: 30px; stroke-width: 1.25; }
.placeholder-panel h2 { margin: 18px 0 0; color: var(--workspace-muted); font-size: 15px; font-weight: 500; }
.placeholder-panel p { max-width: 420px; font-size: 12px; line-height: 1.7; }
.settings-panel { width: 100%; max-width: 660px; padding: 0; }
.cover-settings { display: flex; align-items: center; gap: 20px; padding: 0 0 28px; margin-bottom: 25px; border-bottom: 1px solid var(--workspace-line); }
.cover-preview { width: 88px; height: 88px; overflow: hidden; flex: none; border: 1px solid #ffffff12; border-radius: 6px; background: #282b31; }
.cover-settings h3 { margin: 0 0 6px; font-size: 12px; font-weight: 550; }
.cover-settings p { margin: 0 0 13px; color: var(--workspace-muted); font-size: 11px; line-height: 1.6; }
.cover-actions { display: flex; flex-wrap: wrap; gap: 8px; }
.cover-actions button { height: 30px; display: inline-flex; align-items: center; gap: 6px; padding: 0 10px; border: 1px solid #ffffff12; border-radius: 6px; background: var(--studio-raised, #25272c); color: #c4c7cf; font-size: 11px; }
.cover-actions button:hover { background: #303238; }
.cover-actions button:last-child { background: transparent; border-color: transparent; color: var(--workspace-muted); }
.cover-settings input[hidden] { display: none; }
.settings-panel > label, .work-modal > label { display: flex; flex-direction: column; gap: 8px; color: #c3c6cd; font-size: 12px; }
.settings-panel > label { margin: 22px 0; }
.settings-panel input, .settings-panel textarea, .work-modal input:not([type=checkbox]), .work-modal select { box-sizing: border-box; width: 100%; height: 32px; padding: 0 10px; border: 1px solid #ffffff14; border-radius: 6px; outline: 0; background: #17191d; color: var(--workspace-text); font-size: 12px; }
.settings-panel input:focus, .settings-panel textarea:focus, .work-modal input:focus, .work-modal select:focus { border-color: #92969f; }
.settings-panel textarea { min-height: 100px; height: auto; padding: 10px; resize: vertical; line-height: 1.6; }
.settings-panel small { color: var(--workspace-quiet); font-size: 11px; line-height: 1.5; }
.settings-panel > .new-train { margin-top: 24px; }
.work-error, .recovery-banner { display: flex; align-items: center; gap: 12px; margin-bottom: 18px; padding: 11px 14px; border: 1px solid #b879792e; border-radius: 6px; background: #b8797909; color: #d3a3a3; font-size: 12px; line-height: 1.5; }
.work-error button { margin-left: auto; border: 0; background: none; color: inherit; text-decoration: underline; white-space: nowrap; }
.recovery-banner { color: #cbb690; border-color: #cbb6902e; background: #cbb69009; }
.workbench-main.editing { position: relative; overflow: hidden; padding: 0; }
.workbench-main.editing > .workspace-heading { display: none; }
.workbench-main.editing > .work-error, .workbench-main.editing > .recovery-banner { position: absolute; z-index: 8; left: 20px; right: 20px; top: 8px; }
.workbench-main.editing .train-editor { height: 100%; }
.work-modal-scrim { position: fixed; z-index: 100; inset: 0; display: grid; place-items: center; padding: 24px 0; overflow: auto; background: #0009; backdrop-filter: blur(3px); }
.work-modal { box-sizing: border-box; width: min(440px, calc(100vw - 32px)); max-height: calc(100vh - 48px); overflow: auto; padding: 24px; border: 1px solid #ffffff16; border-radius: 9px; background: #202226; box-shadow: 0 24px 80px #0008; }
.work-modal > div:first-child, .work-modal footer { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.work-modal h2 { margin: 0; font: 550 17px/1.4 var(--studio-font, 'Inter', 'Segoe UI', sans-serif); letter-spacing: -.2px; }
.work-modal > div:first-child > button { width: 26px; height: 26px; display: grid; place-items: center; padding: 0; border: 0; border-radius: 4px; background: none; color: var(--workspace-muted); }
.work-modal > div:first-child > button:hover { background: #ffffff08; }
.work-modal > label { margin-top: 22px; }
.work-modal footer { justify-content: flex-end; gap: 8px; margin-top: 28px; padding-top: 18px; border-top: 1px solid var(--workspace-line); }
.work-modal footer button { height: 32px; padding: 0 12px; border: 1px solid #ffffff14; border-radius: 6px; background: #ffffff03; color: #b8bdc6; font-size: 12px; }
.work-modal footer button:hover { background: #ffffff09; }
.work-modal footer button:last-child { border-color: #e4e5e8; background: #e4e5e8; color: #25272c; font-weight: 550; }
.export-modal > label { margin-top: 20px; }
.work-modal .export-visibility { flex-direction: row; align-items: flex-start; gap: 9px; }
.export-visibility input { width: 14px; height: 14px; margin: 2px 0 0; flex: none; accent-color: #c9cbd1; }
.export-visibility small { display: block; margin-top: 5px; color: var(--workspace-quiet); font-size: 11px; line-height: 1.5; }
.export-issues { display: flex; flex-direction: column; align-items: stretch; max-height: 180px; overflow: auto; gap: 7px; margin-top: 18px; }
.export-issues button { display: flex; align-items: flex-start; gap: 8px; padding: 10px; border: 1px solid #b879792e; border-radius: 6px; background: #b8797909; color: #d3a3a3; text-align: left; font-size: 11px; line-height: 1.5; }
.export-issues button > svg { flex: none; margin-top: 1px; }
.export-issues button.warning { color: #cbb690; border-color: #cbb6902e; background: #cbb69009; }
.work-notice { position: fixed; z-index: 120; right: 22px; bottom: 22px; display: flex; align-items: center; gap: 18px; max-width: calc(100vw - 44px); padding: 12px 15px; border: 1px solid #ffffff16; border-radius: 6px; background: #292b30; box-shadow: 0 8px 32px #0005; color: #d2d5dc; font-size: 12px; }
.work-notice button { display: grid; place-items: center; padding: 0; border: 0; background: none; color: var(--workspace-muted); }
@media (max-width: 1150px) {
  .workbench-main { padding: 28px 28px 40px; }
  .type-card { align-items: flex-start; padding: 13px; gap: 9px; }
  .type-card > svg { width: 18px; height: 18px; }
  .type-card em { position: absolute; right: 9px; top: 8px; font-size: 8px; }
  .planned-card span { padding-top: 8px; }
}
@media (max-width: 950px) {
  .type-cards { grid-template-columns: 1fr; gap: 8px; }
  .type-card { min-height: 62px; align-items: center; }
  .type-card em { position: static; align-self: center; font-size: 9px; }
  .planned-card span { padding-top: 0; }
  .content-table-head, .content-row { grid-template-columns: minmax(140px, 2fr) minmax(115px, 1fr) 16px; gap: 10px; padding: 0 8px; }
  .content-table-head > span:nth-child(2), .content-row > .item-kind { display: none; }
}
@media (max-width: 760px) {
  .workbench-toolbar { padding: 0 18px; }
  .workbench-sidebar { width: 58px; padding: 20px 9px; }
  .project-identity, .nav-caption, .work-nav-rule { display: none; }
  .work-nav { font-size: 0; justify-content: center; padding: 0; }
  .work-nav small { display: none; }
  .workbench-sidebar nav { gap: 8px; }
  .workbench-main { padding: 24px 20px 32px; }
  .workspace-heading { align-items: flex-start; gap: 16px; }
  .workspace-heading h1 { font-size: 24px; }
  .toolbar-right { gap: 10px; }
  .cover-settings { align-items: flex-start; gap: 14px; }
  .cover-preview { width: 64px; height: 64px; }
}
</style>
