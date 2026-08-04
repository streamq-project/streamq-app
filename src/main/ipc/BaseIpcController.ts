import { ipcMain, IpcMain, IpcMainInvokeEvent, IpcMainEvent } from 'electron'
import { EventEmitter } from 'events'
import type { IpcError, IpcInvokes, IpcRendererSends, IpcSyncs } from '../../shared/ipc-types'
import { native, NativeEvents } from '../napi'
import { logger } from '../utils/Logger'

const globalNativeEmitter = new EventEmitter()
const registeredNativeEvents = new Set<keyof NativeEvents>()

type InvokeHandler<K extends keyof IpcInvokes> =
  (event: IpcMainInvokeEvent, ...args: Parameters<IpcInvokes[K]>) => ReturnType<IpcInvokes[K]> | Promise<ReturnType<IpcInvokes[K]>>

type SyncHandler<K extends keyof IpcSyncs> =
  (event: IpcMainEvent, ...args: Parameters<IpcSyncs[K]>) => ReturnType<IpcSyncs[K]>

type RendererSendHandler<K extends keyof IpcRendererSends> =
  (event: IpcMainEvent, ...args: Parameters<IpcRendererSends[K]>) => void

type NativeHandler<K extends keyof NativeEvents> =
  (payload: Parameters<NativeEvents[K]>[1]) => void

const serializeError = (error: unknown): IpcError => {
  if (!error || typeof error !== 'object') {
    return { name: 'Error', message: String(error) }
  }

  const values = error as Record<string, unknown>
  return {
    ...Object.fromEntries(Object.getOwnPropertyNames(error).map(name => [name, values[name]])),
    name: error instanceof Error ? error.name : 'Error',
    message: error instanceof Error ? error.message : String(error)
  }
}

export abstract class BaseIpcController {
  private registeredChannels: string[] = []
  private nativeEventCleanups: (() => void)[] = []

  constructor(private readonly ipc: IpcMain = ipcMain) {}

  protected defineInvoke<K extends keyof IpcInvokes>(channel: K, listener: InvokeHandler<K>): void {
    this.ipc.handle(channel, async (event, ...args) => {
      try {
        return { ok: true, value: await listener(event, ...(args as Parameters<IpcInvokes[K]>)) }
      } catch (error) {
        return { ok: false, error: serializeError(error) }
      }
    })
    this.registeredChannels.push(channel)
  }

  protected defineSync<K extends keyof IpcSyncs>(channel: K, listener: SyncHandler<K>): void {
    this.ipc.on(channel, (event, ...args) => {
      event.returnValue = listener(event, ...(args as Parameters<IpcSyncs[K]>))
    })
    this.registeredChannels.push(channel)
  }

  protected defineRendererSend<K extends keyof IpcRendererSends>(channel: K, listener: RendererSendHandler<K>): void {
    this.ipc.on(channel, (event, ...args) => {
      listener(event, ...(args as Parameters<IpcRendererSends[K]>))
    })
    this.registeredChannels.push(channel)
  }

  protected defineNativeEvent<K extends keyof NativeEvents>(eventName: K, listener: NativeHandler<K>): void {
    if (!registeredNativeEvents.has(eventName)) {
      registeredNativeEvents.add(eventName)
      native.on(eventName, (err: Error | null, payload: unknown) => {
        if (err) return logger.error(`Native event error [${eventName}]`, err)
        globalNativeEmitter.emit(eventName, payload)
      })
    }

    globalNativeEmitter.on(eventName, listener)
    this.nativeEventCleanups.push(() => globalNativeEmitter.off(eventName, listener))
  }

  public destroy(): void {
    this.registeredChannels.forEach(channel => {
      this.ipc.removeHandler(channel)
      this.ipc.removeAllListeners(channel)
    })
    this.registeredChannels = []

    this.nativeEventCleanups.forEach(cleanup => cleanup())
    this.nativeEventCleanups = []
  }
}
