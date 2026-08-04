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
      const key = player.getVideoData?.()?.video_id || video.currentSrc || video.src || 'unknown'

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const win = window as any
      const state = win.__streamqGainState
      let gain: number | null = Math.max(0, Math.min(4,
        state?.key === key && Number.isFinite(state.gain) && state.gain
          ? state.gain
          : (player.getVolume() > 0 ? video.volume / (player.getVolume() / 100) : null)
      ))

      if (!gain && vol > 0) {
        player.setVolume(1)
        gain = (video.volume / 0.01) || null
        if (!gain) {
          const apiTarget = Math.round(vol * 100)
          player.setVolume(apiTarget)
          return { inputVol: vol, gain, apiTarget, resultInVideo: video.volume, videoKey: key }
        }
      }

      win.__streamqGainState = { key, gain }

      const desired = clamp(vol * gain)
      const apiTarget = Math.round(desired * 100)

      player.setVolume(apiTarget)
      video.volume = clamp(apiTarget === 0 && desired > 0 ? desired : Math.min(desired, apiTarget / 100))

      return { inputVol: vol, gain, desiredFinal: desired, apiTarget, resultInVideo: video.volume, videoKey: key }
    }, safeVol)
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
