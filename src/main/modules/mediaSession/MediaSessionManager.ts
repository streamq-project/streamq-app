import { WebContents, WebFrameMain } from 'electron'
import type { MprisMetadata } from '../../napi'
import { IframeMediaSessionController } from './IframeController'
import { MprisController } from './MprisController'
import { BaseFrameManager } from '../../utils/BaseFrameManager'
import { logger } from '../../utils/Logger'
import os from 'os'

export type MediaSessionData = Omit<MprisMetadata, 'volume'>

export type MprisPlaybackState = Partial<{
  status: MediaSessionPlaybackState
  position: number
  volume: number
  shuffle: boolean
  repeat: 'none' | 'track' | 'playlist'
}>

export class MediaSessionManager extends BaseFrameManager<IframeMediaSessionController> {
  protected log = logger.child('MediaSessionManager')
  private mprisController: MprisController | null = null
  private currentData: MediaSessionData | null = null

  constructor(wc: WebContents) {
    super(wc)

    if (os.platform() === 'linux') {
      this.mprisController = new MprisController()
      this.mprisController.init().catch(e => this.log.error('Failed to initialize MPRIS controller', e))
    }
  }

  protected isTargetFrame(frame: WebFrameMain): boolean {
    return frame.url.includes('youtube.com')
  }

  protected async createController(frame: WebFrameMain): Promise<IframeMediaSessionController> {
    const controller = new IframeMediaSessionController(frame)
    await controller.init()

    if (this.currentData) await controller.update(this.currentData)
    return controller
  }

  protected async onFrameRemoved(_id: number, controller: IframeMediaSessionController): Promise<void> {
    await controller.destroy()
  }

  async updateMediaSession(data: MediaSessionData | null): Promise<void> {
    this.currentData = data
    if (!data) return

    for (const controller of this.controllers.values()) {
      controller.update(data).catch(e => this.log.error('Failed to update iframe media session', e))
    }
  }

  async updateMprisMetadata(data: MediaSessionData | null): Promise<void> {
    this.mprisController?.update(data).catch(e => this.log.error('Failed to update MPRIS metadata', e))
  }

  async updateMprisPlaybackState(state: MprisPlaybackState): Promise<void> {
    this.mprisController?.updatePlaybackState(state).catch(e => this.log.error('Failed to update MPRIS playback state', e))
  }

  override async destroy(): Promise<void> {
    await this.mprisController?.destroy().catch(e => this.log.error('Failed to destroy MPRIS controller', e))
    await super.destroy()
  }
}
