import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './styles/app.css'
import { App } from './App'
import { ErrorBoundary } from './components/ErrorBoundary'
import { applySavedTheme, StartupProblemScreen } from './components/StartupProblem'
import { pauseWhenHidden } from './lib/motion'
import { startupProblem } from './lib/startup'
import { toUiError } from './lib/backend'
import { useApp } from './lib/store'

pauseWhenHidden()

// A promise nobody handled is a failed action the person should hear about, in plain
// words (toUiError never shows the raw text), instead of vanishing in the console.
addEventListener('unhandledrejection', (e) => {
  useApp.setState({ toast: { ...toUiError(e.reason), at: Date.now() } })
})

const root = document.getElementById('root')
if (root) {
  // When the download list couldn't be opened, the backend runs in problem mode
  // and the window explains it instead of showing the app.
  void startupProblem()
    .catch(() => null)
    .then((problem) => {
      if (problem) applySavedTheme()
      createRoot(root).render(
        <StrictMode>
          <ErrorBoundary>
            {problem ? <StartupProblemScreen backend={problem} /> : <App />}
          </ErrorBoundary>
        </StrictMode>,
      )
    })
}
