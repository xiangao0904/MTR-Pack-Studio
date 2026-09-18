<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { ArrowRight, Box, ChevronDown, Database, Folder, Home, Monitor, MoreHorizontal, Package, Plus, Search, Settings2, TrainFront, Trees, Upload, X } from '@lucide/vue'
import { t } from '../i18n'
import { createTrain, getProject, type ContentEntry, type ProjectData, type ProjectSummary } from '../lib/projects'

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

const allItems = computed(() => [...(data.value?.content || [])].sort((a, b) => b.updatedAt - a.updatedAt))
const visibleItems = computed(() => allItems.value.filter(item => (section.value !== 'trains' || item.kind === 'train') && item.name.toLowerCase().includes(query.value.toLowerCase())))
const recentItems = computed(() => allItems.value.slice(0, 4))
const sectionTitle = computed(() => ({ all: t('allContentHeading'), trains: t('trainsHeading'), objects: t('decorativeObjects'), pids: t('pids'), assets: t('assetLibrary'), settings: t('projectSettings') } as Partial<Record<Section, string>>)[section.value] || '')

async function load() {
  loading.value = true
  try { data.value = await getProject(props.project.path); error.value = '' }
  catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) }
  finally { loading.value = false }
}
onMounted(load)
watch(() => props.project.path, load)

async function submitTrain() {
  if (!trainName.value.trim()) return
  busy.value = true
  try {
    await createTrain(props.project.path, trainName.value.trim())
    creating.value = false
    trainName.value = ''
    section.value = 'trains'
    await load()
  } catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) }
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
      <div class="toolbar-right"><div class="pack-chip"><Package :size="17" />{{ t('packEyebrow') }}<ChevronDown :size="15" /></div><button class="export-button" @click="notice = t('exportLater')"><Upload :size="18" />{{ t('exportPack') }}</button></div>
    </div>
    <div class="workbench-body">
      <aside class="workbench-sidebar">
        <div class="project-identity"><div class="project-image"><TrainFront :size="31" /></div><div><strong>{{ data?.name || project.name }}</strong><span>{{ t('packEyebrow') }}</span></div></div>
        <nav :aria-label="t('allContent')">
          <button :class="['work-nav', { active: section === 'overview' }]" @click="section = 'overview'; selectedTrain = null"><Home :size="19" fill="currentColor" />{{ t('overview') }}</button>
          <span class="nav-caption">{{ t('contentGroup') }}</span>
          <button :class="['work-nav', { active: section === 'all' }]" @click="section = 'all'; selectedTrain = null"><Box :size="19" />{{ t('allContent') }}</button>
          <button :class="['work-nav', { active: section === 'trains' }]" @click="section = 'trains'; selectedTrain = null"><TrainFront :size="19" />{{ t('trains') }}</button>
          <button :class="['work-nav', { active: section === 'objects' }]" @click="section = 'objects'; selectedTrain = null"><Trees :size="19" />{{ t('decorativeObjects') }}<small>{{ t('planned') }}</small></button>
          <button :class="['work-nav', { active: section === 'pids' }]" @click="section = 'pids'; selectedTrain = null"><Monitor :size="19" />{{ t('pids') }}<small>{{ t('planned') }}</small></button>
          <div class="work-nav-rule"></div><span class="nav-caption">{{ t('projectGroup') }}</span>
          <button :class="['work-nav', { active: section === 'assets' }]" @click="section = 'assets'; selectedTrain = null"><Database :size="19" />{{ t('assetLibrary') }}</button>
          <button :class="['work-nav', { active: section === 'settings' }]" @click="section = 'settings'; selectedTrain = null"><Settings2 :size="19" />{{ t('projectSettings') }}</button>
        </nav>
      </aside>
      <main class="workbench-main">
        <div v-if="error" class="work-error">{{ error }}<button @click="load">{{ t('retry') }}</button></div>
        <template v-if="!loading && data">
          <template v-if="section === 'overview'">
            <div class="workspace-heading"><div><h1>{{ t('projectOverview') }}</h1><p>{{ t('overviewSubtitle') }}</p></div><button class="new-train" @click="creating = true"><Plus :size="22" />{{ t('newTrain') }}</button></div>
            <section class="summary-card"><span class="summary-label">{{ t('projectLabel') }}</span><h2>{{ data.name }}</h2><p>{{ data.description || t('projectSummary') }}</p><span class="summary-target">{{ t('packEyebrow') }}</span></section>
            <section class="content-types"><h2>{{ t('contentTypes') }}</h2><p>{{ t('contentTypesHint') }}</p><div class="type-cards">
              <button class="type-card" @click="section = 'trains'"><TrainFront :size="30" /><span><strong>{{ t('trains') }}</strong><small>{{ t('trainTypeHint') }}</small></span><ArrowRight :size="18" /></button>
              <button class="type-card planned-card" @click="section = 'objects'"><Trees :size="30" /><span><strong>{{ t('decorativeObjects') }}</strong><small>{{ t('objectTypeHint') }}</small></span><em>{{ t('planned') }}</em></button>
              <button class="type-card planned-card" @click="section = 'pids'"><Monitor :size="30" /><span><strong>{{ t('pids') }}</strong><small>{{ t('pidsTypeHint') }}</small></span><em>{{ t('planned') }}</em></button>
            </div></section>
            <section class="recent-content"><div class="recent-heading"><div><h2>{{ t('recentlyEdited') }}</h2><p>{{ t('recentlyEditedHint') }}</p></div><button @click="section = 'all'">{{ t('viewAll') }}<ArrowRight :size="16" /></button></div><div class="content-table"><div class="content-table-head"><span>{{ t('itemName') }}</span><span>{{ t('itemType') }}</span><span>{{ t('itemLastEdited') }}</span></div><button v-for="item in recentItems" :key="item.id" class="content-row" @click="selectedTrain = item; section = 'trains'"><span class="item-name"><span class="item-icon"><TrainFront :size="22" /></span><span><strong>{{ item.name }}</strong><small>{{ t('trains') }}</small></span></span><span class="item-kind"><TrainFront :size="16" />{{ t('trains') }}</span><span>{{ formatDate(item.updatedAt) }}</span><MoreHorizontal :size="18" /></button><div v-if="!recentItems.length" class="content-empty"><TrainFront :size="30" /><strong>{{ t('noContent') }}</strong><span>{{ t('noContentHint') }}</span></div></div></section>
          </template>
          <template v-else>
            <div class="workspace-heading"><div><h1>{{ selectedTrain?.name || sectionTitle }}</h1><p>{{ selectedTrain ? t('trainEditorHint') : section === 'trains' ? t('trainTypeHint') : section === 'all' ? t('contentTypesHint') : section === 'assets' ? t('assetsHint') : section === 'settings' ? t('projectSettingsHint') : t('plannedHint') }}</p></div><button v-if="section === 'trains' || section === 'all'" class="new-train" @click="creating = true"><Plus :size="22" />{{ t('newTrain') }}</button></div>
            <template v-if="section === 'trains' || section === 'all'"><div v-if="selectedTrain" class="placeholder-panel"><TrainFront :size="40" /><h2>{{ selectedTrain.name }}</h2><p>{{ t('trainEditorHint') }}</p><button @click="selectedTrain = null">{{ t('back') }}</button></div><template v-else><label class="content-search"><Search :size="18" /><input v-model="query" :placeholder="t('contentSearch')" /></label><div class="content-table list-table"><div class="content-table-head"><span>{{ t('itemName') }}</span><span>{{ t('itemType') }}</span><span>{{ t('itemLastEdited') }}</span></div><button v-for="item in visibleItems" :key="item.id" class="content-row" @click="selectedTrain = item; section = 'trains'"><span class="item-name"><span class="item-icon"><TrainFront :size="22" /></span><span><strong>{{ item.name }}</strong><small>{{ t('trains') }}</small></span></span><span class="item-kind"><TrainFront :size="16" />{{ t('trains') }}</span><span>{{ formatDate(item.updatedAt) }}</span><MoreHorizontal :size="18" /></button><div v-if="!visibleItems.length" class="content-empty"><TrainFront :size="30" /><strong>{{ section === 'trains' ? t('noTrains') : t('noContent') }}</strong><span>{{ section === 'trains' ? t('noTrainsHint') : t('noContentHint') }}</span></div></div></template></template>
            <div v-else class="placeholder-panel"><component :is="section === 'objects' ? Trees : section === 'pids' ? Monitor : section === 'assets' ? Folder : Settings2" :size="42" /><h2>{{ sectionTitle }}</h2><p>{{ section === 'assets' ? t('assetsHint') : section === 'settings' ? t('projectSettingsHint') : t('plannedHint') }}</p></div>
          </template>
        </template>
      </main>
    </div>
    <div v-if="creating" class="work-modal-scrim" @click.self="creating = false"><form class="work-modal" @submit.prevent="submitTrain"><div><h2>{{ t('newTrain') }}</h2><button type="button" :aria-label="t('close')" @click="creating = false"><X :size="18" /></button></div><label>{{ t('trainName') }}<input v-model="trainName" autofocus maxlength="80" :placeholder="t('trainNamePlaceholder')" /></label><footer><button type="button" @click="creating = false">{{ t('cancel') }}</button><button type="submit" :disabled="busy || !trainName.trim()">{{ t('createTrain') }}</button></footer></form></div>
    <div v-if="notice" class="work-notice" role="status">{{ notice }}<button @click="notice = ''"><X :size="16" /></button></div>
  </div>
</template>

<style scoped>
.workbench{flex:1;min-height:0;display:flex;flex-direction:column;background:#191e22;color:#f3f5f8}.workbench-toolbar{height:50px;flex:none;display:flex;align-items:center;justify-content:space-between;padding:0 26px;border-bottom:1px solid #353d45;background:#1d2227}.breadcrumbs,.breadcrumbs button,.toolbar-right{display:flex;align-items:center;gap:14px}.breadcrumbs{font-size:13px;color:#d4dce5}.breadcrumbs button{border:0;background:none;color:#d4dce5;padding:0}.breadcrumbs span{color:#7e8993}.breadcrumbs strong{font-weight:500}.pack-chip,.export-button{height:34px;border:1px solid #3f4953;border-radius:7px;background:#282f36;color:#dce3eb;display:flex;align-items:center;gap:10px;padding:0 12px;font-size:12px}.pack-chip{font-size:11px;letter-spacing:.04em}.export-button{background:#444d59;border-color:#444d59;font-weight:600}.export-button:hover{background:#56616e}.workbench-body{min-height:0;flex:1;display:flex}.workbench-sidebar{width:250px;flex:none;border-right:1px solid #353d45;background:linear-gradient(150deg,#22282e,#1b2024);padding:22px 15px;overflow:auto}.project-identity{display:flex;align-items:center;gap:12px;margin:0 8px 20px;min-width:0}.project-image{width:68px;height:68px;display:grid;place-items:center;flex:none;border:1px solid #58636d;border-radius:7px;background:linear-gradient(135deg,#77899a,#303c47 50%,#151a1e);color:#f2f5f9}.project-identity div:last-child{min-width:0}.project-identity strong,.project-identity span{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.project-identity strong{font-size:13px}.project-identity span{font-size:10px;color:#aeb8c4;margin-top:7px;letter-spacing:.06em}.workbench-sidebar nav{display:flex;flex-direction:column;gap:4px}.work-nav{min-height:42px;width:100%;display:flex;align-items:center;gap:14px;border:0;border-radius:7px;padding:0 13px;text-align:left;background:transparent;color:#d6dde5;font-size:13px;white-space:nowrap}.work-nav:hover{background:#313941}.work-nav.active{background:#3b434d;color:#fff;box-shadow:inset 2px 0 #f0f4f9}.work-nav small,.type-card em{margin-left:auto;font-size:10px;font-style:normal;color:#c0c8d2;background:#343c45;border-radius:20px;padding:4px 8px}.nav-caption{font-size:12px;color:#8995a1;margin:17px 13px 7px}.work-nav-rule{height:1px;background:#3e4750;margin:14px 8px 0}.workbench-main{min-width:0;flex:1;overflow:auto;padding:23px clamp(25px,3vw,42px) 45px}.workspace-heading{display:flex;align-items:center;justify-content:space-between;gap:20px;margin-bottom:21px}.workspace-heading h1{font:600 34px/1.15 Outfit,'Segoe UI',sans-serif;margin:0 0 4px}.workspace-heading p{margin:0;color:#c6ced8;font-size:15px}.new-train{height:39px;display:flex;align-items:center;gap:9px;white-space:nowrap;border:1px solid #f8fbff;border-radius:8px;padding:0 17px;background:linear-gradient(120deg,#f8fbff,#dfe8f6);color:#1e2730;font-weight:700;font-size:13px}.new-train:hover{filter:brightness(.93)}.summary-card{min-height:166px;position:relative;border:1px solid #454e57;border-radius:10px;padding:26px 31px;background:linear-gradient(120deg,#262e35,#1d2429);overflow:hidden}.summary-card:after{content:'';position:absolute;right:-35px;bottom:-100px;width:280px;height:280px;border:1px solid #ffffff0b;border-radius:50%;box-shadow:0 0 0 48px #ffffff05,0 0 0 96px #ffffff03;pointer-events:none}.summary-label{color:#b7c2cd;font-size:10px;letter-spacing:.2em}.summary-card h2{font:600 31px Outfit,'Segoe UI',sans-serif;margin:13px 0 5px}.summary-card p{color:#c0c9d2;font-size:13px;margin:0}.summary-target{display:block;color:#8e9aa6;font-size:10px;letter-spacing:.14em;margin-top:15px}.content-types,.recent-content{margin-top:22px}.content-types h2,.recent-content h2{font:600 18px Outfit,'Segoe UI',sans-serif;margin:0 0 3px}.content-types>p,.recent-content p{color:#aab4be;font-size:12px;margin:0 0 13px}.type-cards{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:12px}.type-card{min-height:90px;border:1px solid #59636d;border-radius:9px;background:linear-gradient(125deg,#283038,#22282e);color:#ecf0f5;display:flex;align-items:center;gap:16px;padding:15px 17px;text-align:left}.type-card:hover{border-color:#bdc9d6}.type-card>svg{flex:none}.type-card span{display:flex;flex-direction:column;gap:7px;min-width:0}.type-card strong{font-size:13px}.type-card small{font-size:11px;line-height:1.4;color:#aeb8c3}.type-card>svg:last-child{margin-left:auto}.planned-card{color:#a7b0ba;border-color:#414a53;background:#20262b}.planned-card em{align-self:flex-start;white-space:nowrap}.recent-heading{display:flex;justify-content:space-between;align-items:start}.recent-heading button{display:flex;align-items:center;gap:8px;border:0;background:none;color:#b9c3ce;font-size:12px}.content-table{border:1px solid #39434c;border-radius:9px;overflow:hidden;background:#1e2529}.content-table-head,.content-row{display:grid;grid-template-columns:minmax(180px,2fr) minmax(100px,1fr) minmax(130px,1fr) 22px;align-items:center;column-gap:12px;padding:0 17px}.content-table-head{height:32px;background:#293138;color:#b3bec9;font-size:10px;letter-spacing:.04em}.content-row{width:100%;min-height:58px;border:0;border-top:1px solid #354049;background:transparent;color:#c7d0da;text-align:left;font-size:12px}.content-row:hover{background:#29323a}.item-name,.item-kind{display:flex;align-items:center;gap:12px}.item-name strong,.item-name small{display:block}.item-name strong{color:#f3f5f8;font-size:13px}.item-name small{color:#9ca8b3;font-size:10px;margin-top:3px}.item-icon{width:39px;height:39px;display:grid;place-items:center;border:1px solid #64717e;border-radius:6px;background:#303d48}.content-empty{min-height:145px;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:7px;color:#9facb8;font-size:12px}.content-empty strong{color:#e8edf2;font-size:13px}.content-search{height:40px;max-width:330px;display:flex;align-items:center;gap:10px;border:1px solid #4b5661;border-radius:8px;padding:0 12px;color:#b9c4ce;margin:12px 0 18px}.content-search input{width:100%;border:0;outline:0;background:transparent;color:#fff;font-size:12px}.placeholder-panel{min-height:250px;border:1px solid #414b54;border-radius:10px;background:#22292f;display:flex;flex-direction:column;align-items:center;justify-content:center;color:#a9b5c0;margin-top:18px;text-align:center;padding:30px}.placeholder-panel h2{color:#f1f4f7;margin:15px 0 3px;font-size:18px}.placeholder-panel p{font-size:13px}.placeholder-panel button{border:1px solid #5c6975;border-radius:6px;background:#35404a;color:#e9eff5;padding:8px 12px}.work-error{border:1px solid #a26363;border-radius:7px;padding:12px;margin-bottom:12px;color:#ffc5c5}.work-error button{margin-left:10px;background:none;border:0;color:inherit;text-decoration:underline}.work-modal-scrim{position:fixed;z-index:20;inset:0;background:#080b0dc9;display:grid;place-items:center}.work-modal{width:min(430px,calc(100vw - 32px));background:#282f36;border:1px solid #5c6874;border-radius:12px;padding:25px;box-shadow:0 20px 80px #0009}.work-modal>div,.work-modal footer{display:flex;align-items:center;justify-content:space-between}.work-modal h2{font:600 23px Outfit,'Segoe UI',sans-serif;margin:0}.work-modal button{border:0;background:none;color:#dce4ec}.work-modal label{display:flex;flex-direction:column;gap:8px;margin-top:28px;font-size:12px}.work-modal input{height:40px;border:1px solid #5c6874;background:#1c2329;color:#fff;border-radius:7px;padding:0 11px;outline:0}.work-modal footer{justify-content:flex-end;gap:10px;margin-top:27px}.work-modal footer button{border:1px solid #5c6874;border-radius:7px;padding:9px 13px}.work-modal footer button:last-child{background:#eaf2fb;border-color:#fff;color:#1b232c;font-weight:700}.work-modal footer button:disabled{opacity:.5}.work-notice{position:fixed;z-index:22;right:20px;bottom:20px;background:#39434d;border:1px solid #6b7885;border-radius:8px;padding:11px 14px;display:flex;align-items:center;gap:14px;font-size:12px}.work-notice button{background:none;border:0;color:#fff}@media(max-width:1100px){.workbench-sidebar{width:205px}.type-cards{grid-template-columns:1fr}.type-card{min-height:75px}}@media(max-width:800px){.workbench-sidebar{width:65px;padding:18px 8px}.project-identity,.nav-caption,.work-nav-rule{display:none}.work-nav{font-size:0;justify-content:center;padding:0}.work-nav small{display:none}.workspace-heading h1{font-size:27px}.pack-chip{display:none}}
</style>

