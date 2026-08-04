import { app, dialog, shell } from 'electron'
import { spawnSync } from 'node:child_process'
import path from 'path'
import os from 'os'

import type { AudioSource, AppMediaSession, StreamQNative } from '../../native/dist/index'
import { settings } from './settings'
import { logger } from './utils/Logger'

const isDev = !app.isPackaged
const log = logger.child('napi')

const LINUX_RUNTIME_LIBRARIES = [
  'libwebkit2gtk-4.1.so.0',
  'libgtk-layer-shell.so.0',
  'libpulse.so.0'
] as const

const getMissingLinuxRuntimeLibraries = (nativePath: string, error: unknown) => {
  if (!(error instanceof Error) || (error as NodeJS.ErrnoException).code !== 'ERR_DLOPEN_FAILED') return

  const firstMissing = error.message.match(/([^/\s:]+\.so(?:\.[^/\s:]+)*): cannot open shared object file/)?.[1]
  if (!firstMissing || !LINUX_RUNTIME_LIBRARIES.some(library => library === firstMissing)) return

  const missing = new Set([firstMissing])
  const result = spawnSync('ldd', [nativePath], { encoding: 'utf8' })
  const output = `${result.stdout || ''}\n${result.stderr || ''}`

  for (const match of output.matchAll(/^\s*(\S+)\s+=>\s+not found\s*$/gm)) missing.add(match[1])
  return [...missing].sort()
}

const showMissingLinuxRuntimeLibrariesError = (libraries: string[]) => {
  const showError = () => {
    const response = dialog.showMessageBoxSync({
      type: 'error',
      title: 'Missing System Dependencies',
      message: 'Failed to start StreamQ',
      detail: 'The following required system libraries were not found:\n\n' +
        libraries.join('\n') +
        '\n\nInstall the required runtime packages for your Linux distribution and start StreamQ again.',
      buttons: ['Go to Download', 'Quit'],
      defaultId: 0,
      cancelId: 1
    })

    if (response === 0)
      return shell.openExternal('https://streamq.io/download').then(() => app.exit(1), () => app.exit(1))

    app.exit(1)
  }

  if (app.isReady()) showError()
  else app.prependOnceListener('ready', showError)
}

const ADDON_NAMES: Record<string, Record<string, string>> = {
  win32: {
    x64: 'index.win32-x64-msvc.node',
    arm64: 'index.win32-arm64-msvc.node'
  },
  linux: {
    x64: 'index.linux-x64-gnu.node',
    arm64: 'index.linux-arm64-gnu.node'
  }
}

export type MediaActionPayload =
  | { action: 'play' | 'pause' | 'playpause' | 'stop' | 'next' | 'previous' }
  | { action: 'seekForward' | 'seekBackward', offset?: number }
  | { action: 'seek', offset: number }
  | { action: 'seekTo', position: number }
  | { action: 'setVolume', value: number }
  | { action: 'volumeUp' | 'volumeDown' }
  | { action: 'setShuffle', state: boolean }
  | { action: 'setRepeat', state: 'none' | 'track' | 'playlist' }

export interface NativeEvents {
  keyDown: (err: Error | null, key: number) => void
  keyUp: (err: Error | null, key: number) => void
  mouseDown: (err: Error | null, key: number) => void
  mouseUp: (err: Error | null, key: number) => void
  keybindPressed: (err: Error | null, action: string) => void
  mediaAction: (err: Error | null, action: MediaActionPayload) => void
  mediaSessionsChanged: (err: Error | null, sessions: AppMediaSession[]) => void
  sourcesChanged: (err: Error | null, sources: AudioSource[]) => void
  appAudioSourceChanged: (err: Error | null, source: AudioSource | null) => void
}

export const native = (() => {
  const platform = os.platform()
  const arch = os.arch()
  const addonName = ADDON_NAMES[platform]?.[arch] ?? `index.${platform}-${arch}.node`

  const nativePath = isDev
    ? path.join(app.getAppPath(), 'native', 'dist', addonName)
    : path.join(path.dirname(process.resourcesPath), 'modules', 'native', addonName)

  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const { StreamQNative } = require(nativePath)
    const addon = new StreamQNative({ debug: isDev, keybinds: [], extractThumbnails: settings.data.extractThumbnails }) as Omit<StreamQNative, 'on'> & {
      on<K extends keyof NativeEvents>(ev: K, callback: NativeEvents[K]): void
    }

    addon.attachLogger((err: Error | null, rawMsg: string) => {
      if (err) return log.error('Failed to receive log from Rust', err)
      if (rawMsg) logger.logFromRustRaw(rawMsg)
    })

    return addon
  } catch (e) {
    const missingLibraries = platform === 'linux' ? getMissingLinuxRuntimeLibraries(nativePath, e) : undefined

    if (missingLibraries) {
      showMissingLinuxRuntimeLibrariesError(missingLibraries)
      return undefined as never
    }

    log.error(`Failed to load native addon from ${nativePath}`, e)
    throw e
  }
})()

export type * from '../../native/dist/index'
