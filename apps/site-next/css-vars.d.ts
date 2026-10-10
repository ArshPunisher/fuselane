// The markup sets CSS custom properties inline (style={{ '--i': '3' }}).
import 'react'

declare module 'react' {
  interface CSSProperties {
    [key: `--${string}`]: string | number
  }
}
