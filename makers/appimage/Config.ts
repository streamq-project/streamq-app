export interface MakerAppImageOptionsConfig {
  id?: string
  productName?: string
  description?: string
  genericName?: string
  bin?: string
  icon?: string
  categories?: string[]
  mimeType?: string[]
}

export interface MakerAppImageUpdaterConfig {
  url: string
  channel?: string
  updaterCacheDirName?: string
}

export interface MakerAppImageConfig {
  options?: MakerAppImageOptionsConfig
  updater?: MakerAppImageUpdaterConfig
}
