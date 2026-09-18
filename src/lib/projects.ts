import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

export interface ProjectSummary { name: string; path: string; lastOpened: number }
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
