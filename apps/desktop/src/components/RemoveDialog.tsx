import { FolderOpen, Trash, X } from '@phosphor-icons/react'
import { useEffect, useRef, useState } from 'react'

/** The platform's name for where deleted files go. */
export const BIN = /Win/i.test(navigator.platform) ? 'Recycle Bin' : 'Trash'

export interface RemoveChoice {
  label: string
  /** The destructive choice is red and goes last. */
  danger?: boolean
  run: () => Promise<unknown> | void
}

/**
 * Asks before removing, and waits (B8.2): a modal that stays until a choice is
 * made. Esc and a click outside mean Cancel. It never closes on a timer.
 */
export function RemoveDialog({
  open,
  title,
  text,
  where,
  facts,
  choices,
  onClose,
}: {
  open: boolean
  title: string
  text: string
  where: string
  facts: string
  choices: RemoveChoice[]
  onClose: () => void
}) {
  const ref = useRef<HTMLDialogElement>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    const d = ref.current
    if (!d) return
    if (open && !d.open) {
      setBusy(false)
      d.showModal()
      // Start on Cancel: Enter right after opening must not delete anything.
      d.querySelector<HTMLButtonElement>('[data-cancel]')?.focus()
    }
    if (!open && d.open) d.close()
  }, [open])

  const pick = async (c: RemoveChoice) => {
    setBusy(true)
    try {
      await c.run()
    } finally {
      setBusy(false)
      onClose()
    }
  }

  return (
    <dialog
      ref={ref}
      className="dialog remove-dialog"
      aria-labelledby="rm-title"
      aria-describedby="rm-text"
      onClose={onClose}
      onCancel={(e) => {
        e.preventDefault()
        onClose()
      }}
      onClick={(e) => {
        // A click on the backdrop lands on the dialog element itself.
        if (e.target === e.currentTarget) onClose()
      }}
    >
      {open && (
        <form method="dialog" onSubmit={(e) => e.preventDefault()}>
          <header className="dialog-head">
            <h2 id="rm-title">{title}</h2>
            <button type="button" className="icon-btn" aria-label="Close" onClick={onClose}>
              <X size={16} aria-hidden />
            </button>
          </header>
          <p id="rm-text" className="dialog-text">
            {text}
          </p>
          <div className="file-summary">
            <span aria-hidden>
              <FolderOpen size={18} />
            </span>
            <span>
              <b translate="no" title={where}>
                {where}
              </b>
              <span className="muted num">{facts}</span>
            </span>
          </div>
          <footer className="dialog-foot">
            <button type="button" className="btn btn-ghost" data-cancel onClick={onClose}>
              Cancel
            </button>
            {choices.map((c) => (
              <button
                key={c.label}
                type="button"
                className={c.danger ? 'btn btn-danger' : 'btn'}
                disabled={busy}
                onClick={() => void pick(c)}
              >
                {c.danger && <Trash size={16} aria-hidden />}
                {c.label}
              </button>
            ))}
          </footer>
        </form>
      )}
    </dialog>
  )
}
