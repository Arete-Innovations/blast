// custom/main — extension point for `main.ts`. Add your own Vue plugin
// installs here (toast service, dayjs, custom directives, etc.) without
// touching the framework `main.ts`. The framework calls this AFTER
// PrimeVue and the router are installed and BEFORE app.mount.

import type { App } from 'vue'

export function installCustomPlugins(_app: App): void {
  // Add `_app.use(YourPlugin, opts)` lines here.
}
