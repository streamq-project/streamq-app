import * as Sentry from '@sentry/electron/main'
import { app } from 'electron'
import fs from 'fs'
import path from 'path'
import os from 'os'
import { logger } from './Logger'
import { settings } from '../settings'

interface GPUInfo {
  gpuDevice?: Array<{
    vendorId: number
    deviceId: number
    vendorString?: string
    deviceString?: string
    driverVersion?: string
  }>
}

const anonymizeLogs = (logText: string) => logText
  .replace(/[A-Z]:\\Users\\[^\\]+\\/gi, 'C:\\Users\\<USER>\\')
  .replace(/\/home\/[^/]+\//gi, '/home/<USER>/')
  .replace(/\/Users\/[^/]+\//gi, '/Users/<USER>/')

export function initSentry() {
  let cachedGpuContext: Record<string, string | number | undefined> | null = null

  app.getGPUInfo('basic').then((info) => {
    const gpuInfo = info as GPUInfo

    if (gpuInfo && gpuInfo.gpuDevice && gpuInfo.gpuDevice.length > 0) {
      const gpu = gpuInfo.gpuDevice[0]

      let vendorName = gpu.vendorString
      const modelName = gpu.deviceString

      if (!vendorName || vendorName === 'Unknown') {
        const vendors: Record<number, string> = {
          4318: 'NVIDIA',
          4098: 'AMD',
          32902: 'Intel',
          4203: 'Apple'
        }
        vendorName = vendors[gpu.vendorId] || 'Unknown'
      }

      cachedGpuContext = {
        name: modelName || vendorName,
        vendor_name: vendorName,
        id: gpu.deviceId,
        vendor_id: gpu.vendorId,
        version: gpu.driverVersion
      }
    }
  }).catch(e => logger.warn('Failed to fetch GPU info for Sentry', e))

  Sentry.init({
    dsn: 'https://50ba40a9c2eb39924bc4dff8beea8f71@sentry.streamq.io/4511184830857296',
    release: `streamq-app@${app.getVersion()}`,
    environment: app.isPackaged ? 'production' : 'development'
  })

  Sentry.setContext('browser', {
    name: 'Chrome',
    version: process.versions.chrome,
  })

  const cpus = os.cpus()
  Sentry.setContext('device', {
    name: os.hostname(),
    model: cpus.length > 0 ? cpus[0].model : 'PC',
    family: 'Desktop',
    arch: os.arch(),
  })

  Sentry.addEventProcessor((event, hint) => {
    const isManualReport = event.tags && event.tags.manual_report === 'true'

    if (!settings.data.allowCrashReports && !isManualReport) {
      logger.info('Sentry event dropped: User disabled automatic crash reports')
      return null
    }

    if (cachedGpuContext) {
      event.contexts = event.contexts || {}
      event.contexts.gpu = { ...event.contexts.gpu, ...cachedGpuContext }
    }

    try {
      const logFiles = logger.getLogsForCrashReport()
      if (hint) {
        hint.attachments = hint.attachments || []

        logFiles.forEach((filePath) => {
          if (fs.existsSync(filePath)) {
            const rawData = fs.readFileSync(filePath, 'utf8')
            const safeData = anonymizeLogs(rawData)

            hint.attachments!.push({
              filename: path.basename(filePath),
              data: Buffer.from(safeData, 'utf8'),
              contentType: 'text/plain',
            })
          }
        })
      }
    } catch (e) {
      logger.error('Failed to attach logs to Sentry event', e)
    }

    return event
  })

  logger.info('Sentry initialized')
}
