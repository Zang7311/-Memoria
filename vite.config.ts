import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [vue()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      //    另外忽略原子写入的临时目录/文件：编辑器（含 AI 工具）保存时先写
      //    `.<name>.<pid>.<uuid>.tmpdir/<name>.tmp` 再改名，Windows 上 fs.watch 撞上
      //    这些影子文件会抛 EBUSY 直接把 vite 进程带崩（dev 会话反复失败的真凶）。
      ignored: ["**/src-tauri/**", "**/.*.tmpdir/**", "**/*.tmp", "**/.git/**"],
    },
  },
}));
