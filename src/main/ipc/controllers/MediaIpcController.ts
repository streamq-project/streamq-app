import { BaseIpcController } from '../BaseIpcController'
import { settings } from '../../settings'
import { native } from '../../napi'
import type { MainWindow } from '../../app/Main'
import { MediaSessionManager } from '../../modules/mediaSession/MediaSessionManager'

export class MediaIpcController extends BaseIpcController {
  private mediaSessionManager: MediaSessionManager

  constructor(private mainWindow: MainWindow) {
    super()

    this.mediaSessionManager = new MediaSessionManager(this.mainWindow.window.webContents)

    native.on('mediaAction', (_err, payload) => {
      this.mainWindow.window.webContents.send('media:action', payload)
    })

    this.defineInvoke('media:setSystemMediaControlsSession', (_, isActive) => {
      settings.set('systemMediaControlsSession', isActive)
    })
    this.defineInvoke('media:setExtractThumbnails', (_, isActive) => {
      settings.set('extractThumbnails', isActive)
    })
    this.defineInvoke('media:setAppSource', (_, source) => native.media.setAppSource(source))
    this.defineInvoke('media:updateMediaSession', (_, data) => this.mediaSessionManager.updateMediaSession(data))
    this.defineInvoke('mpris:updateMetadata', (_, data) => this.mediaSessionManager.updateMprisMetadata(data))
    this.defineInvoke('mpris:updatePlaybackState', (_, state) => this.mediaSessionManager.updateMprisPlaybackState(state))
    this.defineInvoke('media:pause', (_, apps) => native.media.pause(apps))
    this.defineInvoke('media:resume', (_, apps) => native.media.resume(apps))
    this.defineInvoke('media:setVolume', (_, app, volume) => native.media.setVolume(app, volume))
    this.defineInvoke('media:getAudioSources', () => native.audio.getAudioSources())
  }

  public override destroy(): void {
    this.mediaSessionManager.destroy()
    super.destroy()
  }
}
