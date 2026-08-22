import { useEffect, useId, useRef, useState, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { X } from 'lucide-react'

interface ModalProps {
  open: boolean
  onClose: () => void
  title: string
  children: ReactNode
  bodyClassName?: string
  panelClassName?: string
}

export function Modal({ open, onClose, title, children, bodyClassName, panelClassName }: ModalProps) {
  const overlayRef = useRef<HTMLDivElement>(null)
  const panelRef = useRef<HTMLDivElement>(null)
  const closeButtonRef = useRef<HTMLButtonElement>(null)
  const previousFocusRef = useRef<HTMLElement | null>(null)
  const onCloseRef = useRef(onClose)
  const modalIdRef = useRef(`modal-${++modalSequence}`)
  const titleId = useId()
  const [portalHost, setPortalHost] = useState<HTMLElement | null>(null)

  useEffect(() => {
    onCloseRef.current = onClose
  }, [onClose])

  useEffect(() => {
    if (!open || typeof document === 'undefined') return
    const { host, release } = acquireModalHost(document)
    setPortalHost(host)
    return () => {
      setPortalHost(null)
      release()
    }
  }, [open])

  useEffect(() => {
    if (!open || !portalHost) return
    const modalId = modalIdRef.current
    const overlay = overlayRef.current
    previousFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
    if (overlay) {
      modalRegistry.set(modalId, {
        element: overlay,
        ariaHidden: overlay.getAttribute('aria-hidden'),
        inert: overlay.hasAttribute('inert'),
      })
    }
    modalStack.push(modalId)
    syncModalInteractivity()
    const restoreBackground = lockBackground(overlay?.ownerDocument ?? document)
    const frame = window.requestAnimationFrame(() => {
      if (panelRef.current?.contains(document.activeElement)) return
      const focusTarget = firstFocusable(panelRef.current) ?? closeButtonRef.current
      focusTarget?.focus()
    })
    const handler = (event: KeyboardEvent) => {
      if (modalStack.at(-1) !== modalId) return
      if (event.key === 'Escape') {
        event.preventDefault()
        onCloseRef.current()
        return
      }
      if (event.key === 'Tab') {
        trapFocus(event, panelRef.current)
      }
    }
    document.addEventListener('keydown', handler)
    return () => {
      window.cancelAnimationFrame(frame)
      document.removeEventListener('keydown', handler)
      const stackIndex = modalStack.lastIndexOf(modalId)
      if (stackIndex >= 0) modalStack.splice(stackIndex, 1)
      restoreModalInteractivity(modalId)
      restoreBackground()
      syncModalInteractivity()
      if (previousFocusRef.current?.isConnected) {
        previousFocusRef.current.focus()
      }
    }
  }, [open, portalHost])

  if (!open || !portalHost) return null

  return createPortal(
    <div
      className="modal-overlay"
      data-modal-root="true"
      ref={overlayRef}
      onClick={(event) => {
        if (event.target === overlayRef.current) onCloseRef.current()
      }}
    >
      <div
        ref={panelRef}
        className={panelClassName ? `modal-container ${panelClassName}` : 'modal-container'}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <header className="modal-header">
          <h2 id={titleId}>{title}</h2>
          <button
            ref={closeButtonRef}
            className="icon-button"
            aria-label="关闭设置"
            onClick={() => onCloseRef.current()}
          >
            <X size={18} />
          </button>
        </header>
        <div className={bodyClassName ? `modal-body ${bodyClassName}` : 'modal-body'}>
          {children}
        </div>
      </div>
    </div>,
    portalHost,
  )
}

let modalSequence = 0
const modalStack: string[] = []
const modalRegistry = new Map<string, {
  element: HTMLElement
  ariaHidden: string | null
  inert: boolean
}>()
const backgroundLocks = new Map<HTMLElement, {
  count: number
  ariaHidden: string | null
  inert: boolean
}>()

const MODAL_HOST_ID = 'lifesub-modal-host'
const FOCUSABLE = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

function acquireModalHost(documentRef: Document) {
  let host = documentRef.getElementById(MODAL_HOST_ID)
  if (!host) {
    host = documentRef.createElement('div')
    host.id = MODAL_HOST_ID
    documentRef.body.appendChild(host)
  }

  const currentCount = Number(host.dataset.refCount ?? '0') + 1
  host.dataset.refCount = String(currentCount)

  return {
    host,
    release: () => {
      const nextCount = Number(host?.dataset.refCount ?? '1') - 1
      if (nextCount <= 0) {
        host?.remove()
        return
      }
      host.dataset.refCount = String(nextCount)
    },
  }
}

function firstFocusable(root: HTMLElement | null) {
  return listFocusable(root)[0] ?? null
}

function listFocusable(root: HTMLElement | null) {
  if (!root) return [] as HTMLElement[]
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((element) => !element.hasAttribute('disabled'))
}

function trapFocus(event: KeyboardEvent, root: HTMLElement | null) {
  const focusable = listFocusable(root)
  if (focusable.length === 0) return
  const first = focusable[0]
  const last = focusable[focusable.length - 1]
  const active = document.activeElement as HTMLElement | null

  if (event.shiftKey && active === first) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && active === last) {
    event.preventDefault()
    first.focus()
  }
}

function lockBackground(documentRef: Document) {
  const host = documentRef.getElementById(MODAL_HOST_ID)
  if (!host) return () => undefined

  const targets = Array.from(documentRef.body.children)
    .filter((element) => element !== host)
    .map((element) => element as HTMLElement)

  targets.forEach((element) => {
    const existing = backgroundLocks.get(element)
    if (!existing) {
      backgroundLocks.set(element, {
        count: 1,
        ariaHidden: element.getAttribute('aria-hidden'),
        inert: element.hasAttribute('inert'),
      })
    } else {
      existing.count += 1
    }
    element.setAttribute('aria-hidden', 'true')
    element.setAttribute('inert', '')
  })

  return () => {
    targets.forEach((element) => {
      const lock = backgroundLocks.get(element)
      if (!lock) return
      lock.count -= 1
      if (lock.count > 0) return
      backgroundLocks.delete(element)
      if (lock.ariaHidden === null) {
        element.removeAttribute('aria-hidden')
      } else {
        element.setAttribute('aria-hidden', lock.ariaHidden)
      }
      if (!lock.inert) {
        element.removeAttribute('inert')
      }
    })
  }
}

function syncModalInteractivity() {
  const topModalId = modalStack.at(-1)
  modalRegistry.forEach((entry, modalId) => {
    if (modalId === topModalId) {
      restoreElementState(entry)
      return
    }
    entry.element.setAttribute('aria-hidden', 'true')
    entry.element.setAttribute('inert', '')
  })
}

function restoreModalInteractivity(modalId: string) {
  const entry = modalRegistry.get(modalId)
  if (!entry) return
  restoreElementState(entry)
  modalRegistry.delete(modalId)
}

function restoreElementState(entry: { element: HTMLElement, ariaHidden: string | null, inert: boolean }) {
  if (entry.ariaHidden === null) {
    entry.element.removeAttribute('aria-hidden')
  } else {
    entry.element.setAttribute('aria-hidden', entry.ariaHidden)
  }
  if (!entry.inert) {
    entry.element.removeAttribute('inert')
  }
}
