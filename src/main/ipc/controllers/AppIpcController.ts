import { app, screen, shell } from 'electron'
import isDev from 'electron-is-dev'
import { BaseIpcController } from '../BaseIpcController'
import { settings } from '../../settings'
import { native } from '../../napi'
import type { MainWindow } from '../../app/Main'
import { logger } from '../../utils/Logger'
import * as Sentry from '@sentry/electron/main'
import os from 'os'

export class AppIpcController extends BaseIpcController {
  constructor(private mainWindow: MainWindow) {
    super()

    this.defineNativeEvent('mediaSessionsChanged', sessions => {
      this.log.info('Media sessions changed', sessions)
      if (this.mainWindow.window.isDestroyed()) return
      this.mainWindow.window.webContents.send('media:sessionsChanged', sessions)
    })
    this.defineNativeEvent('keybindPressed', action => {
      if (this.mainWindow.window.isDestroyed()) return
      this.mainWindow.window.webContents.send('input:keybindPressed', action)
    })
    this.defineNativeEvent('sourcesChanged', sources => {
      this.log.info('Sources changed', sources)
      if (this.mainWindow.window.isDestroyed()) return
      this.mainWindow.window.webContents.send('media:sourcesChanged', sources)
    })
    this.defineNativeEvent('appAudioSourceChanged', src => {
      this.log.info('Audio source changed', src)
      if (this.mainWindow.window.isDestroyed()) return
      this.mainWindow.window.webContents.send('media:appAudioSourceChanged', src)
    })

    this.defineSync('app:init', () => ({
      version: app.getVersion(),
      platform: os.platform(),
      isMaximized: this.mainWindow.window.isMaximized(),
      settings: settings.data,
      media: native.media.getState(),
      decorations: native.window.getDecorations()
    }))
    this.defineSync('app:getLocale', () => app.getLocale())

    this.defineInvoke('app:log', (_, level: string, context: string, options: { redact?: string[] }, ...args: unknown[]) => {
      logger.logFromFrontend(level, context, options, ...args)
    })
    this.defineInvoke('app:getLogs', () => logger.getLogs())
    this.defineInvoke('app:getDisplays', () => screen.getAllDisplays())
    this.defineInvoke('app:setLanguage', (_, lang) => settings.set('language', lang))
    this.defineInvoke('app:setAllowCrashReports', (_, isAllowed) => settings.set('allowCrashReports', isAllowed))
    this.defineInvoke('app:openWindowsAppsVolume', () => shell.openExternal('ms-settings:apps-volume'))
    this.defineInvoke('app:sendReport', (_, userId: string, userMessage) => {
      this.log.info('User initiated manual bug report')

      Sentry.captureMessage(`User Feedback: ${userMessage}`, {
        level: 'info',
        tags: { manual_report: 'true' },
        user: { id: userId }
      })
    })
    this.defineInvoke('app:relaunch', () => {
      if (!isDev) app.relaunch()
      app.exit()
    })
    this.defineInvoke('app:quit', () => app.quit())
  }

  private log = logger.child('AppIpcController', {
    redact: [
      '[*].title', '[*].artist', '[*].url', '[*].art', 'title', 'artist', 'url', 'art'
    ]
  })
}
