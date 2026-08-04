import { WebContents, WebFrameMain } from 'electron'
import { BaseFrameManager } from '../../utils/BaseFrameManager'
import { YoutubeUtils, VideoVolumeState } from './YoutubeUtils'

export class YoutubeManager extends BaseFrameManager<YoutubeUtils> {
  constructor(wc: WebContents) {
    super(wc)
  }

  protected isTargetFrame(frame: WebFrameMain): boolean {
    return frame.origin === 'https://www.youtube.com'
  }

  protected createController(frame: WebFrameMain): YoutubeUtils {
    return new YoutubeUtils(frame)
  }

  protected onFrameRemoved(): void {}

  async setVideoVolume(vol: number): Promise<VideoVolumeState> {
    const promises = Array.from(this.controllers.values()).map(controller =>
      controller.setVideoVolume(vol)
    )
    const results = await Promise.allSettled(promises)
    const firstSuccess = results.find(r => r.status === 'fulfilled' && r.value !== null)
    return firstSuccess?.status === 'fulfilled' ? firstSuccess.value : null
  }

  async getAspectRatio(): Promise<number | null> {
    const promises = Array.from(this.controllers.values()).map(controller =>
      controller.getAspectRatio()
    )
    const results = await Promise.allSettled(promises)
    const firstSuccess = results.find(r => r.status === 'fulfilled' && r.value !== null)
    return firstSuccess?.status === 'fulfilled' ? firstSuccess.value : null
  }

  async setPiPMode(isActive: boolean, isOnTop: boolean): Promise<void> {
    for (const controller of this.controllers.values()) {
      await controller.setPiPMode(isActive, isOnTop)
    }
  }

  setPiPAlwaysOnTopMode(isOnTop: boolean): void {
    for (const controller of this.controllers.values()) {
      controller.setPiPAlwaysOnTopMode(isOnTop)
    }
  }
}
