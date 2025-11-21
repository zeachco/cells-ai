import { defineConfig } from "vite";
import path from "path";
import fs from "fs";

// Custom plugin to watch WASM file and trigger full reload
function wasmReloadPlugin() {
  return {
    name: "wasm-reload",
    configureServer(server) {
      const wasmPath = path.resolve(
        "target/wasm32-unknown-unknown/release/cells.wasm",
      );

      server.watcher.add(wasmPath);

      server.watcher.on("change", (file) => {
        if (file === wasmPath) {
          console.log("WASM file changed, reloading...");
          server.ws.send({
            type: "full-reload",
            path: "*",
          });
        }
      });

      // Serve WASM file at /cells.wasm during dev
      server.middlewares.use((req, res, next) => {
        if (req.url === "/cells.wasm") {
          if (fs.existsSync(wasmPath)) {
            res.setHeader("Content-Type", "application/wasm");
            fs.createReadStream(wasmPath).pipe(res);
          } else {
            res.statusCode = 404;
            res.end("WASM file not found");
          }
        } else {
          next();
        }
      });
    },
  };
}

// Custom plugin to copy WASM file to dist during build
function wasmCopyPlugin() {
  return {
    name: "wasm-copy",
    closeBundle() {
      const srcPath = path.resolve(
        "target/wasm32-unknown-unknown/release/cells.wasm",
      );
      const destPath = path.resolve("dist/cells.wasm");

      if (fs.existsSync(srcPath)) {
        fs.copyFileSync(srcPath, destPath);
        console.log("WASM file copied to dist");
      } else {
        console.warn("WASM file not found at:", srcPath);
      }
    },
  };
}

export default defineConfig({
  plugins: [wasmReloadPlugin(), wasmCopyPlugin()],
  base: process.env.GITHUB_REPOSITORY
    ? `/${process.env.GITHUB_REPOSITORY.split("/")[1]}/`
    : "/",
  server: {
    port: 3000,
  },
  publicDir: false,
  build: {
    outDir: "dist",
  },
});
