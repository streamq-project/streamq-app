import { contextBridge } from 'electron'
import { createInvoker, createSubscriber, sendSync } from './utils'

const api = {
  app: {
    init: () => sendSync('app:init'),
    setLanguage: createInvoker('app:setLanguage'),
    setAllowCrashReports: createInvoker('app:setAllowCrashReports'),
    log: createInvoker('app:log'),
    getLogs: createInvoker('app:getLogs'),
    sendReport: createInvoker('app:sendReport'),
    relaunch: createInvoker('app:relaunch'),
    quit: createInvoker('app:quit'),
    openWindowsAppsVolume: createInvoker('app:openWindowsAppsVolume'),
    getLocale: () => sendSync('app:getLocale'),
    getDisplays: createInvoker('app:getDisplays'),
    onAuth: createSubscriber('app:auth')
  },
  window: {
    minimize: createInvoker('window:minimize'),
    unmaximize: createInvoker('window:unmaximize'),
    maximize: createInvoker('window:maximize'),
    setAcrylic: createInvoker('window:setAcrylic'),
    onUpdateIsMaximized: createSubscriber('window:updateIsMaximized')
  },
  widgets: {
    create: createInvoker('widget:create'),
    remove: createInvoker('widget:remove'),
    destroyAll: createInvoker('widget:destroyAll')
  },
  youtube: {
    setVideoVolume: createInvoker('youtube:setVideoVolume'),
    getAspectRatio: createInvoker('youtube:getAspectRatio'),
    setYoutubePiPMode: createInvoker('youtube:setYoutubePiPMode'),
    setYoutubePiPAlwaysOnTopMode: createInvoker('youtube:setYoutubePiPAlwaysOnTopMode')
  },
  proxy: {
    setYoutubeConnectionMethod: createInvoker('proxy:setYoutubeConnectionMethod')
  },
  media: {
    setSystemMediaControlsSession: createInvoker('media:setSystemMediaControlsSession'),
    setExtractThumbnails: createInvoker('media:setExtractThumbnails'),
    setAppSource: createInvoker('media:setAppSource'),
    updateMediaSession: createInvoker('media:updateMediaSession'),
    pause: createInvoker('media:pause'),
    resume: createInvoker('media:resume'),
    setVolume: createInvoker('media:setVolume'),
    getAudioSources: createInvoker('media:getAudioSources'),
    onMediaSessionsChanged: createSubscriber('media:sessionsChanged'),
    onSourcesChanged: createSubscriber('media:sourcesChanged'),
    onAppAudioSourceChanged: createSubscriber('media:appAudioSourceChanged'),
    onMediaAction: createSubscriber('media:action')
  },
  mpris: {
    updateMetadata: createInvoker('mpris:updateMetadata'),
    updatePlaybackState: createInvoker('mpris:updatePlaybackState')
  },
  input: {
    setKeybinds: createInvoker('input:setKeybinds'),
    onKeybindPressed: createSubscriber('input:keybindPressed')
  }
}

contextBridge.exposeInMainWorld('appAPI', api)

export type AppAPI = typeof api
