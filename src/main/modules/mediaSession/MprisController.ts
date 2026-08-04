import type { MediaSessionData, MprisPlaybackState } from './MediaSessionManager'
import { native } from '../../napi'
import { logger } from '../../utils/Logger'

export class MprisController {
  private log = logger.child('MprisController')
  private initialized = false

  async init(): Promise<void> {
    if (this.initialized || process.platform !== 'linux') return

    try {
      native.mpris.init()
      this.initialized = true
      this.log.info('Rust Controller initialized successfully')
    } catch (error) {
      this.log.error('Failed to initialize native MPRIS', error)
    }
  }

  async update(data: MediaSessionData | null): Promise<void> {
    if (!this.initialized) return

    native.mpris.updateMetadata(data)
  }

  async updatePlaybackState(state: MprisPlaybackState): Promise<void> {
    if (!this.initialized) return
    native.mpris.updatePlaybackState(state)
  }

  async destroy(): Promise<void> {
    if (!this.initialized) return
    native.mpris.updatePlaybackState({ status: 'none' })
    this.initialized = false
  }
}