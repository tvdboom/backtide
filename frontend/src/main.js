import { createApp } from 'vue'
import './style.css'
import App from './app.vue'
import { applyTheme, resolveTheme } from './theme'

applyTheme(resolveTheme())
createApp(App).mount('#app')
