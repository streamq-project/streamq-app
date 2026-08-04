import { ipcRenderer, IpcRendererEvent } from 'electron'
import type { IpcInvokes, IpcEvents, IpcResult, IpcSyncs, IpcRendererSends } from '../shared/ipc-types'

export const createInvoker = <K extends keyof IpcInvokes>(channel: K) =>
  (...args: Parameters<IpcInvokes[K]>): Promise<IpcResult<Awaited<ReturnType<IpcInvokes[K]>>>> =>
    ipcRenderer.invoke(channel, ...args)

export const createSubscriber = <K extends keyof IpcEvents>(channel: K) =>
  (cb: IpcEvents[K]) => {
    const listener = (_: IpcRendererEvent, ...args: Parameters<IpcEvents[K]>) =>
      (cb as (...args: unknown[]) => void)(...args)

    ipcRenderer.on(channel, listener)
    return () => {
      ipcRenderer.off(channel, listener)
    }
  }



export const createSender = <K extends keyof IpcRendererSends>(channel: K) =>
  (...args: Parameters<IpcRendererSends[K]>): void =>
    ipcRenderer.send(channel, ...args)

export const sendSync = <K extends keyof IpcSyncs>(channel: K): ReturnType<IpcSyncs[K]> =>
  ipcRenderer.sendSync(channel)
