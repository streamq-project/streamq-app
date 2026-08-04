import { createSender } from './utils'

type SearchRoot = Document | ShadowRoot

const sendResize = createSender('widget:resize')

const findWidget = (root: SearchRoot): HTMLElement | null => {
  const widget = root.querySelector<HTMLElement>('div.widget')
  if (widget) return widget

  const elements = root.querySelectorAll<HTMLElement>('*')
  for (let index = 0; index < elements.length; index++) {
    const shadowRoot = elements[index].shadowRoot
    if (!shadowRoot) continue
    const shadowWidget = findWidget(shadowRoot)
    if (shadowWidget) return shadowWidget
  }

  return null
}

const observeWidget = () => {
  const widget = findWidget(document)
  if (!widget) {
    requestAnimationFrame(observeWidget)
    return
  }

  let lastWidth = 0
  let lastHeight = 0
  const resize = () => {
    const bounds = widget.getBoundingClientRect()
    const width = Math.ceil(bounds.width)
    const height = Math.ceil(bounds.height)
    if (width <= 0 || height <= 0 || (width === lastWidth && height === lastHeight)) return

    lastWidth = width
    lastHeight = height
    sendResize(width, height)
  }

  new ResizeObserver(resize).observe(widget)
  resize()
}

observeWidget()
