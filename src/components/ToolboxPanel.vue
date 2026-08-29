<!-- 《铃·记忆体》工具箱面板（AI-6 任务 7 / 4.2）
     九宫格布局：图标 + 名称；点击执行系统命令；底部添加工具；长按/右键删除 -->
<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useDesktopStore } from '../stores/desktopStore'
import { useSettingStore } from '../stores/settingStore'
import type { ToolboxItem } from '../types'
import PixelArtPanel from './PixelArtPanel.vue'

const desktop = useDesktopStore()
const setting = useSettingStore()

// 关闭工具箱（父组件监听后隐藏）
const emit = defineEmits<{ (e: 'close'): void }>()

const executingId = ref<string | null>(null)
const feedback = ref<{ ok: boolean; text: string } | null>(null)
// —— 像素画板（批次3，前端组件）——
const showPixelArt = ref(false)
// —— 输出弹窗（执行有输出的工具时弹出完整内容，避免视觉盲区）——
const showOutput = ref(false)
const outputContent = ref('')
const outputTitle = ref('执行输出')

// —— 添加弹窗 ——
const showEditor = ref(false)
const editingItem = ref<ToolboxItem>({ id: '', name: '', icon: '🔧', command: '', enabled: true })
const editorError = ref('')

// —— 删除确认 ——
const showDeleteConfirm = ref(false)
const pendingDeleteId = ref('')

onMounted(() => {
  desktop.loadToolboxItems()
})

async function runItem(item: ToolboxItem) {
  if (executingId.value) return
  // WiFi 密码工具：提前声明（仅解自己的 + 不上传）
  if (item.id === 'wifi-pwd') {
    const ok = confirm(
      '⚠️ WiFi 密码查看声明：\n\n· 仅查看本机已保存的 WiFi 密码（你自己的网络）\n· 所有处理均在本机完成，不会上传任何数据\n\n是否继续？'
    )
    if (!ok) return
  }
  // XOR 破解声明（仅限授权数据）
  if (item.id === 'xor') {
    const ok = confirm(
      '⚠️ XOR 破解声明：\n\n· 仅用于破解你拥有或获授权测试的加密数据\n· 请勿用于非法破解他人数据\n\n是否继续？'
    )
    if (!ok) return
  }
  // 像素画板：前端交互组件，不执行命令
  if (item.id === 'pixel-art') {
    showPixelArt.value = true
    return
  }
  // 需要输入参数的工具：先弹输入框
  let input: string | undefined
  if (item.needs_input) {
    const v = window.prompt(item.input_label || `请输入「${item.name}」所需参数：`, item.input_placeholder || '')
    if (v === null) return // 用户取消
    input = v
  }
  executingId.value = item.id
  feedback.value = null
  const result = await desktop.executeToolboxItem(item.id, input)
  executingId.value = null
  if (result?.error) {
    feedback.value = { ok: false, text: result.error }
    // 失败也弹窗显示完整原因
    outputTitle.value = `✗ ${item.name} · 失败`
    outputContent.value = result.error
    showOutput.value = true
  } else if (result?.output) {
    feedback.value = { ok: true, text: `✓ ${item.name} 已执行（点查看输出）` }
    // 有输出则弹独立窗口完整展示，避免顶部小区域视觉盲区
    outputTitle.value = `📄 ${item.name} · 输出`
    outputContent.value = result.output
    showOutput.value = true
  } else {
    feedback.value = { ok: true, text: `✓ ${item.name} 已执行` }
  }
  // 反馈 4 秒后消失（不影响弹窗）
  setTimeout(() => (feedback.value = null), 4000)
}

// —— 添加 ——
function openAdd() {
  editingItem.value = { id: `user_${Date.now()}`, name: '', icon: '🔧', command: '', enabled: true }
  editorError.value = ''
  showEditor.value = true
}

async function saveItem() {
  const it = editingItem.value
  if (!it.name.trim()) {
    editorError.value = '请输入工具名称'
    return
  }
  if (!it.command.trim()) {
    editorError.value = '请输入要执行的命令'
    return
  }
  try {
    await desktop.addOrUpdateToolboxItem({ ...it, icon: it.icon.trim() || '🔧' })
    showEditor.value = false
  } catch (e) {
    editorError.value = String(e)
  }
}

// —— 删除（仅用户自定义条目，预设不可删）——
function requestDelete(item: ToolboxItem) {
  if (!item.id.startsWith('user_')) {
    feedback.value = { ok: false, text: '预设工具不可删除' }
    setTimeout(() => (feedback.value = null), 3000)
    return
  }
  pendingDeleteId.value = item.id
  showDeleteConfirm.value = true
}

async function confirmDelete() {
  await desktop.removeToolboxItem(pendingDeleteId.value)
  showDeleteConfirm.value = false
}
</script>

<template>
  <div class="toolbox-panel">
    <!-- 像素画板（批次3，全屏遮罩） -->
    <PixelArtPanel v-if="showPixelArt" @close="showPixelArt = false" />
    <div class="panel-header">
      <span class="panel-title">工具箱</span>
      <div class="header-btns">
        <button class="add-btn" title="添加工具" @click="openAdd">＋</button>
        <button class="close-btn" title="关闭工具箱" @click="emit('close')">✕</button>
      </div>
    </div>

    <!-- 执行反馈 -->
    <div v-if="feedback" class="feedback" :class="feedback.ok ? 'ok' : 'err'">
      {{ feedback.text }}
    </div>

    <!-- 九宫格 -->
    <div class="grid">
      <div
        v-for="item in desktop.toolboxItems"
        :key="item.id"
        class="cell"
        :class="{ running: executingId === item.id, disabled: !item.enabled }"
        @click="runItem(item)"
        @contextmenu.prevent="requestDelete(item)"
      >
        <span v-if="setting.emojiMode !== 'off'" class="cell-icon">{{ item.icon }}</span>
        <span class="cell-name">{{ item.name }}</span>
        <span v-if="executingId === item.id" class="spinner">⏳</span>
      </div>
      <div v-if="desktop.toolboxLoading" class="cell loading-cell">加载中…</div>
    </div>

    <div class="panel-footer">清理内存＝释放所有进程工作集＋清系统缓存（管理员模式更强）· 右键工具可删除（仅自定义）</div>

    <!-- 输出弹窗：完整显示命令输出（端口/清理等），可滚动，无视觉盲区 -->
    <div v-if="showOutput" class="modal-mask" @click.self="showOutput = false">
      <div class="modal output-modal">
        <div class="modal-title">{{ outputTitle }}</div>
        <pre class="output-pre">{{ outputContent }}</pre>
        <div class="modal-actions">
          <button class="btn confirm" @click="showOutput = false">关闭</button>
        </div>
      </div>
    </div>

    <!-- 添加/编辑弹窗 -->
    <div v-if="showEditor" class="modal-mask" @click.self="showEditor = false">
      <div class="modal">
        <div class="modal-title">{{ editingItem.id.startsWith('user_') && !editingItem.name ? '添加工具' : '编辑工具' }}</div>
        <label class="field">
          <span>名称</span>
          <input v-model="editingItem.name" placeholder="如：打开我的世界" />
        </label>
        <label class="field">
          <span>图标（emoji）</span>
          <input v-model="editingItem.icon" placeholder="如：🎮" />
        </label>
        <label class="field">
          <span>命令</span>
          <textarea v-model="editingItem.command" rows="3" placeholder="如：D:\game\mc.exe 或 notepad" />
        </label>
        <div v-if="editorError" class="editor-error">{{ editorError }}</div>
        <div class="modal-actions">
          <button class="btn cancel" @click="showEditor = false">取消</button>
          <button class="btn confirm" @click="saveItem">保存</button>
        </div>
      </div>
    </div>

    <!-- 删除确认 -->
    <div v-if="showDeleteConfirm" class="modal-mask" @click.self="showDeleteConfirm = false">
      <div class="modal small">
        <div class="modal-title">删除这个工具？</div>
        <div class="modal-actions">
          <button class="btn cancel" @click="showDeleteConfirm = false">取消</button>
          <button class="btn danger" @click="confirmDelete">删除</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.toolbox-panel {
  position: fixed;
  right: 16px;
  bottom: 100px;
  width: 300px;
  max-height: 82vh;
  display: flex;
  flex-direction: column;
  background: var(--bg-bar, rgba(30, 28, 32, 0.92));
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  border-radius: 14px;
  padding: 12px;
  backdrop-filter: blur(10px);
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.35);
  z-index: 500;
  color: var(--text-main, #eee6e7);
}
.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
  flex-shrink: 0;
}
.header-btns { display: flex; gap: 6px; }
.panel-title {
  font-weight: 600;
  font-size: 14px;
}
.add-btn {
  width: 26px;
  height: 26px;
  border-radius: 8px;
  border: none;
  background: var(--accent, #ff7a94);
  color: #fff;
  font-size: 16px;
  cursor: pointer;
}
.close-btn {
  width: 26px;
  height: 26px;
  border-radius: 8px;
  border: none;
  background: rgba(128, 128, 128, 0.2);
  color: var(--text-main, #eee6e7);
  font-size: 14px;
  line-height: 1;
  cursor: pointer;
}
.close-btn:hover { background: rgba(255, 107, 107, 0.35); color: #ff8b8b; }
.feedback {
  margin-bottom: 8px;
  padding: 6px 10px;
  border-radius: 8px;
  font-size: 12px;
  word-break: break-all;
  max-height: 60px;
  overflow: auto;
}
.feedback.ok { background: rgba(90, 200, 120, 0.18); color: #7fd99a; }
.feedback.err { background: rgba(255, 107, 107, 0.18); color: #ff8b8b; }
.grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 10px;
  flex: 1;
  overflow-y: auto;
  padding-right: 2px;
  min-height: 0;
}
.cell {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 14px 6px;
  min-height: 82px;
  border-radius: 12px;
  background: var(--input-bg, #2a272b);
  cursor: pointer;
  transition: transform 0.12s ease, background 0.12s ease;
  position: relative;
}
.cell:hover {
  transform: translateY(-2px);
  background: rgba(255, 138, 171, 0.18);
}
.cell.running { opacity: 0.6; }
.cell.disabled { opacity: 0.4; }
.cell-icon {
  font-size: 32px;
  line-height: 1;
}
.cell-name {
  font-size: 13px;
  text-align: center;
  word-break: break-all;
}
.spinner {
  position: absolute;
  top: 4px;
  right: 6px;
  font-size: 12px;
}
.loading-cell {
  color: var(--text-secondary, #9a9294);
  font-size: 12px;
}
.panel-footer {
  margin-top: 10px;
  text-align: center;
  font-size: 11px;
  color: var(--text-secondary, #9a9294);
}
.modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 600;
  padding: 16px;
  box-sizing: border-box;
}
.modal {
  width: 320px;
  max-height: 88vh;
  overflow-y: auto;
  box-sizing: border-box;
  background: var(--bg-bar, #262328);
  border: 1px solid var(--border, rgba(255, 255, 255, 0.15));
  border-radius: 14px;
  padding: 18px;
}
.modal.small { width: 240px; }
.output-modal { width: 560px; max-width: 90vw; }
.output-pre {
  margin: 0 0 12px;
  padding: 10px;
  max-height: 60vh;
  overflow: auto;
  background: var(--input-bg, #1e1c20);
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  border-radius: 8px;
  font-family: Consolas, 'Courier New', monospace;
  font-size: 12px;
  line-height: 1.6;
  color: var(--text-main, #eee6e7);
  white-space: pre-wrap;
  word-break: break-all;
}
.modal-title {
  font-weight: 600;
  margin-bottom: 14px;
}
.field {
  display: block;
  margin-bottom: 12px;
}
.field span {
  display: block;
  font-size: 12px;
  color: var(--text-secondary, #9a9294);
  margin-bottom: 4px;
}
.field input,
.field textarea {
  width: 100%;
  box-sizing: border-box;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid var(--border, rgba(255, 255, 255, 0.15));
  background: var(--input-bg, #2a272b);
  color: var(--text-main, #eee6e7);
  font-size: 13px;
  resize: vertical;
}
.editor-error {
  color: #ff8b8b;
  font-size: 12px;
  margin-bottom: 10px;
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}
.btn {
  padding: 6px 16px;
  border-radius: 8px;
  border: none;
  cursor: pointer;
  font-size: 13px;
}
.btn.cancel { background: rgba(128, 128, 128, 0.2); color: var(--text-main); }
.btn.confirm { background: var(--accent, #ff7a94); color: #fff; }
.btn.danger { background: #d9534f; color: #fff; }
</style>
