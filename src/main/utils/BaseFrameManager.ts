import { WebContents, WebFrameMain } from 'electron'
import { logger } from './Logger'

export abstract class BaseFrameManager<T> {
  protected log = logger.child('BaseFrameManager')
  protected controllers = new Map<number, T>()

  constructor(protected wc: WebContents) {
    this.watch()
  }

  protected abstract isTargetFrame(frame: WebFrameMain): boolean
  protected abstract createController(frame: WebFrameMain): T | Promise<T>
  protected abstract onFrameRemoved(id: number, controller: T): void | Promise<void>

  async syncFrames(): Promise<void> {
    const alive = new Set<number>()

    for (const frame of this.wc.mainFrame.frames) {
      if (!this.isTargetFrame(frame)) continue

      alive.add(frame.routingId)

      if (!this.controllers.has(frame.routingId)) {
        try {
          const controller = await this.createController(frame)
          this.controllers.set(frame.routingId, controller)
        } catch (error) {
          this.log.error('Failed to create controller', error)
        }
      }
    }

    for (const [id, controller] of this.controllers) {
      if (!alive.has(id)) {
        try {
          await this.onFrameRemoved(id, controller)
        } catch (error) {
          this.log.error('Failed to remove controller', error)
        }
        this.controllers.delete(id)
      }
    }
  }

  private resyncHandler = () => {
    this.syncFrames().catch(err => {
      this.log.error('Failed to sync frames', err)
    })
  }

  private watch(): void {
    this.wc.on('dom-ready', this.resyncHandler)
    this.wc.on('did-frame-finish-load', this.resyncHandler)
    this.wc.on('did-navigate-in-page', this.resyncHandler)
    this.wc.on('did-start-navigation', this.resyncHandler)

    this.resyncHandler()
  }

  async destroy(): Promise<void> {
    this.wc.removeListener('dom-ready', this.resyncHandler)
    this.wc.removeListener('did-frame-finish-load', this.resyncHandler)
    this.wc.removeListener('did-navigate-in-page', this.resyncHandler)
    this.wc.removeListener('did-start-navigation', this.resyncHandler)

    for (const [id, controller] of this.controllers) {
      try {
        await this.onFrameRemoved(id, controller)
      } catch (error) {
        this.log.error('Failed to remove controller on destroy', error)
      }
    }
    this.controllers.clear()
  }
}
