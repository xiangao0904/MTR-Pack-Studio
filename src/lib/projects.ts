import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

export interface ProjectSummary { name: string; path: string; lastOpened: number }
export interface ContentEntry { id: string; kind: string; name: string; file: string; updatedAt: number }
export interface ProjectData { name: string; target: string; description: string; content: ContentEntry[] }
const key = 'mtr-pack-studio:recent-projects'
const inTauri = () => '__TAURI_INTERNALS__' in window
function browserProjects(): ProjectSummary[] {
  try { return JSON.parse(localStorage.getItem(key) || '[]') as ProjectSummary[] } catch { return [] }
}
function rememberBrowser(project: ProjectSummary) {
  localStorage.setItem(key, JSON.stringify([project, ...browserProjects().filter(p => p.path !== project.path)]))
}
export async function loadRecentProjects(): Promise<ProjectSummary[]> {
  return inTauri() ? invoke<ProjectSummary[]>('list_recent_projects') : browserProjects()
}
export async function createProject(name: string, parent: string): Promise<ProjectSummary> {
  if (inTauri()) return invoke<ProjectSummary>('create_project', { name, parent: parent || null })
  const project = { name, path: `${parent || 'Documents/MTR Pack Studio'}/${name}`, lastOpened: Date.now() }
  rememberBrowser(project)
  return project
}
export async function chooseParentDirectory(): Promise<string | null> {
  if (!inTauri()) throw new Error('Choosing local folders requires the desktop app.')
  return open({ directory: true, multiple: false, title: 'Choose a parent folder' })
}
export async function openProject(path?: string): Promise<ProjectSummary | null> {
  if (inTauri()) {
    const selected = path ?? await open({ directory: true, multiple: false, title: 'Open MTR Pack Studio Project' })
    return selected ? invoke<ProjectSummary>('open_project', { path: selected }) : null
  }
  const project = browserProjects().find(p => p.path === path)
  if (project) { const updated = { ...project, lastOpened: Date.now() }; rememberBrowser(updated); return updated }
  throw new Error('Opening local folders requires the desktop app.')
}
export async function removeRecentProject(path: string): Promise<void> {
  if (inTauri()) return invoke('remove_recent_project', { path })
  localStorage.setItem(key, JSON.stringify(browserProjects().filter(p => p.path !== path)))
}

const dataKey = (path: string) => `mtr-pack-studio:project:${path}`
export async function getProject(path: string): Promise<ProjectData> {
  if (inTauri()) return invoke<ProjectData>('get_project', { path })
  const stored = localStorage.getItem(dataKey(path))
  return stored ? JSON.parse(stored) as ProjectData : { name: browserProjects().find(p => p.path === path)?.name || '', target: 'mtr4', description: '', content: [] }
}
export async function createTrain(path: string, name: string): Promise<ContentEntry> {
  if (inTauri()) return invoke<ContentEntry>('create_train', { path, name })
  const project = await getProject(path)
  const id = `train-${Date.now()}`
  const entry = { id, kind: 'train', name, file: `content/trains/${id}.json`, updatedAt: Date.now() }
  project.content.push(entry)
  localStorage.setItem(dataKey(path), JSON.stringify(project))
  return entry
}
