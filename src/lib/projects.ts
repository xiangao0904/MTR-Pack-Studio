import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import { t } from '../i18n'
import { bindPreviewTextures } from './preview-glb'

export interface ProjectSummary { name: string; path: string; lastOpened: number }
export interface ContentEntry { id: string; kind: string; name: string; file: string; updatedAt: number; resources?: string[] }
export interface ProjectData { name: string; namespace: string; description: string; content: ContentEntry[]; path: string; recovered: boolean }
export type PlacementPreset = 'all' | 'first' | 'last' | 'odd' | 'even' | 'every' | 'custom'
export interface CarPlacementRule { preset: PlacementPreset; every?: number; offset: number; whitelist: string; blacklist: string }
export interface EndConfiguration { gangway: boolean; barrier: boolean }
export interface MaterialBinding { materialId: string; textureAssetId?: string }
export interface ModelLayer { id: string; name: string; assetId: string; flipTextureV: boolean; visible: boolean; materialBindings: MaterialBinding[]; partRules: Record<string, CarPlacementRule> }
export interface CarriageDefinition { thumbnailHash?: string; id: string; exportId: string; name: string; length: number; width: number; bogie1Position: number; bogie2Position: number; couplingPadding1: number; couplingPadding2: number; end1: EndConfiguration; end2: EndConfiguration; placement: CarPlacementRule; bodyModels: ModelLayer[]; bogie1Models: ModelLayer[]; bogie2Models: ModelLayer[] }
export interface PreviewCarriage { carriageId: string; reversed: boolean }
export interface TrainDefinition { id: string; revision: number; exportId: string; name: string; description: string; color: string; tags: string[]; mtr3BaseTrainType: string; carriages: CarriageDefinition[]; previewConsist: PreviewCarriage[] }
export type ModelFormat = 'obj' | 'fbx' | 'mqo'
export interface ModelPartSummary { id: string; name: string; triangleCount: number }
export interface ModelMaterial { id: string; name: string; color: [number, number, number, number]; texture?: string }
export interface AssetDefinition { materials: ModelMaterial[]; id: string; name: string; sourceFormat: ModelFormat; sourceHash: string; documentHash: string; previewHash: string; dependencies: { name: string; hash: string; mediaType: string }[]; parts: ModelPartSummary[]; warnings: string[] }
export interface ImportAnalysis { format: ModelFormat; missingDependencies: string[]; parts: ModelPartSummary[]; warnings: string[] }
export interface ExportOptions { target: 'mtr4' | 'mtr3_nte'; minecraftVersion: string; modelFormat: 'obj' | 'mqo' }
export interface ValidationIssue { severity: 'error' | 'warning'; message: string; trainId?: string; carriageId?: string; layerId?: string | null; field?: string }
export interface ExportReport { path: string; fileCount: number; warnings: ValidationIssue[] }

let browserActivePath = ''
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
  browserActivePath = path
  if (inTauri()) {
    const project = await invoke<ProjectData | null>('get_active_project')
    if (!project) throw new Error('No project is open.')
    return project
  }
  const stored = localStorage.getItem(dataKey(path))
  if (!stored) throw new Error('The browser preview project could not be found.')
  const project = JSON.parse(stored) as ProjectData; project.namespace ||= slug(project.name, 'mtr_pack'); return project
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
  if (!inTauri()) return 'preview://studio-train'
  return open({ directory: false, multiple: false, title: 'Import Model', filters: [{ name: '3D Models', extensions: ['obj', 'fbx', 'mqo'] }] })
}

export async function chooseModelDependency(name: string): Promise<string | null> {
  if (!inTauri()) return null
  const extension = name.split('.').pop() || ''
  return open({ directory: false, multiple: false, title: `${t('locateDependency')} ${name}`, filters: extension ? [{ name, extensions: [extension] }] : undefined })
}

export async function analyzeModelImport(path: string, dependencyOverrides: Record<string, string> = {}): Promise<ImportAnalysis> {
  if (!inTauri()) { const asset = await getModelAsset('fixture-studio-train'); return {format: 'obj', missingDependencies: [], parts: asset.parts, warnings: [t('browserDemoModels')]} }
  return invoke<ImportAnalysis>('analyze_model_import', { path, dependencyOverrides })
}

export async function importModel(trainId: string, carriageId: string, slot: 'body' | 'bogie1' | 'bogie2', path: string, dependencyOverrides: Record<string, string> = {}, expectedRevision?: number): Promise<{ train: TrainDefinition; asset: AssetDefinition }> {
  if (!inTauri()) {
    const asset = await getModelAsset(slot === 'body' ? 'fixture-studio-train' : 'fixture-studio-bogie')
    const train = await getTrain(browserActivePath, trainId); const carriage = train.carriages.find(car => car.id === carriageId)
    if (!carriage) throw new Error(t('noCarriage'))
    const layers = slot === 'body' ? carriage.bodyModels : slot === 'bogie1' ? carriage.bogie1Models : carriage.bogie2Models
    layers.push({id: crypto.randomUUID(), name: asset.name, assetId: asset.id, visible: true, flipTextureV: false, materialBindings: [], partRules: {}})
    return { train: await updateTrain(browserActivePath, train, expectedRevision ?? train.revision), asset }
  }
  const result = await invoke<{train: TrainDefinition; asset: AssetDefinition}>('import_model', { trainId, carriageId, slot, path, dependencyOverrides, expectedRevision }); result.asset = await getModelAsset(result.asset.id); return result
}

export async function getModelAsset(assetId: string): Promise<AssetDefinition> {
  if (!inTauri()) { const name = assetId === 'fixture-studio-train' ? 'studio-train' : assetId === 'fixture-studio-bogie' ? 'studio-bogie' : null; if (!name) throw new Error(t('missingModel')); const asset: AssetDefinition = await (await fetch(`/fixtures/${name}.json`)).json(); asset.warnings = [t('browserDemoModels')]; return asset }
  const [asset, materials] = await Promise.all([invoke<AssetDefinition>('get_model_asset', { assetId }), invoke<ModelMaterial[]>('get_model_materials', { assetId })]); return { ...asset, materials }
}

export async function getModelPreview(assetId: string, materialBindings: MaterialBinding[] = []): Promise<ArrayBuffer> {
  if (!inTauri()) {
    const asset = await getModelAsset(assetId); const name = asset.id === 'fixture-studio-train' ? 'studio-train' : 'studio-bogie'
    const source = await (await fetch(`/fixtures/${name}.glb`)).arrayBuffer()
    const replacements = await Promise.all(materialBindings.filter(binding => binding.textureAssetId).map(async binding => ({ materialId: binding.materialId, bytes: await getImageAsset(binding.textureAssetId!) })))
    return bindPreviewTextures(source, replacements)
  }
  return invoke<ArrayBuffer>('get_model_preview', { assetId, materialBindings })
}

export async function updateProjectSettings(path: string, namespace: string, description: string): Promise<ProjectData> {
  if (inTauri()) return invoke<ProjectData>('update_project_settings', { namespace, description })
  const project = await getProject(path); project.namespace = namespace; project.description = description; localStorage.setItem(dataKey(path), JSON.stringify(project)); return project
}

export async function validateExport(options: ExportOptions): Promise<ValidationIssue[]> {
  if (inTauri()) return invoke<ValidationIssue[]>('validate_export', { options })
  return [{ severity: 'warning', message: t('browserExportWarning') }]
}

export async function chooseExportPath(projectName: string, target: ExportOptions['target']): Promise<string | null> {
  if (!inTauri()) return null
  const suffix = target === 'mtr4' ? 'MTR4' : 'MTR3-NTE'
  return save({ title: t('exportDialogTitle'), defaultPath: `${projectName.replace(/[<>:"/\\|?*\u0000-\u001f]/g, '_')} ${suffix}.zip`, filters: [{ name: t('resourcePack'), extensions: ['zip'] }] })
}

export async function exportResourcePack(path: string, options: ExportOptions): Promise<ExportReport> {
  if (!inTauri()) throw new Error('Resource pack export requires the desktop app.')
  return invoke<ExportReport>('export_resource_pack', { path, options })
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

export async function getProjectCover(path: string): Promise<ArrayBuffer | null> {
  if (inTauri()) return invoke<ArrayBuffer>('get_project_cover', { path })
  const value = localStorage.getItem(`${dataKey(path)}:cover`)
  return value ? (await fetch(value)).arrayBuffer() : null
}
export async function setProjectCover(path: string, file: File | null): Promise<void> {
  if (file && file.size > 32 * 1024 * 1024) throw new Error(t('imageTooLarge'))
  let bytes: ArrayBuffer | null = null
  if (file) {
    const bitmap = await createImageBitmap(file)
    const canvas = document.createElement('canvas'); const ratio = Math.min(1, 512 / Math.max(bitmap.width, bitmap.height))
    canvas.width = Math.max(1, Math.round(bitmap.width * ratio)); canvas.height = Math.max(1, Math.round(bitmap.height * ratio))
    canvas.getContext('2d')!.drawImage(bitmap, 0, 0, canvas.width, canvas.height); bitmap.close()
    const blob = await new Promise<Blob>((resolve, reject) => canvas.toBlob(blob => blob ? resolve(blob) : reject(new Error(t('imageReadFailed'))), 'image/png'))
    bytes = await blob.arrayBuffer()
    if (!inTauri()) localStorage.setItem(`${dataKey(path)}:cover`, canvas.toDataURL('image/png'))
  }
  if (inTauri()) await invoke('set_project_cover', { bytes: bytes ? Array.from(new Uint8Array(bytes)) : null })
  else if (!file) localStorage.removeItem(`${dataKey(path)}:cover`)
}

export async function chooseTextureFile(): Promise<string | null> {
  if (!inTauri()) {
    const file = await new Promise<File | null>(resolve => { const input = document.createElement('input'); input.type = 'file'; input.accept = 'image/png,image/jpeg,image/webp'; input.onchange = () => resolve(input.files?.[0] || null); input.oncancel = () => resolve(null); input.click() })
    if (!file) return null
    if (file.size > 32*1024*1024) throw new Error(t('imageTooLarge'))
    const bitmap = await createImageBitmap(file); const canvas = document.createElement('canvas'); canvas.width = bitmap.width; canvas.height = bitmap.height
    canvas.getContext('2d')!.drawImage(bitmap,0,0); bitmap.close()
    const blob = await new Promise<Blob>((resolve,reject) => canvas.toBlob(value => value ? resolve(value) : reject(new Error(t('imageReadFailed'))),'image/png'))
    return `preview-image:${await storeImageBytes(new Uint8Array(await blob.arrayBuffer()))}`
  }
  return open({ directory: false, multiple: false, title: t('chooseTexture'), filters: [{ name: t('imageFiles'), extensions: ['png','jpg','jpeg','webp'] }] })
}
export async function importTextureFile(path: string): Promise<string> {
  if (!inTauri() && path.startsWith('preview-image:')) return path.slice('preview-image:'.length)
  return invoke<string>('import_texture_file', { path })
}
export async function storeImageBytes(bytes: Uint8Array): Promise<string> {
  if (!inTauri()) {
    const hash = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', new Uint8Array(bytes)))).map(v => v.toString(16).padStart(2, '0')).join('')
    const data = await new Promise<string>(resolve => { const reader = new FileReader(); reader.onload = () => resolve(String(reader.result)); reader.readAsDataURL(new Blob([new Uint8Array(bytes)], { type: 'image/png' })) })
    localStorage.setItem(`mtr-pack-studio:image:${hash}`, data); return hash
  }
  return invoke<string>('store_image_bytes', { bytes: Array.from(bytes) })
}
export async function getImageAsset(hash: string): Promise<ArrayBuffer> {
  if (inTauri()) return invoke<ArrayBuffer>('get_image_asset', { hash })
  const data = localStorage.getItem(`mtr-pack-studio:image:${hash}`); if (!data) throw new Error(t('imageReadFailed'))
  return (await fetch(data)).arrayBuffer()
}
