import { createApp } from 'vue'

import App from './App.vue'
import installPrimeVue from '@/generated/plugins/primevue'
import router from './router'
import '@/generated/styles/tokens.css'
import './styles/base.css'

const app = createApp(App)
installPrimeVue(app)
app.use(router)
app.mount('#app')
