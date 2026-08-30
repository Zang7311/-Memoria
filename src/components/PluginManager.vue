<!-- 《铃·记忆体》插件管理界面（AI-5）
     插件列表（启用开关/权限配置/卸载）+ 安装插件对话框 + 终端命令扩展 + 插件市场入口 -->
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { usePluginStore } from '../stores/pluginStore'
import { PERMISSION_LABELS } from '../types'
import type { Plugin } from '../types'
import PluginStore from './PluginStore.vue'

const store = usePluginStore()

// 面板展开/收起
const expanded = ref(true)

// 安装对话框
const showInstall = ref(false)
const installSource = ref('')
const installError = ref('')

// 终端命令表单
const showTermForm = ref(false)
const termName = ref('')
const termCommand = ref('')
const termDescription = ref('')
const termError = ref('')

// 权限展开的插件 id
const permExpanded = ref<Set<string>>(new Set())

function togglePermExpand(id: string) {
  const s = new Set(permExpanded.value)
  if (s.has(id)) s.delete(id)
  else s.add(id)
  permExpanded.value = s
}

// 当前 tab：插件 / 终端命令 / 市场
const tab = ref<'plugins' | 'terminal' | 'store'>('plugins')

onMounted(() => {
  store.loadPlugins()
})

// 插件是否已授权某权限（后端 Plugin.granted 同步）
function grantedOf(p: Plugin): string[] {
  return p.granted || []
}

function hasPerm(p: Plugin, perm: string): boolean {
  return grantedOf(p).includes(perm)
}

async function togglePerm(p: Plugin, perm: string) {
  try {
    const updated = await store.setPermission(p.id, perm, !hasPerm(p, perm))
    // store 已把更新后的插件写回列表，直接同步本地引用
    Object.assign(p, updated)
  } catch (e) {
    alert(String(e))
  }
}

// 插件是否为终端命令（main 为空）
function isTerminal(p: Plugin) {
  return p.manifest.main.trim() === ''
}

// 插件是否为 Hermes 兼容
function isHermes(p: Plugin) {
  return p.manifest.hermes_compatible
}

// 技能动作类型标签
function actionLabel(_p: Plugin, action: string): string {
  if (action.startsWith('command:')) return '🖥️ 终端命令'
  if (action.startsWith('builtin:')) return '⚙️ 内置动作'
  return '🧩 JS 技能'
}

// 安装
async function onInstall() {
  const src = installSource.value.trim()
  if (!src) {
    installError.value = '请输入本地路径或 Git URL'
    return
  }
  installError.value = ''
  try {
    await store.install(src)
    showInstall.value = false
    installSource.value = ''
    // 安装成功 → 弹权限确认单（P2：安全关键，默认全不勾，最小权限原则）
    const installed = store.plugins.find((p) => p.manifest.permissions?.length)
    if (installed && installed.manifest.permissions.length > 0) {
      openPermConfirm(installed)
    } else {
      alert('安装成功！请先启用插件（如无权限申请则直接可用）。')
    }
  } catch (e) {
    installError.value = String(e)
  }
}

// —— P2：安装后权限确认单 ——
const permConfirm = ref<Plugin | null>(null)
const permChecked = ref<Set<string>>(new Set())
const permConfirmBusy = ref(false)
const permConfirmMsg = ref('')

function openPermConfirm(p: Plugin) {
  permChecked.value = new Set()
  permConfirmMsg.value = ''
  permConfirm.value = p
}
function togglePermCheck(perm: string) {
  const s = new Set(permChecked.value)
  if (s.has(perm)) s.delete(perm)
  else s.add(perm)
  permChecked.value = s
}
// 确认授权（勾选的权限批量授予）
async function confirmPerms() {
  const p = permConfirm.value
  if (!p) return
  permConfirmBusy.value = true
  permConfirmMsg.value = ''
  try {
    for (const perm of p.manifest.permissions) {
      if (permChecked.value.has(perm) && !hasPerm(p, perm)) {
        const updated = await store.setPermission(p.id, perm, true)
        Object.assign(p, updated)
      }
    }
    permConfirm.value = null
  } catch (e) {
    permConfirmMsg.value = `授权失败：${String(e)}`
  } finally {
    permConfirmBusy.value = false
  }
}
// 全部拒绝（不授权，插件默认无权限可用）
function rejectPerms() {
  permConfirm.value = null
}

// 卸载（带确认）
async function onUninstall(p: Plugin) {
  if (confirm(`确定卸载插件「${p.name}」吗？\n（插件目录将被删除）`)) {
    try {
      await store.uninstall(p.id)
    } catch (e) {
      alert(String(e))
    }
  }
}

// 启用/禁用
async function onToggle(p: Plugin) {
  try {
    await store.toggle(p)
  } catch (e) {
    alert(String(e))
  }
}

// 终端命令
async function onAddTerminal() {
  const name = termName.value.trim()
  const cmd = termCommand.value.trim()
  termError.value = ''
  if (!name || !cmd) {
    termError.value = '命令名与命令内容不能为空'
    return
  }
  try {
    await store.addTerminal(name, cmd, termDescription.value.trim())
    termName.value = ''
    termCommand.value = ''
    termDescription.value = ''
    showTermForm.value = false
    alert('终端命令已添加！注意：system 权限默认禁用，需在下方手动开启后才能执行。')
  } catch (e) {
    termError.value = String(e)
  }
}

const normalPlugins = computed(() => store.plugins.filter((p) => !isTerminal(p)))
const terminalPlugins = computed(() => store.plugins.filter((p) => isTerminal(p)))
</script>

<template>
  <aside class="plugin-panel" :class="{ collapsed: !expanded }">
    <div class="panel-header">
      <span class="panel-title">🧩 插件管理</span>
      <button class="icon-btn" @click="expanded = !expanded">{{ expanded ? '▶' : '◀' }}</button>
    </div>

    <template v-if="expanded">
      <!-- Tab 切换 -->
      <div class="tabs">
        <button :class="{ active: tab === 'plugins' }" @click="tab = 'plugins'">插件 ({{ normalPlugins.length }})</button>
        <button :class="{ active: tab === 'terminal' }" @click="tab = 'terminal'">终端命令 ({{ terminalPlugins.length }})</button>
        <button :class="{ active: tab === 'store' }" @click="tab = 'store'">市场</button>
      </div>

      <div v-if="store.errorMsg" class="error-box">{{ store.errorMsg }}</div>

      <!-- ============ 插件列表 ============ -->
      <div v-if="tab === 'plugins'" class="plugin-list">
        <button class="primary-btn" @click="showInstall = true">＋ 安装插件</button>

        <div v-if="normalPlugins.length === 0" class="empty-tip">暂无插件，点击上方按钮安装～</div>

        <div v-for="p in normalPlugins" :key="p.id" class="plugin-card">
          <div class="card-top">
            <div class="card-info">
              <div class="card-name">
                {{ p.name }}
                <span v-if="isHermes(p)" class="tag hermes">Hermes</span>
              </div>
              <div class="card-meta">v{{ p.version }} · {{ p.author }}</div>
              <div class="card-desc">{{ p.description || '（无描述）' }}</div>
              <div class="card-skills">
                <span v-for="s in p.manifest.skills" :key="s.name" class="skill-tag" :title="s.description">
                  {{ s.name }}
                  <em>{{ actionLabel(p, s.action) }}</em>
                </span>
              </div>
            </div>
            <div class="card-actions">
              <label class="switch">
                <input type="checkbox" :checked="p.enabled" @change="onToggle(p)" />
                <span class="slider"></span>
              </label>
              <button class="mini-btn danger" @click="onUninstall(p)">卸载</button>
            </div>
          </div>

          <!-- 权限区 -->
          <div class="perm-area">
            <div class="perm-title" @click="togglePermExpand(p.id)">
              权限配置 <span class="perm-toggle">{{ permExpanded.has(p.id) ? '▲' : '▼' }}</span>
            </div>
            <div v-if="permExpanded.has(p.id)" class="perm-list">
              <div v-if="p.manifest.permissions.length === 0" class="perm-empty">该插件未声明任何权限（默认无权限）</div>
              <label v-for="perm in p.manifest.permissions" :key="perm" class="perm-item">
                <input type="checkbox" :checked="hasPerm(p, perm)" @change="togglePerm(p, perm)" />
                <span :class="{ danger: perm === 'system' }">
                  {{ PERMISSION_LABELS[perm] || perm }}
                  <em v-if="perm === 'system'">⚠️ 高风险</em>
                </span>
              </label>
            </div>
          </div>
        </div>
      </div>

      <!-- ============ 终端命令 ============ -->
      <div v-if="tab === 'terminal'" class="plugin-list">
        <button class="primary-btn" @click="showTermForm = !showTermForm">{{ showTermForm ? '收起表单' : '＋ 添加终端命令' }}</button>

        <div v-if="showTermForm" class="term-form">
          <input v-model="termName" placeholder="命令名（如 clean_temp）" />
          <input v-model="termCommand" placeholder="命令内容（如 del /q %TEMP%\*）" />
          <input v-model="termDescription" placeholder="描述（可选）" />
          <div v-if="termError" class="error-box">{{ termError }}</div>
          <button class="primary-btn small" @click="onAddTerminal">✓ 保存命令</button>
        </div>

        <div v-if="terminalPlugins.length === 0 && !showTermForm" class="empty-tip">暂无自定义终端命令～</div>

        <div v-for="p in terminalPlugins" :key="p.id" class="plugin-card">
          <div class="card-top">
            <div class="card-info">
              <div class="card-name">{{ p.manifest.skills[0]?.name }}</div>
              <div class="card-desc">{{ p.manifest.skills[0]?.description || '（无描述）' }}</div>
              <code class="cmd-code">{{ p.manifest.skills[0]?.action.replace('command:', '') }}</code>
            </div>
            <div class="card-actions">
              <label class="switch">
                <input type="checkbox" :checked="p.enabled" @change="onToggle(p)" />
                <span class="slider"></span>
              </label>
              <button class="mini-btn danger" @click="onUninstall(p)">删除</button>
            </div>
          </div>
          <div class="perm-area">
            <div class="perm-title">权限：{{ hasPerm(p, 'system') ? '✅ 已授权 system' : '⛔ 未授权（无法执行）' }}</div>
            <div class="perm-list">
              <label class="perm-item">
                <input type="checkbox" :checked="hasPerm(p, 'system')" @change="togglePerm(p, 'system')" />
                <span class="danger">system 系统命令 <em>⚠️ 高风险，默认禁用</em></span>
              </label>
            </div>
          </div>
        </div>
      </div>

      <!-- ============ 插件市场 ============ -->
      <div v-if="tab === 'store'" class="plugin-list">
        <PluginStore @installed="store.loadPlugins()" />
      </div>
    </template>

    <!-- 安装对话框 -->
    <div v-if="showInstall" class="modal-mask" @click.self="showInstall = false">
      <div class="modal">
        <h3>安装插件</h3>
        <p class="modal-tip">输入本地插件目录路径，或 Git 仓库 URL</p>
        <input v-model="installSource" placeholder="C:\path\to\plugin 或 https://github.com/xxx/plugin.git" @keyup.enter="onInstall" />
        <div v-if="installError" class="error-box">{{ installError }}</div>
        <div class="modal-actions">
          <button class="mini-btn" @click="showInstall = false">取消</button>
          <button class="primary-btn small" @click="onInstall">安装</button>
        </div>
      </div>
    </div>

    <!-- P2：安装后权限确认单（安全关键，默认全不勾，最小权限原则） -->
    <div v-if="permConfirm" class="modal-mask">
      <div class="modal perm-modal">
        <h3>🔐 插件权限确认</h3>
        <p class="modal-tip">
          插件「<b>{{ permConfirm.name }}</b>」申请以下权限：
        </p>
        <div class="perm-list">
          <label
            v-for="perm in permConfirm.manifest.permissions"
            :key="perm"
            class="perm-item"
            :class="{ high: perm === 'system' }"
          >
            <input
              type="checkbox"
              :checked="permChecked.has(perm)"
              @change="togglePermCheck(perm)"
            />
            <span class="perm-name">{{ PERMISSION_LABELS[perm] || perm }}</span>
            <span v-if="perm === 'system'" class="perm-warn">⚠️ 高风险</span>
          </label>
        </div>
        <p class="modal-tip">默认全不勾选（最小权限）。未授权的功能会提示权限不足。</p>
        <div v-if="permConfirmMsg" class="error-box">{{ permConfirmMsg }}</div>
        <div class="modal-actions">
          <button class="mini-btn" :disabled="permConfirmBusy" @click="rejectPerms">全部拒绝</button>
          <button class="primary-btn small" :disabled="permConfirmBusy" @click="confirmPerms">
            {{ permConfirmBusy ? '授权中…' : `确认授权（${permChecked.size}）` }}
          </button>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.plugin-panel {
  width: 340px;
  border-left: 1px solid var(--border, rgba(128, 128, 128, 0.2));
  background: var(--bg-bar, rgba(255, 255, 255, 0.6));
  display: flex;
  flex-direction: column;
  min-height: 0;
  transition: width 0.2s ease;
}
.plugin-panel.collapsed {
  width: 44px;
}
.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border, rgba(128, 128, 128, 0.2));
}
.panel-title {
  font-weight: 600;
  font-size: var(--fs-14);
}
.icon-btn {
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: var(--fs-12);
  opacity: 0.7;
}
.tabs {
  display: flex;
  gap: 4px;
  padding: 8px 10px;
  border-bottom: 1px solid var(--border, rgba(128, 128, 128, 0.2));
}
.tabs button {
  flex: 1;
  border: none;
  background: transparent;
  padding: 6px 4px;
  border-radius: 6px;
  cursor: pointer;
  font-size: var(--fs-12);
  color: var(--text-main, #222);
  opacity: 0.7;
}
.tabs button.active {
  background: rgba(255, 182, 193, 0.35);
  opacity: 1;
  font-weight: 600;
}
.plugin-list {
  flex: 1;
  overflow-y: auto;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.plugin-card {
  border: 1px solid var(--border, rgba(128, 128, 128, 0.25));
  border-radius: 10px;
  padding: 10px;
  background: var(--bg-main, rgba(255, 255, 255, 0.5));
}
.card-top {
  display: flex;
  justify-content: space-between;
  gap: 8px;
}
.card-name {
  font-weight: 600;
  font-size: var(--fs-14);
  display: flex;
  align-items: center;
  gap: 6px;
}
.tag {
  font-size: var(--fs-10);
  padding: 1px 6px;
  border-radius: 8px;
  font-weight: 400;
}
.tag.hermes {
  background: var(--info-bg);
  color: var(--info);
}
.card-meta {
  font-size: var(--fs-11);
  opacity: 0.6;
  margin-top: 2px;
}
.card-desc {
  font-size: var(--fs-12);
  opacity: 0.8;
  margin-top: 4px;
  line-height: 1.4;
}
.card-skills {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 6px;
}
.skill-tag {
  font-size: var(--fs-11);
  background: rgba(255, 182, 193, 0.25);
  border-radius: 6px;
  padding: 2px 6px;
}
.skill-tag em {
  font-style: normal;
  opacity: 0.55;
  margin-left: 4px;
}
.card-actions {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
}
.switch {
  position: relative;
  display: inline-block;
  width: 36px;
  height: 20px;
}
.switch input {
  display: none;
}
.slider {
  position: absolute;
  inset: 0;
  background: var(--text-secondary, #ccc);
  border-radius: 20px;
  transition: 0.2s;
  cursor: pointer;
}
.slider::before {
  content: '';
  position: absolute;
  width: 14px;
  height: 14px;
  left: 3px;
  top: 3px;
  background: #ffffff; /* 开关滑块固定白色 */
  border-radius: 50%;
  transition: 0.2s;
}
.switch input:checked + .slider {
  background: var(--accent);
}
.switch input:checked + .slider::before {
  transform: translateX(16px);
}
.mini-btn {
  border: 1px solid var(--border, rgba(128, 128, 128, 0.4));
  background: transparent;
  border-radius: 6px;
  padding: 2px 8px;
  font-size: var(--fs-11);
  cursor: pointer;
}
.mini-btn.danger {
  color: var(--danger);
  border-color: rgba(220, 38, 38, 0.4);
}
.mini-btn.danger:hover {
  background: rgba(220, 38, 38, 0.08);
}
.primary-btn {
  border: none;
  background: linear-gradient(135deg, color-mix(in srgb, var(--accent) 45%, #fff), var(--accent));
  color: var(--text-user);
  border-radius: 8px;
  padding: 7px 12px;
  font-size: var(--fs-13);
  cursor: pointer;
  font-weight: 600;
}
.primary-btn.small {
  padding: 5px 10px;
  font-size: var(--fs-12);
}
.primary-btn:hover {
  filter: brightness(1.05);
}
.perm-area {
  margin-top: 8px;
  border-top: 1px dashed var(--border, rgba(128, 128, 128, 0.3));
  padding-top: 6px;
}
.perm-title {
  font-size: var(--fs-12);
  opacity: 0.7;
  cursor: pointer;
  display: flex;
  justify-content: space-between;
}
.perm-toggle {
  font-size: var(--fs-10);
}
.perm-list {
  margin-top: 6px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.perm-item {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--fs-12);
  cursor: pointer;
}
.perm-item .danger {
  color: var(--danger);
}
.perm-item em {
  font-style: normal;
  font-size: var(--fs-10);
  opacity: 0.6;
  margin-left: 4px;
}
.perm-empty {
  font-size: var(--fs-12);
  opacity: 0.6;
}
.empty-tip {
  text-align: center;
  color: var(--text-main, #888);
  font-size: var(--fs-12);
  padding: 20px 0;
}
.error-box {
  background: rgba(220, 38, 38, 0.1);
  color: var(--danger);
  border-radius: 6px;
  padding: 6px 10px;
  font-size: var(--fs-12);
  margin: 8px 10px 0;
  word-break: break-all;
}
.term-form {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.term-form input,
.modal input {
  border: 1px solid var(--border, rgba(128, 128, 128, 0.4));
  background: var(--bg-main, #fff);
  color: var(--text-main, #222);
  border-radius: 6px;
  padding: 7px 10px;
  font-size: var(--fs-12);
}
.cmd-code {
  display: block;
  background: rgba(0, 0, 0, 0.06);
  border-radius: 4px;
  padding: 3px 6px;
  font-size: var(--fs-11);
  margin-top: 4px;
  word-break: break-all;
}
.modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.modal {
  background: var(--bg-main, #fff);
  border-radius: 12px;
  padding: 18px;
  width: 380px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.modal h3 {
  margin: 0;
  font-size: var(--fs-15);
}
.modal-tip {
  margin: 0;
  font-size: var(--fs-12);
  opacity: 0.7;
}
.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
/* —— P2：权限确认单样式 —— */
.perm-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 220px;
  overflow-y: auto;
}
.perm-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  border-radius: 8px;
  border: 1px solid var(--border, rgba(128, 128, 128, 0.25));
  cursor: pointer;
  font-size: var(--fs-13);
  transition: background 0.15s;
}
.perm-item:hover { background: rgba(128, 128, 128, 0.08); }
.perm-item.high { border-color: rgba(217, 83, 79, 0.4); background: rgba(217, 83, 79, 0.06); }
.perm-item input { accent-color: var(--accent, #ff7a94); cursor: pointer; }
.perm-name { flex: 1; }
.perm-warn {
  font-size: var(--fs-10);
  color: var(--danger, #d9534f);
  font-weight: 600;
}
</style>
