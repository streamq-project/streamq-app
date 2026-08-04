import { WebFrameMain } from 'electron'
import { MediaSessionData } from './MediaSessionManager'
import { executeInFrame } from '../../utils/executeInFrame'

type StreamqMediaSessionWindow = Window & {
  __isStreamqMediaSessionInitialized?: boolean
  __streamqOriginalSetActionHandler?: MediaSession['setActionHandler']
  __streamqSetMediaMetadata?: (metadata: MediaMetadata | null) => void
}

export class IframeMediaSessionController {
  constructor(private frame: WebFrameMain) {}

  async init(): Promise<void> {
    await executeInFrame(this.frame, () => {
      const win = window as StreamqMediaSessionWindow
      if (win.__isStreamqMediaSessionInitialized) return
      win.__isStreamqMediaSessionInitialized = true

      if ('mediaSession' in navigator) {
        const emitMediaAction = (action: string) => {
          window.parent.postMessage({ type: 'streamq:media-action', action }, '*')
        }
        const streamqHandlers: Partial<Record<MediaSessionAction, MediaSessionActionHandler>> = {
          previoustrack: () => emitMediaAction('previous'),
          nexttrack: () => emitMediaAction('next')
        }

        const originalSetActionHandler = navigator.mediaSession.setActionHandler.bind(navigator.mediaSession)
        win.__streamqOriginalSetActionHandler = originalSetActionHandler

        const metadataDescriptor = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(navigator.mediaSession), 'metadata')
        if (metadataDescriptor?.set) {
          win.__streamqSetMediaMetadata = metadataDescriptor.set.bind(navigator.mediaSession)
          Object.defineProperty(navigator.mediaSession, 'metadata', {
            configurable: true,
            enumerable: metadataDescriptor.enumerable,
            get: metadataDescriptor.get?.bind(navigator.mediaSession),
            set: () => {}
          })
        }

        navigator.mediaSession.setActionHandler = (name, handler) => {
          originalSetActionHandler(name, streamqHandlers[name] ?? handler)
        }

        for (const [name, handler] of Object.entries(streamqHandlers)) {
          try {
            originalSetActionHandler(name as MediaSessionAction, handler)
          } catch {/* ignore */}
        }
      }
    })
  }

  async update(data: MediaSessionData): Promise<void> {
    await executeInFrame(this.frame, (data: MediaSessionData) => {
      if (!('mediaSession' in navigator)) return
      const win = window as StreamqMediaSessionWindow
      const metadata = new MediaMetadata({
        title: data.title,
        artist: data.artist,
        album: data.album,
        artwork: data.artUrl ? [{ src: data.artUrl }] : []
      })

      if (win.__streamqSetMediaMetadata) {
        win.__streamqSetMediaMetadata(metadata)
      } else {
        navigator.mediaSession.metadata = metadata
      }
    }, data)
  }

  async destroy(): Promise<void> {
    try {
      await executeInFrame(this.frame, () => {
        const win = window as StreamqMediaSessionWindow

        if ('mediaSession' in navigator) {
          if (win.__streamqSetMediaMetadata) {
            win.__streamqSetMediaMetadata(null)
            Reflect.deleteProperty(navigator.mediaSession, 'metadata')
          } else {
            navigator.mediaSession.metadata = null
          }
          if (win.__streamqOriginalSetActionHandler) {
            navigator.mediaSession.setActionHandler = win.__streamqOriginalSetActionHandler
          }
        }

        delete win.__streamqSetMediaMetadata
        delete win.__streamqOriginalSetActionHandler
        delete win.__isStreamqMediaSessionInitialized
      })
    } catch { /* ignore */ }
  }
}
