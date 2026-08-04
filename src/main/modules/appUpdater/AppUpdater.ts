import { AppImageUpdater, NsisUpdater } from 'electron-updater'
import { config } from '../../config'
import { app, dialog, shell } from 'electron'
import { logger } from '../../utils/Logger'
import os from 'os'

export type LoadingStatus = 'checking-for-update' | 'downloading' | 'updating' | 'starting'

type UpdaterEvents = {
  onStatus: (status: LoadingStatus) => void
  onProgress: (percent: number) => void
}

export class AppUpdater {
  private log = logger.child('Updater')
  private updater: AppImageUpdater | NsisUpdater | null = null

  constructor(private readonly events: UpdaterEvents) {
    if (!config.updatesUrl) return
    const ElectronUpdater = os.platform() === 'linux' ? AppImageUpdater : NsisUpdater
    this.updater = new ElectronUpdater({ provider: 'generic', url: config.updatesUrl })
    this.updater.logger = this.log
  }

  run(): Promise<void> {
    return new Promise<void>((res, rej) => {
      if (process.argv.includes('--skip-update')) {
        this.log.info('Skipped due to --skip-update flag')
        return res()
      }

      if (!this.updater) {
        this.log.info('Skipped: updatesUrl not configured')
        return res()
      }

      this.events.onStatus('checking-for-update')
      this.updater.on('error', async err => {
        if (/net::ERR_/.test(err.message)) {
          this.log.warn('Network issue. Retrying in 5 seconds...', err)
          setTimeout(() => this.checkForUpdates()?.catch(e => this.log.error(e)), 5000)
        } else {
          this.log.reportError('Update failed', err)
          this.updater?.removeAllListeners()
          const { response } = await dialog.showMessageBox({
            type: 'error',
            title: 'Update Error',
            message: 'Failed to update StreamQ',
            detail: 'An error occurred while checking for or downloading the update. Please download and install the latest version manually.',
            buttons: ['Go to Download', 'Quit'],
            defaultId: 0,
            cancelId: 1
          })

          if (response === 0) await shell.openExternal('https://streamq.io/download')
          app.quit()
        }
      })
      this.updater.on('update-available', () => this.events.onStatus('downloading'))
      this.updater.on('download-progress', p => {
        this.log.info('Download progress', p)
        this.events.onProgress(p.percent)
      })
      this.updater.on('update-cancelled', rej)
      this.updater.on('update-not-available', () => res())
      this.updater.on('update-downloaded', () => {
        this.events.onStatus('updating')
        this.updater?.quitAndInstall(true, true)
      })
      if (!app.isPackaged) return res()
      this.checkForUpdates()?.catch(() => {})
    })
  }

  private checkForUpdates() {
    return this.updater?.checkForUpdates()
      .then(result => result?.downloadPromise)
  }
}
