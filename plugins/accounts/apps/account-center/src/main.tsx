import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import { resolveAccountCenterMode } from './mode'

createRoot(document.getElementById('app')!).render(
  <StrictMode><App mode={resolveAccountCenterMode(window.location.search)} /></StrictMode>,
)
