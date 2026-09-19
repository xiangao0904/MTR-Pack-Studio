import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'

export interface ProjectSummary { name: string; path: string; lastOpened: number }
export interface ContentEntry { id: string; kind: string; name: string; file: string; updatedAt: number; resources?: string[] }
export interface ProjectData { name: string; target: string; description: string; content: ContentEntry[]; path: string; recovered: boolean }

const recentKey = 'mtr-pack-studio:recent-projects'
const inTauri = () => '__TAURI_INTERNALS__' in window
const dataKey = (path: string) => `mtr-pack-studio:project:${path}`

function browserProjects(): ProjectSummary[] {
  try { return JSON.parse(localStorage.getItem(recentKey) || '[]') as ProjectSummary[] } catch { return [] }
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
  if (inTauri()) return summaryFromData(await invoke<ProjectData>('create_project', { path: resolvedPath, name, target: 'mtr4' }))
  const project: ProjectData = { name, path: resolvedPath, target: 'mtr4', description: '', content: [], recovered: false }
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

export async function createTrain(path: string, name: string): Promise<ContentEntry> {
  if (inTauri()) return invoke<ContentEntry>('create_train', { name })
  const project = await getProject(path)
  const id = `train-${Date.now()}`
  const entry = { id, kind: 'train', name, file: `content/trains/${id}.json`, updatedAt: Date.now(), resources: [] }
  project.content.push(entry)
  localStorage.setItem(dataKey(path), JSON.stringify(project))
  return entry
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
