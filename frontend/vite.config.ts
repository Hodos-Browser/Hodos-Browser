import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  server: {
    host: '127.0.0.1',
    port: 5137,
    cors: true
  },
  build: {
    // ⭐ THIS is the load-bearing engine binding — not the `browserslist` field in
    // package.json. Nothing in this project currently reads browserslist (no
    // postcss, autoprefixer, lightningcss, babel or plugin-legacy), so that field
    // is declaration-of-intent only. esbuild reads THIS.
    //
    // Why it needs to exist at all: this bundle runs exclusively inside the
    // Chromium we ship (CEF 150 = Chromium 150.0.7871.187) — never in a general
    // browser. Before this line, the target was Vite's conservative default and
    // nothing tied the output to the engine. That was safe only by accident,
    // because our Chromium happened to be far newer than anything Vite targets.
    //
    // Pinning it makes the relationship structural in both directions: we stop
    // down-levelling syntax the engine supports natively, and a dependency that
    // emits syntax NEWER than our engine now fails at build time instead of
    // appearing as a blank overlay at runtime.
    //
    // ⛔ Bump this in lockstep with the CEF/Chromium pin. The owning doc for that
    // pin is cef-native/CLAUDE.md; read CEF_VERSION, never the Chromium version
    // alone, when checking which engine is actually staged.
    target: 'chrome150',
    rollupOptions: {
      output: {
        manualChunks: {
          'vendor-react': ['react', 'react-dom', 'react-router-dom'],
          'vendor-mui': ['@mui/material', '@mui/icons-material'],
        }
      }
    }
  }
})
