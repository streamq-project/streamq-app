import { WebFrameMain } from 'electron'

export function executeInFrame<T, A extends unknown[]>(frame: WebFrameMain | undefined | null, fn: (...args: A) => T, ...args: A): Promise<T> {
  if (!frame) return Promise.resolve(undefined as unknown as T)
  const argsStr = args.map(a => JSON.stringify(a)).join(',')
  return frame.executeJavaScript(`(${fn.toString()})(${argsStr})`, true) as Promise<T>
}
