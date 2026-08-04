import log from 'electron-log/main'
import type { FormatParams } from 'electron-log'
import { app } from 'electron'
import path from 'path'
import fs from 'fs'
import util from 'util'
import * as Sentry from '@sentry/electron/main'
import fastRedact from 'fast-redact'

interface RustLogMeta {
  __rust: true
  ts: string
  target?: string
  file?: string
  line?: number
  threadId?: string | number
  span?: string
  redact?: string[]
}

interface FrontendLogMeta {
  __frontend: true
  context: string
}

interface NodeLogMeta {
  __node: true
  context: string
  redact?: string[]
}

type LogMeta = RustLogMeta | FrontendLogMeta | NodeLogMeta

interface RustSpan {
  name: string
  [key: string]: unknown
}

interface RustLogObject {
  timestamp?: string
  level?: string
  target?: string
  filename?: string
  file?: string
  line_number?: number
  line?: number
  threadId?: string | number
  threadName?: string
  fields?: Record<string, unknown> & { message?: string; sentry?: boolean; __redact?: string }
  span?: RustSpan
  spans?: RustSpan[]
}

export class ContextLogger {
  constructor(private logger: typeof log, private meta: NodeLogMeta) {}

  public info(...args: unknown[]) { this.logger.info(this.meta, ...args) }
  public warn(...args: unknown[]) { this.logger.warn(this.meta, ...args) }
  public error(...args: unknown[]) { this.logger.error(this.meta, ...args) }
  public debug(...args: unknown[]) { this.logger.debug(this.meta, ...args) }
  public trace(...args: unknown[]) { this.logger.silly(this.meta, ...args) }

  public reportError(error: Error | unknown, ...args: unknown[]) {
    this.logger.error(this.meta, error, ...args)
    Sentry.captureException(error, {
      contexts: { Main: { context: this.meta.context, args } }
    })
  }

  public reportWarn(message: string, ...args: unknown[]) {
    this.logger.warn(this.meta, message, ...args)
    Sentry.captureMessage(message, {
      level: 'warning',
      contexts: { Main: { context: this.meta.context, args } }
    })
  }
}

export class AppLogger {
  private logger: typeof log
  private logDir: string
  private redactors = new Map<string, ReturnType<typeof fastRedact>>()
  private frontendConsoleLogs = process.env.STREAMQ_FRONTEND_LOGS === '1'

  constructor() {
    this.logger = log
    this.logDir = path.join(app.getPath('userData'), 'logs')

    this.logger.transports.console.format = (msg) => [this.formatLog(msg, true)]
    this.logger.transports.file.resolvePathFn = () => path.join(this.logDir, 'main.log')
    this.logger.transports.file.format = (msg) => [this.formatLog(msg, false)]
    this.logger.transports.file.maxSize = 5 * 1024 * 1024
    this.logger.errorHandler.startCatching()

    this.logger.hooks.push((msg, _transport, transportName) => {
      if (transportName !== 'console' || this.frontendConsoleLogs) return msg
      const meta = msg.data[0]
      if (typeof meta === 'object' && meta !== null && '__frontend' in meta) return false
      return msg
    })
  }

  private getRedactor(paths: string[]) {
    const key = paths.sort().join(',')
    if (!this.redactors.has(key)) {
      this.redactors.set(key, fastRedact({ paths, censor: '<HIDDEN>', serialize: false }))
    }
    return this.redactors.get(key)!
  }

  private formatJsDate(date: Date): string {
    const pad = (n: number, w: number) => n.toString().padStart(w, '0')
    const y = date.getUTCFullYear()
    const m = pad(date.getUTCMonth() + 1, 2)
    const d = pad(date.getUTCDate(), 2)
    const h = pad(date.getUTCHours(), 2)
    const min = pad(date.getUTCMinutes(), 2)
    const s = pad(date.getUTCSeconds(), 2)
    const ms = pad(date.getUTCMilliseconds(), 3)
    return `${y}-${m}-${d}T${h}:${min}:${s}.${ms}000Z`
  }

  private formatLog = ({ message: msg }: FormatParams, useColors: boolean): string => {
    let ts = this.formatJsDate(msg.date || new Date())
    let source = 'Main'
    let context = ''
    let extraPrefix = ''
    let textArgs = msg.data
    let meta: LogMeta | null = null

    if (msg.data.length > 0 && typeof msg.data[0] === 'object' && msg.data[0] !== null) {
      meta = msg.data[0] as LogMeta
      if ('__rust' in meta) {
        source = 'Native'
        ts = meta.ts
        textArgs = msg.data.slice(1)

        if (meta.target && meta.target.startsWith('streamq_native::') && meta.target.split('::').length === 2) {
          context = meta.target.split('::')[1]
        } else if (meta.file) {
          const parts = meta.file.split('/')
          const fileName = parts[parts.length - 1].replace('.rs', '')
          context = fileName.charAt(0).toUpperCase() + fileName.slice(1)
        } else {
          context = meta.target || 'Native'
        }

        const prefixParts = []
        if (meta.threadId) {
          const tid = String(meta.threadId)
          prefixParts.push(tid.startsWith('ThreadId') ? tid : `ThreadId(${tid})`)
        }
        if (meta.file && meta.line) {
          prefixParts.push(`${meta.file}:${meta.line}:`)
        }
        if (meta.span) {
          prefixParts.push(meta.span)
        }
        extraPrefix = prefixParts.join(' ')
      }
      else if ('__frontend' in meta) {
        source = 'UI'
        context = meta.context || ''
        textArgs = msg.data.slice(1)
      }
      else if ('__node' in meta) {
        source = 'Main'
        context = meta.context || ''
        textArgs = msg.data.slice(1)
      } else {
        meta = null
      }
    }

    if (!useColors && meta && 'redact' in meta && meta.redact && meta.redact.length > 0) {
      const redactor = this.getRedactor(meta.redact)
      textArgs = textArgs.map((arg: unknown) => {
        if (typeof arg === 'object' && arg !== null) {
          try {
            return redactor(JSON.parse(JSON.stringify(arg)))
          } catch {
            return arg
          }
        }
        return arg
      })
    }

    const level = msg.level.toUpperCase().padStart(5)
    let prefix = context ? `[${source}::${context}]` : `[${source}]`
    if (extraPrefix) prefix += ` ${extraPrefix}`

    const text = textArgs.map((arg: unknown) => {
      if (typeof arg === 'object' && arg !== null) {
        if (meta && '__rust' in meta) {
          return Object.entries(arg)
            .map(([k, v]) => `${k}=${typeof v === 'string' ? `"${v}"` : v}`)
            .join(' ')
        }
        if (useColors) return '\n' + util.inspect(arg, { colors: true, depth: 4, breakLength: 80 })
        else return JSON.stringify(arg)
      }
      return arg
    }).join(' ')

    if (useColors) {
      const colors: Record<string, string> = { INFO: '\x1b[32m', WARN: '\x1b[33m', ERROR: '\x1b[31m', DEBUG: '\x1b[34m', SILLY: '\x1b[35m' }
      const color = colors[level.trim()] || '\x1b[0m'
      const reset = '\x1b[0m'
      const dim = '\x1b[2m'
      return `${dim}${ts}${reset} ${color}${level}${reset} ${dim}${prefix}${reset} ${text}`
    }
    return `${ts} ${level} ${prefix} ${text}`
  }

  private dispatchLog(level: string, meta: LogMeta, ...args: unknown[]) {
    switch (level.toLowerCase()) {
      case 'error':
        this.logger.error(meta, ...args)
        break
      case 'warn':
        this.logger.warn(meta, ...args)
        break
      case 'debug':
        this.logger.debug(meta, ...args)
        break
      case 'trace':
        this.logger.silly(meta, ...args)
        break
      default:
        this.logger.info(meta, ...args)
        break
    }
  }

  public info(...args: unknown[]): void { this.logger.info(...args) }
  public warn(...args: unknown[]): void { this.logger.warn(...args) }
  public error(...args: unknown[]): void { this.logger.error(...args) }
  public debug(...args: unknown[]): void { this.logger.debug(...args) }

  public child(context: string, options?: { redact?: string[] }): ContextLogger {
    return new ContextLogger(this.logger, { __node: true, context, redact: options?.redact })
  }

  public logFromFrontend(level: string, context: string, options: { redact?: string[] }, ...args: unknown[]): void {
    this.dispatchLog(level, { __frontend: true, context, redact: options.redact }, ...args)
  }

  public logFromRustRaw(rawMessage: string): void {
    if (!rawMessage) return

    try {
      const logObj = JSON.parse(rawMessage) as RustLogObject
      const ts = logObj.timestamp || this.formatJsDate(new Date())
      const level = logObj.level || 'INFO'
      const text = logObj.fields?.message || ''

      const reportToSentry = logObj.fields?.sentry === true

      let redactKeys: string[] | undefined = undefined
      if (logObj.fields?.__redact && typeof logObj.fields.__redact === 'string') {
        redactKeys = logObj.fields.__redact.split(',').map((s) => s.trim())
      }

      const extraFieldsObj = Object.fromEntries(
        Object.entries(logObj.fields || {}).filter(([k]) => k !== 'message' && k !== 'sentry' && k !== '__redact')
      )

      let spanStr = ''
      if (logObj.spans && Array.isArray(logObj.spans)) {
        spanStr = logObj.spans.map((s) => {
          let name = s.name
          const sFields = Object.entries(s)
            .filter(([k]) => k !== 'name')
            .map(([k, v]) => `${k}=${typeof v === 'string' ? `"${v}"` : v}`)
            .join(', ')
          if (sFields) name += `{${sFields}}`
          return name
        }).join(':')
      } else if (logObj.span) {
        let name = logObj.span.name
        const sFields = Object.entries(logObj.span)
          .filter(([k]) => k !== 'name')
          .map(([k, v]) => `${k}=${typeof v === 'string' ? `"${v}"` : v}`)
          .join(', ')
        if (sFields) name += `{${sFields}}`
        spanStr = name
      }

      const meta: RustLogMeta = {
        __rust: true,
        ts,
        target: logObj.target,
        file: logObj.filename || logObj.file,
        line: logObj.line_number || logObj.line,
        threadId: logObj.threadId || logObj.threadName,
        span: spanStr,
        redact: redactKeys
      }

      const args: unknown[] = []
      if (text) args.push(text)
      if (Object.keys(extraFieldsObj).length > 0) args.push(extraFieldsObj)

      this.dispatchLog(level, meta, ...args)

      if (reportToSentry) {
        Sentry.captureMessage(text, {
          level: level.toLowerCase() === 'error' ? 'error' : 'warning',
          contexts: { Native: { ...meta } }
        })
      }

    } catch {
      const fallbackMeta: RustLogMeta = { __rust: true, ts: this.formatJsDate(new Date()), target: 'Native' }
      this.logger.info(fallbackMeta, rawMessage.trim())
    }
  }

  public async getLogs(): Promise<string> {
    let result = ''
    for (const file of [path.join(this.logDir, 'main.old.log'), path.join(this.logDir, 'main.log')]) {
      result += await fs.promises.readFile(file, 'utf8').catch(() => '')
    }
    return result
  }

  public getLogsForCrashReport(): string[] {
    const filesToAttach: string[] = []
    const currentLog = path.join(this.logDir, 'main.log')
    const oldLog = path.join(this.logDir, 'main.old.log')
    if (fs.existsSync(currentLog)) filesToAttach.push(currentLog)
    if (fs.existsSync(oldLog)) filesToAttach.push(oldLog)
    return filesToAttach
  }
}

export const logger = new AppLogger()
