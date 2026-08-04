export type YoutubeProxyConnectionDirect = {
  method: 'direct'
}

export type YoutubeProxyConnectionXray = {
  method: 'xray'
  port: number
  config: string
}

export type YoutubeProxyConnectionProxy = {
  method: 'proxy'
  host: string
  port: number
}

export type YoutubeProxyConnection = {
  direct: YoutubeProxyConnectionDirect
  xray: YoutubeProxyConnectionXray
  proxy: YoutubeProxyConnectionProxy
}
