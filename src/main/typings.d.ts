declare const BOOTSTRAP_WINDOW_VITE_DEV_SERVER_URL: string | undefined
declare const BOOTSTRAP_WINDOW_VITE_NAME: string

declare module "*.yml" {
  const data: Record<string, unknown>
  export default data
}

declare module "*?raw" {
  const content: string
  export default content
}
