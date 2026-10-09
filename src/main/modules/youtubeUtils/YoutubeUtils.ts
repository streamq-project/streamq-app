import { native } from '../../napi'
import { WebFrameMain } from 'electron'
import { executeInFrame } from '../../utils/executeInFrame'

export type VideoAspectRatio = number | null

export type VideoVolumeState = {
  inputVol: number
  gain: number | null
  apiTarget: number
  resultInVideo: number
  videoKey: string
  desiredFinal?: number
} | null

export type VideoStatus = {
  errorCode: string | null
  isPlayable: boolean
} | null

export class YoutubeUtils {
  constructor(private frame: WebFrameMain) {}

  getAspectRatio(): Promise<VideoAspectRatio> {
    return executeInFrame(this.frame, () => {
      const video = document.querySelector('video')
      if (!video || video.videoWidth <= 0 || video.videoHeight <= 0) return null

      return video.videoWidth / video.videoHeight
    })
  }

  setVideoVolume(vol: number): Promise<VideoVolumeState> {
    const safeVol = Number.isFinite(vol) ? Math.max(0, Math.min(1, vol)) : 0
    return executeInFrame(this.frame, (vol: number) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const player = document.querySelector<any>('#movie_player, .html5-video-player')
      const video = document.querySelector('video')
      if (!player || !video) return null

      const clamp = (x: number) => Math.max(0, Math.min(1, x))
      const getKey = () =>
        player.getVideoData?.()?.video_id || video.currentSrc || video.src || 'unknown'
      const key = getKey()
      const descriptor = Object.getOwnPropertyDescriptor(HTMLMediaElement.prototype, 'volume')!

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const win = window as any
      const states: WeakMap<HTMLVideoElement, { key: string; gain: number | null }> =
        win.__streamqVolumeStates ??= new WeakMap()
      const state = states.get(video) ?? { key, gain: null }

      if (!states.has(video)) {
        Object.defineProperty(video, 'volume', {
          configurable: true,
          enumerable: descriptor.enumerable,
          get: descriptor.get,
          set(value: number) {
            const currentKey = getKey()
            if (state.key !== currentKey) {
              state.key = currentKey
              state.gain = null
            }
            if (state.gain === null) descriptor.set!.call(video, value)
          }
        })
        states.set(video, state)
      }

      if (state.key !== key) {
        state.key = key
        state.gain = null
      }

      if (state.gain === null && vol > 0) {
        player.setVolume(1)
        const measuredGain = video.volume / 0.01
        if (Number.isFinite(measuredGain) && measuredGain > 0)
          state.gain = Math.min(1, measuredGain)
      }

      const gain = state.gain
      if (gain === null) {
        const apiTarget = vol > 0 ? Math.max(1, Math.round(vol * 100)) : 0
        player.setVolume(apiTarget)
        if (vol === 0) descriptor.set!.call(video, 0)
        return { inputVol: vol, gain, apiTarget, resultInVideo: video.volume, videoKey: key }
      }

      const desired = clamp(vol * gain)
      const apiTarget = Math.round(desired * 100)

      player.setVolume(apiTarget)
      descriptor.set!.call(video, desired)

      return { inputVol: vol, gain, desiredFinal: desired, apiTarget, resultInVideo: video.volume, videoKey: key }
    }, safeVol)
  }

  getErrorCode(): Promise<VideoStatus> {
    return executeInFrame(this.frame, () => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const player = document.querySelector<any>('#movie_player, .html5-video-player')
      const videoData = player?.getVideoData()
      return videoData ? { errorCode: videoData.errorCode, isPlayable: videoData.isPlayable } : null
    })
  }

  async setPiPMode(isActive: boolean, isOnTop: boolean) {
    await executeInFrame(this.frame, (active: boolean) => {
      const video = document.querySelector('video')
      if (!video) return
      if (active) {
        video.requestPictureInPicture()
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const win = window as any
        if (!win.streamqLeavePiPListener) {
          win.streamqLeavePiPListener = () => window.parent.postMessage({ type: 'streamq:leavepictureinpicture' }, '*')
        }
        video.removeEventListener('leavepictureinpicture', win.streamqLeavePiPListener)
        video.addEventListener('leavepictureinpicture', win.streamqLeavePiPListener)
      } else {
        document.exitPictureInPicture()
      }
    }, isActive)
    if (isActive) native.window.setPipAlwaysOnTopMode(isOnTop)
  }
  setPiPAlwaysOnTopMode(isOnTop: boolean) {
    native.window.setPipAlwaysOnTopMode(isOnTop)
  }
}
