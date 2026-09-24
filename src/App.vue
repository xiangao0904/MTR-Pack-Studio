<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { ArrowRight, Clock3, Folder, FolderOpen, Grid2X2, HelpCircle, Home, Info, List, Minus, MoreHorizontal, Plus, Save, Search, Settings2, Square, X } from '@lucide/vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { listen } from '@tauri-apps/api/event'
import { t } from './i18n'
import ProjectWorkspace from './components/ProjectWorkspace.vue'
import ProjectArtwork from './components/ProjectArtwork.vue'
import { chooseProjectSavePath, closeProject, createProject, loadRecentProjects, openProject, removeRecentProject, saveProject, takePendingProjectPath, type ProjectSummary } from './lib/projects'

function displayPath(path: string): string {
  if (path.startsWith('\\\\?\\UNC\\')) return `\\\\${path.slice(8)}`
  if (/^\\\\\?\\[a-zA-Z]:\\/.test(path)) return path.slice(4)
  return path
}

type Page = 'home' | 'recent' | 'all'
const page = ref<Page>('home')
const view = ref<'list' | 'grid'>('list')
const projects = ref<ProjectSummary[]>([])
const query = ref('')
const creating = ref(false)
const projectName = ref('')
const projectFile = ref('')
const busy = ref(false)
const notice = ref('')
const menuFor = ref<string | null>(null)
const activeProject = ref<ProjectSummary | null>(null)
const workspace = ref<{ flush: () => Promise<void>; overview: () => Promise<void> }>()
const editorContext = ref<{name:string;status:'saving'|'saved'|'failed'}>({name:'',status:'saved'})
const editing = computed(()=>!!activeProject.value && !!editorContext.value.name)
const filteredProjects = computed(() => projects.value.filter(project => `${project.name} ${project.path}`.toLowerCase().includes(query.value.toLowerCase())))

const desktopListeners: (() => void)[] = []
let unmounted = false
let closing = false
function keepListener(unlisten: () => void) { if (unmounted) unlisten(); else desktopListeners.push(unlisten) }
onBeforeUnmount(() => { unmounted = true; desktopListeners.forEach(unlisten => unlisten()) })
onMounted(async () => {
  await refreshProjects()
  if (isDesktop) {
    keepListener(await listen('open-project-file', async () => {
      const path = await takePendingProjectPath()
      if (path) await openExternalProject(path)
    }))
    keepListener(await getCurrentWindow().onCloseRequested(async event => {
      if (closing) { event.preventDefault(); return }
      closing = true
      try {
        if (activeProject.value) await flushWorkspace()
      } catch (error) { closing = false; event.preventDefault(); showError(error) }
    }))
    const pending = await takePendingProjectPath()
    if (pending) await openExternalProject(pending)
  }
})
async function refreshProjects() { try { projects.value = await loadRecentProjects() } catch (error) { showError(error) } }
async function flushWorkspace() { if (workspace.value) await workspace.value.flush(); else await saveProject() }
async function saveNow() { try { await flushWorkspace(); notice.value = t('projectSaved') } catch (error) { showError(error) } }
function showError(error: unknown) { notice.value = error instanceof Error ? error.message : String(error) }
const isDesktop = '__TAURI_INTERNALS__' in window
async function windowAction(action: 'minimize' | 'toggleMaximize' | 'close') {
  if (!isDesktop) return
  try { await getCurrentWindow()[action]() } catch (error) { showError(error) }
}
async function dragTitlebar(event: MouseEvent) {
  if (!isDesktop || event.button !== 0 || (event.target as Element).closest('button')) return
  try {
    if (event.detail === 2) await getCurrentWindow().toggleMaximize()
    else await getCurrentWindow().startDragging()
  } catch (error) { showError(error) }
}
async function chooseProject() {
  try { busy.value = true; if (activeProject.value) await flushWorkspace(); const project = await openProject(); if (project) { activeProject.value = project; await refreshProjects() } }
  catch (error) { showError(error) } finally { busy.value = false }
}
async function chooseProjectFile() {
  try { const path = await chooseProjectSavePath(projectName.value.trim() || 'Untitled Project'); if (path) projectFile.value = path }
  catch (error) { showError(error) }
}
async function submitProject() {
  if (!projectName.value.trim()) return
  try {
    busy.value = true
    if (activeProject.value) await flushWorkspace()
    if (!projectFile.value.trim()) {
      const path = await chooseProjectSavePath(projectName.value.trim())
      if (!path) return
      projectFile.value = path
    }
    activeProject.value = await createProject(projectName.value.trim(), projectFile.value.trim())
    creating.value = false; projectName.value = ''; projectFile.value = ''
    await refreshProjects()
  } catch (error) { showError(error) } finally { busy.value = false }
}
async function reopenProject(project: ProjectSummary) {
  try { if (activeProject.value) await flushWorkspace(); activeProject.value = await openProject(project.path); await refreshProjects() } catch (error) { showError(error) }
}
async function forgetProject(path: string) {
  try { await removeRecentProject(path); menuFor.value = null; await refreshProjects() } catch (error) { showError(error) }
}
async function returnHome() {
  try { await flushWorkspace(); await closeProject(); activeProject.value = null; await refreshProjects() } catch (error) { showError(error) }
}
async function openExternalProject(path: string) {
  try {
    if (activeProject.value?.path === path) return
    if (activeProject.value) await flushWorkspace()
    const project = await openProject(path)
    if (project) activeProject.value = project
    await refreshProjects()
  } catch (error) { showError(error) }
}
function dateLabel(timestamp: number) {
  const date = new Date(timestamp)
  return date.toDateString() === new Date().toDateString() ? t('today') : date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })
}
</script>

<template>
  <div :class="['app-shell', {'studio-shell': activeProject}]">
    <header :class="['titlebar',{'workspace-titlebar':activeProject}]" @mousedown="dragTitlebar">
      <div class="brand-mark">M</div><div class="brand-copy"><div class="brand-name">MTR Pack Studio <span class="beta">Beta</span></div><div class="brand-tagline">{{ t('productTagline') }}</div></div>
      <nav v-if="activeProject" class="editor-breadcrumbs"><button @click="returnHome">{{ t('home') }}</button><span>/</span><button @click="workspace?.overview()">{{ activeProject?.name }}</button><template v-if="editing"><span>/</span><strong>{{ editorContext.name }}</strong></template></nav><span v-if="activeProject" class="editor-save-wrap"><Transition name="studio-status"><span :key="editorContext.status" :class="['editor-save',editorContext.status]">{{ editorContext.status==='saving'?t('saving'):editorContext.status==='failed'?t('saveFailed'):t('saved') }}</span></Transition></span>
      <div class="titlebar-actions"><button v-if="activeProject" class="utility-action" :title="t('saveProjectShortcut')" @click="saveNow"><Save :size="19" />{{ t('saveProject') }}</button><button class="utility-action" @click="notice = t('settingsLater')"><Settings2 :size="19" />{{ t('settings') }}</button><button class="utility-action" @click="notice = t('helpLater')"><HelpCircle :size="19" />{{ t('help') }}</button><span class="titlebar-divider" aria-hidden="true"></span><button class="window-control" :aria-label="t('minimize')" :title="t('minimize')" @click="windowAction('minimize')"><Minus :size="19" /></button><button class="window-control" :aria-label="t('maximize')" :title="t('maximize')" @click="windowAction('toggleMaximize')"><Square :size="16" /></button><button class="window-control close-control" :aria-label="t('close')" :title="t('close')" @click="windowAction('close')"><X :size="20" /></button></div>
    </header>
    <Transition :name="activeProject ? 'project-enter' : 'project-return'" mode="out-in">
    <div v-if="!activeProject" class="body-shell">
      <aside class="sidebar"><nav aria-label="Main navigation">
        <button :class="['nav-item', { selected: page === 'home' }]" @click="page = 'home'"><Home :size="22" fill="currentColor" />{{ t('home') }}</button>
        <button class="nav-item" @click="creating = true"><Plus :size="24" />{{ t('newProject') }}</button>
        <button class="nav-item" @click="chooseProject"><Folder :size="22" />{{ t('openProject') }}</button>
        <div class="nav-rule"></div>
        <button :class="['nav-item', { selected: page === 'recent' }]" @click="page = 'recent'"><Clock3 :size="22" />{{ t('recentProjects') }}</button>
        <button :class="['nav-item', { selected: page === 'all' }]" @click="page = 'all'"><List :size="22" />{{ t('allProjects') }}</button>
      </nav><div class="sidebar-bottom"><img class="train-watermark" src="/images/sidebar-train.svg" alt="" aria-hidden="true" /><strong>MTR Pack Studio</strong><span>{{ t('productSubline') }}</span><small>v0.1.0 &nbsp; Beta</small></div></aside>
      <main class="main-panel"><div class="hero-image"></div><div class="main-scroll">
        <section v-if="page === 'home'" class="welcome"><span class="eyebrow">{{ t('workspaceEyebrow') }}</span><h1>{{ t('welcome') }}</h1><p>{{ t('subtitle') }}</p><div class="quick-actions">
          <button class="quick-card primary" @click="creating = true"><Plus :size="36" /><span><strong>{{ t('newProject') }}</strong><small>{{ t('newHint') }}</small></span><ArrowRight class="quick-arrow" :size="21" /></button>
          <button class="quick-card secondary" @click="chooseProject"><Folder :size="33" /><span><strong>{{ t('openProject') }}</strong><small>{{ t('openHint') }}</small></span><ArrowRight class="quick-arrow" :size="21" /></button>
        </div></section>
        <section class="projects-section"><div class="section-heading"><div><span class="eyebrow">{{ page === 'all' ? t('libraryEyebrow') : t('recentEyebrow') }}</span><h2>{{ page === 'all' ? t('allProjects') : t('recentProjects') }}</h2></div><div class="section-tools"><label class="search-box"><Search :size="18" /><input v-model="query" :placeholder="t('search')" /></label><div class="view-toggle"><button :class="{ active: view === 'grid' }" :aria-label="t('gridView')" @click="view = 'grid'"><Grid2X2 :size="19" /></button><button :class="{ active: view === 'list' }" :aria-label="t('listView')" @click="view = 'list'"><List :size="20" /></button></div></div></div>
          <div v-if="filteredProjects.length" :class="['project-collection', view]"><div v-if="view === 'list'" class="list-header"><span>{{ t('name') }}</span><span>{{ t('lastOpened') }}</span><span>{{ t('location') }}</span></div><div v-for="project in filteredProjects" :key="project.path" class="project-row" @click="reopenProject(project)"><div class="project-identity"><div class="project-thumb"><ProjectArtwork :path="project.path" :revision="project.lastOpened" /></div><div><strong>{{ project.name }}</strong><small>{{ t('pack') }}</small></div></div><span class="project-date">{{ dateLabel(project.lastOpened) }}</span><span class="project-path" :title="displayPath(project.path)">{{ displayPath(project.path) }}</span><div class="row-menu"><button :aria-label="t('more')" @click.stop="menuFor = menuFor === project.path ? null : project.path"><MoreHorizontal :size="20" /></button><div v-if="menuFor === project.path" class="menu-popover"><button @click.stop="forgetProject(project.path)">{{ t('remove') }}</button></div></div></div></div>
          <div v-else class="empty-projects"><FolderOpen :size="36" :stroke-width="1.4" /><strong>{{ query ? t('noMatches') : t('empty') }}</strong><span>{{ query ? t('trySearch') : t('emptyHint') }}</span></div>
        </section><button class="drop-zone" @click="chooseProject"><Folder :size="31" /><span><strong>{{ t('drop') }}</strong><small>{{ t('dropHint') }}</small></span></button>
      </div><footer class="statusbar"><span><Info :size="17" />{{ t('tip') }}</span><span>✦ &nbsp; {{ t('footerCredit') }}</span></footer></main>
    </div>
    <ProjectWorkspace v-else ref="workspace" :key="activeProject.path" :project="activeProject" @back="returnHome" @editor-context="editorContext=$event" />
    </Transition>
    <Transition name="studio-dialog"><div v-if="creating" class="modal-scrim" @click.self="creating = false"><form class="create-modal" @submit.prevent="submitProject"><div class="modal-head"><div><span class="eyebrow">{{ t('packEyebrow') }}</span><h2>{{ t('create') }}</h2></div><button type="button" class="close-button" @click="creating = false"><X :size="20" /></button></div><label>{{ t('name') }}<input v-model="projectName" autofocus maxlength="80" :placeholder="t('newPlaceholder')" /></label><label>{{ t('projectFile') }}<span class="folder-field"><input v-model="projectFile" :placeholder="t('projectFileHint')" /><button type="button" @click="chooseProjectFile">{{ t('browse') }}</button></span></label><p>{{ t('projectFileHelp') }}</p><div class="modal-actions"><button type="button" class="cancel-button" @click="creating = false">{{ t('cancel') }}</button><button class="create-button" type="submit" :disabled="busy || !projectName.trim()">{{ t('create') }}</button></div></form></div></Transition>
    <div v-if="notice" class="toast" role="alert">{{ notice }}<button @click="notice = ''"><X :size="16" /></button></div>
  </div>
</template>
