// Keep UI copy separate from components for future locale support.
const en = {
  home: 'Home', newProject: 'New Project', openProject: 'Open Project', recentProjects: 'Recent Projects', allProjects: 'All Projects',
  welcome: 'Welcome back', subtitle: 'Continue building your MTR resource packs.', newHint: 'Create a new MTR resource pack', openHint: 'Choose an existing project folder',
  search: 'Search projects...', name: 'Project name', lastOpened: 'Last opened', location: 'Location', pack: 'MTR resource pack',
  empty: 'No recent projects yet', emptyHint: 'Create a project or open an existing folder to get started.', noMatches: 'No matching projects',
  drop: 'Open a project folder', dropHint: 'Choose a folder containing an MTR Pack Studio project', tip: 'Tip: Projects are stored locally on your computer.',
  create: 'Create Project', parent: 'Parent folder', parentHint: 'Leave empty for Documents/MTR Pack Studio', cancel: 'Cancel', remove: 'Remove from recent',
  back: 'Back to Home', trains: 'Trains', coming: 'Train editing is the next milestone. Your project is ready to use.',
  settings: 'Settings', help: 'Help', settingsLater: 'Settings are coming in a future release.', helpLater: 'Help is coming in a future release.',
  workspaceEyebrow: 'YOUR CREATIVE WORKSPACE', libraryEyebrow: 'YOUR LIBRARY', recentEyebrow: 'PICK UP WHERE YOU LEFT OFF', packEyebrow: 'MTR 4 RESOURCE PACK',
  gridView: 'Grid view', listView: 'List view', more: 'More options', trySearch: 'Try another name or path.', today: 'Today',
  productTagline: 'Build · Design · For MTRmod', productSubline: 'For a richer railway in Minecraft.', footerCredit: 'Create More Possibilities · With MTRmod',
  newPlaceholder: 'My Train Pack', newFolderHint: 'A new project folder will be created at this location.',
  browse: 'Browse',
  minimize: 'Minimize', maximize: 'Maximize or restore', close: 'Close',
} as const
export type MessageKey = keyof typeof en
export const t = (key: MessageKey) => en[key]
