import { defineConfig } from 'vite';
import path from 'path';

// Custom plugin to watch WASM file and trigger full reload
function wasmReloadPlugin() {
  return {
    name: 'wasm-reload',
    configureServer(server) {
      const wasmPath = path.resolve('target/wasm32-unknown-unknown/release/cells.wasm');

      server.watcher.add(wasmPath);

      server.watcher.on('change', (file) => {
        if (file === wasmPath) {
          console.log('WASM file changed, reloading...');
          server.ws.send({
            type: 'full-reload',
            path: '*'
          });
        }
      });
    }
  };
}

export default defineConfig({
  plugins: [wasmReloadPlugin()],
  server: {
    port: 3000,
    open: true,
  },
  publicDir: false,
  build: {
    outDir: 'dist',
  },
});
