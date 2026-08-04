import type { YoutubeProxyConnection } from '../main/modules/youtubeProxy/YoutubeProxyConnection'
import type { MediaSessionData, MprisPlaybackState } from '../main/modules/mediaSession/MediaSessionManager'
import type { LoadingStatus } from '../main/modules/appUpdater/AppUpdater'
import type { MediaResponse, AppMediaSession, MediaActionPayload, Keybind, AudioSource } from '../main/napi'
import type { VideoAspectRatio, VideoVolumeState } from '../main/modules/youtubeUtils/YoutubeUtils'
import type { SimplifyDeep } from 'type-fest'

export type AppLanguage = 'en' | 'ru' | null
export type AppLogLevel = 'debug' | 'info' | 'warn' | 'error'

export type IpcError = {
  name: string
  message: string
  [key: string]: unknown
}

export type IpcResult<T> =
  | { ok: true, value: T }
  | { ok: false, error: IpcError }

export type AppSettings = {
  language: AppLanguage
  systemMediaControlsSession: boolean
  extractThumbnails: boolean
  allowCrashReports: boolean
  acrylic: boolean
}

export type AppBootstrapState = { version: string, settings: AppSettings }
export type AppState = AppBootstrapState & {
  platform: NodeJS.Platform
  isMaximized: boolean
  media: MediaResponse
  decorations: string[] | null
}

export type AppDisplay = SimplifyDeep<Electron.Display>
export type WidgetOptions = { mode: 'window' } | { mode: 'overlay', displayId: number }

export interface IpcInvokes {
  'bootstrap:ready': () => void

  'app:log': (level: AppLogLevel, context: string, options: { redact?: string[] }, ...args: unknown[]) => void
  'app:getLogs': () => string
  'app:getDisplays': () => AppDisplay[]
  'app:setLanguage': (lang: AppLanguage) => void
  'app:setAllowCrashReports': (isAllowed: boolean) => void
  'app:relaunch': () => void
  'app:quit': () => void
  'app:openWindowsAppsVolume': () => void
  'app:sendReport': (userId: string, userMessage: string) => void

  'window:minimize': () => void
  'window:unmaximize': () => void
  'window:maximize': () => void
  'window:setAcrylic': (enable: boolean) => void

  'widget:create': (url: string, options: WidgetOptions) => number
  'widget:remove': (id: number) => void
  'widget:destroyAll': () => void

  'youtube:setVideoVolume': (vol: number) => VideoVolumeState
  'youtube:getAspectRatio': () => VideoAspectRatio
  'youtube:setYoutubePiPMode': (isActive: boolean, isOnTop: boolean) => void
  'youtube:setYoutubePiPAlwaysOnTopMode': (isOnTop: boolean) => void

  'proxy:setYoutubeConnectionMethod': (connection: YoutubeProxyConnection[keyof YoutubeProxyConnection]) => void

  'input:setKeybinds': (keybinds: Keybind[]) => void

  'media:setSystemMediaControlsSession': (isActive: boolean) => void
  'media:setExtractThumbnails': (isActive: boolean) => void
  'media:setAppSource': (source: string) => void
  'media:updateMediaSession': (data: MediaSessionData | null) => void
  'media:pause': (apps: string[]) => AppMediaSession[]
  'media:resume': (apps: string[]) => void
  'media:setVolume': (app: string, voleme: number) => void
  'media:getAudioSources': () => AudioSource[]

  'mpris:updateMetadata': (data: MediaSessionData | null) => void
  'mpris:updatePlaybackState': (state: MprisPlaybackState) => void
}

export interface IpcEvents {
  'app:auth': (code: string) => void
  'bootstrap:status': (status: LoadingStatus) => void
  'bootstrap:progress': (progress: number) => void
  'window:updateIsMaximized': (isMaximized: boolean) => void
  'media:sessionsChanged': (sessions: AppMediaSession[]) => void
  'media:sourcesChanged': (sources: AudioSource[]) => void
  'media:appAudioSourceChanged': (source: AudioSource | null) => void
  'input:keybindPressed': (action: string) => void
  'media:action': (action: MediaActionPayload) => void
}

export interface IpcSyncs {
  'bootstrap:init': () => AppBootstrapState
  'app:init': () => AppState
  'app:getLocale': () => string
}

export interface IpcRendererSends {
  'widget:resize': (width: number, height: number) => void
}
