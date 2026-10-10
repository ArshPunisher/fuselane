import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './styles/app.css'
import { App } from './App'
import { applySavedTheme, StartupProblemScreen } from './components/StartupProblem'
import { pauseWhenHidden } from './lib/motion'
import { startupProblem } from './lib/startup'

pauseWhenHidden()

const root = document.getElementById('root')
if (root) {
  // When the download list couldn't be opened, the backend runs in problem mode
  // and the window explains it instead of showing the app.
  void startupProblem()
    .catch(() => null)
    .then((problem) => {
      if (problem) applySavedTheme()
      createRoot(root).render(
        <StrictMode>{problem ? <StartupProblemScreen backend={problem} /> : <App />}</StrictMode>,
      )
    })
}
