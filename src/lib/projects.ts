import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'

export interface ProjectSummary { name: string; path: string; lastOpened: number }
export interface ContentEntry { id: string; kind: string; name: string; file: string; updatedAt: number; resources?: string[] }
export interface ProjectData { name: string; namespace: string; description: string; content: ContentEntry[]; path: string; recovered: boolean }
export type PlacementPreset = 'all' | 'first' | 'last' | 'odd' | 'even' | 'every' | 'custom'
export interface CarPlacementRule { preset: PlacementPreset; every?: number; offset: number; whitelist: string; blacklist: string }
export interface EndConfiguration { gangway: boolean; barrier: boolean }
export interface MaterialBinding { materialId: string; textureAssetId?: string }
export interface ModelLayer { id: string; name: string; assetId: string; flipTextureV: boolean; visible: boolean; materialBindings: MaterialBinding[]; partRules: Record<string, CarPlacementRule> }
export interface CarriageDefinition { id: string; exportId: string; name: string; length: number; width: number; bogie1Position: number; bogie2Position: number; couplingPadding1: number; couplingPadding2: number; end1: EndConfiguration; end2: EndConfiguration; placement: CarPlacementRule; bodyModels: ModelLayer[]; bogie1Models: ModelLayer[]; bogie2Models: ModelLayer[] }
export interface PreviewCarriage { carriageId: string; reversed: boolean }
export interface TrainDefinition { id: string; revision: number; exportId: string; name: string; description: string; color: string; tags: string[]; mtr3BaseTrainType: string; carriages: CarriageDefinition[]; previewConsist: PreviewCarriage[] }
export type ModelFormat = 'obj' | 'fbx' | 'mqo'
export interface ModelPartSummary { id: string; name: string; triangleCount: number }
export interface AssetDefinition { id: string; name: string; sourceFormat: ModelFormat; sourceHash: string; documentHash: string; previewHash: string; dependencies: { name: string; hash: string; mediaType: string }[]; parts: ModelPartSummary[]; warnings: string[] }
export interface ImportAnalysis { format: ModelFormat; missingDependencies: string[]; parts: ModelPartSummary[]; warnings: string[] }

const recentKey = 'mtr-pack-studio:recent-projects'
const inTauri = () => '__TAURI_INTERNALS__' in window
const dataKey = (path: string) => `mtr-pack-studio:project:${path}`

function browserProjects(): ProjectSummary[] {
  try { return (JSON.parse(localStorage.getItem(recentKey) || '[]') as ProjectSummary[]).filter(project => project.path.toLowerCase().endsWith('.mtrpack')) } catch { return [] }
}
function rememberBrowser(project: ProjectSummary) {
  localStorage.setItem(recentKey, JSON.stringify([project, ...browserProjects().filter(item => item.path !== project.path)]))
}
function ensureExtension(path: string) { return path.toLowerCase().endsWith('.mtrpack') ? path : `${path}.mtrpack` }
function fileNameFor(name: string) { return `${name.replace(/[<>:"/\\|?*\u0000-\u001f]/g, '_').replace(/[. ]+$/g, '') || 'Untitled Project'}.mtrpack` }
function summaryFromData(project: ProjectData): ProjectSummary { return { name: project.name, path: project.path, lastOpened: Date.now() } }

export async function loadRecentProjects(): Promise<ProjectSummary[]> {
  return inTauri() ? invoke<ProjectSummary[]>('list_recent_projects') : browserProjects()
}

export async function chooseProjectSavePath(name: string): Promise<string | null> {
  if (!inTauri()) return `Browser Preview/${fileNameFor(name)}`
  return save({ title: 'Create MTR Pack Studio Project', defaultPath: fileNameFor(name), filters: [{ name: 'MTR Pack Studio Project', extensions: ['mtrpack'] }] })
}

export async function createProject(name: string, path: string): Promise<ProjectSummary> {
  const resolvedPath = ensureExtension(path)
  if (inTauri()) return summaryFromData(await invoke<ProjectData>('create_project', { path: resolvedPath, name }))
  const project: ProjectData = { name, path: resolvedPath, namespace: slug(name, 'mtr_pack'), description: '', content: [], recovered: false }
  localStorage.setItem(dataKey(resolvedPath), JSON.stringify(project))
  const summary = summaryFromData(project)
  rememberBrowser(summary)
  return summary
}

export async function openProject(path?: string): Promise<ProjectSummary | null> {
  if (inTauri()) {
    const selected = path ?? await open({ directory: false, multiple: false, title: 'Open MTR Pack Studio Project', filters: [{ name: 'MTR Pack Studio Project', extensions: ['mtrpack'] }] })
    return selected ? summaryFromData(await invoke<ProjectData>('open_project', { path: selected })) : null
  }
  const project = browserProjects().find(item => item.path === path)
  if (project) { const updated = { ...project, lastOpened: Date.now() }; rememberBrowser(updated); return updated }
  throw new Error('Opening local project files requires the desktop app.')
}

export async function removeRecentProject(path: string): Promise<void> {
  if (inTauri()) return invoke('remove_recent_project', { path })
  localStorage.setItem(recentKey, JSON.stringify(browserProjects().filter(project => project.path !== path)))
}

export async function getProject(path: string): Promise<ProjectData> {
  if (inTauri()) {
    const project = await invoke<ProjectData | null>('get_active_project')
    if (!project) throw new Error('No project is open.')
    return project
  }
  const stored = localStorage.getItem(dataKey(path))
  if (!stored) throw new Error('The browser preview project could not be found.')
  return JSON.parse(stored) as ProjectData
}

function slug(value: string, fallback: string) { return value.toLowerCase().replace(/[^a-z0-9_.-]+/g, '_').replace(/^_+|_+$/g, '') || fallback }
function emptyRule(): CarPlacementRule { return { preset: 'all', offset: 0, whitelist: '', blacklist: '' } }
function browserTrain(name: string, exportId: string): TrainDefinition {
  const id = crypto.randomUUID(); const carriageId = crypto.randomUUID()
  return { id, revision: 1, exportId, name, description: '', color: 'FFFFFF', tags: [], mtr3BaseTrainType: '', previewConsist: [{ carriageId, reversed: false }], carriages: [{ id: carriageId, exportId: 'carriage', name: 'Carriage', length: 20, width: 3, bogie1Position: 7, bogie2Position: -7, couplingPadding1: 0, couplingPadding2: 0, end1: { gangway: false, barrier: false }, end2: { gangway: false, barrier: false }, placement: emptyRule(), bodyModels: [], bogie1Models: [], bogie2Models: [] }] }
}

export async function createTrain(path: string, name: string, exportId = slug(name, 'train')): Promise<ContentEntry> {
  if (inTauri()) return invoke<ContentEntry>('create_train', { name, exportId })
  const project = await getProject(path)
  const train = browserTrain(name, exportId); const id = train.id
  const entry = { id, kind: 'train', name, file: `content/trains/${id}.json`, updatedAt: Date.now(), resources: [] }
  project.content.push(entry)
  localStorage.setItem(dataKey(path), JSON.stringify(project))
  localStorage.setItem(`${dataKey(path)}:train:${id}`, JSON.stringify(train))
  return entry
}

export async function getTrain(path: string, trainId: string): Promise<TrainDefinition> {
  if (inTauri()) return invoke<TrainDefinition>('get_train', { trainId })
  const value = localStorage.getItem(`${dataKey(path)}:train:${trainId}`)
  if (!value) throw new Error('The browser preview train could not be found.')
  return JSON.parse(value) as TrainDefinition
}

export async function updateTrain(path: string, train: TrainDefinition, expectedRevision: number): Promise<TrainDefinition> {
  if (inTauri()) return invoke<TrainDefinition>('update_train', { train, expectedRevision })
  const project = await getProject(path); const current = await getTrain(path, train.id)
  if (current.revision !== expectedRevision) throw new Error('This train changed since it was opened. Reload it before saving again.')
  const updated = { ...train, revision: expectedRevision + 1 }; const entry = project.content.find(item => item.id === train.id)
  if (entry) { entry.name = updated.name; entry.updatedAt = Date.now() }
  localStorage.setItem(dataKey(path), JSON.stringify(project)); localStorage.setItem(`${dataKey(path)}:train:${train.id}`, JSON.stringify(updated))
  return updated
}

export async function deleteTrain(path: string, trainId: string): Promise<void> {
  if (inTauri()) return invoke('delete_train', { trainId })
  const project = await getProject(path); project.content = project.content.filter(item => item.id !== trainId)
  localStorage.setItem(dataKey(path), JSON.stringify(project)); localStorage.removeItem(`${dataKey(path)}:train:${trainId}`)
}

export async function chooseModelFile(): Promise<string | null> {
  if (!inTauri()) return null
  return open({ directory: false, multiple: false, title: 'Import Model', filters: [{ name: '3D Models', extensions: ['obj', 'fbx', 'mqo'] }] })
}

export async function analyzeModelImport(path: string): Promise<ImportAnalysis> {
  if (!inTauri()) throw new Error('Model import requires the desktop app.')
  return invoke<ImportAnalysis>('analyze_model_import', { path })
}

export async function importModel(trainId: string, carriageId: string, slot: 'body' | 'bogie1' | 'bogie2', path: string, dependencyOverrides: Record<string, string> = {}): Promise<{ train: TrainDefinition; asset: AssetDefinition }> {
  if (!inTauri()) throw new Error('Model import requires the desktop app.')
  return invoke('import_model', { trainId, carriageId, slot, path, dependencyOverrides })
}

export async function getModelAsset(assetId: string): Promise<AssetDefinition> {
  if (!inTauri()) throw new Error('Model assets require the desktop app.')
  return invoke<AssetDefinition>('get_model_asset', { assetId })
}

export async function getModelPreview(assetId: string): Promise<ArrayBuffer> {
  if (!inTauri()) throw new Error('Model preview requires the desktop app.')
  return invoke<ArrayBuffer>('get_model_preview', { assetId })
}

export async function saveProject(): Promise<void> {
  if (inTauri()) await invoke('save_project')
}

export async function closeProject(): Promise<void> {
  if (inTauri()) await invoke('close_project')
}

export async function takePendingProjectPath(): Promise<string | null> {
  return inTauri() ? invoke<string | null>('take_pending_project_path') : null
}
