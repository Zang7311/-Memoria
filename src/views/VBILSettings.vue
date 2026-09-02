<!-- 《铃·记忆体》VBIL 虚拟形象互联设置
     总开关 / 响应模式 / 端口显示 / 在线客户端列表 / 白名单 / 扫描形象 -->
<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

interface OnlineClient {
  id: string
  name: string | null
  capabilities: string[]
  connected_at: string
  missed_pongs: number
}

interface VbilStatus {
  enabled: boolean
  mode: string
  port: number
  whitelist: string[]
}

interface SuspectedAvatar {
  hwnd: number
  title: string
  class: string
  process: string
}

const status = ref<VbilStatus>({ enabled: false, mode: 'rule_only', port: 54547, whitelist: [] })
const clients = ref<OnlineClient[]>([])
const suspected = ref<SuspectedAvatar[]>([])
const loading = ref(false)
const scanning = ref(false)
const msg = ref('')
const newWhitelistId = ref('')

async function loadStatus() {
  try {
    status.value = await invoke<VbilStatus>('get_vbil_status')
  } catch (e) {
    msg.value = `加载状态失败：${e}`
  }
}

async function loadClients() {
  try {
    clients.value = await invoke<OnlineClient[]>('get_online_clients')
  } catch (e) {
    msg.value = `加载客户端列表失败：${e}`
  }
}

async function refresh() {
  loading.value = true
  await Promise.all([loadStatus(), loadClients()])
  loading.value = false
}

async function toggleEnabled() {
  try {
    await invoke('set_vbil_enabled', { enabled: !status.value.enabled })
    status.value.enabled = !status.value.enabled
    msg.value = status.value.enabled ? '已开启虚拟形象互联' : '已关闭虚拟形象互联'
    setTimeout(() => (msg.value = ''), 3000)
  } catch (e) {
    msg.value = String(e)
  }
}

async function onModeChange(e: Event) {
  const mode = (e.target as HTMLSelectElement).value
  try {
    await invoke('set_vbil_mode', { mode })
    status.value.mode = mode
  } catch (e) {
    msg.value = String(e)
  }
}

async function doScan() {
  scanning.value = true
  try {
    suspected.value = await invoke<SuspectedAvatar[]>('scan_windows')
    if (suspected.value.length === 0) {
      msg.value = '未发现疑似虚拟形象窗口'
      setTimeout(() => (msg.value = ''), 3000)
    }
  } catch (e) {
    msg.value = `扫描失败：${e}`
  } finally {
    scanning.value = false
  }
}

async function addWhitelist() {
  const id = newWhitelistId.value.trim()
  if (!id) return
  const list = [...status.value.whitelist, id]
  try {
    await invoke('set_whitelist', { ids: list })
    status.value.whitelist = list
    newWhitelistId.value = ''
  } catch (e) {
    msg.value = String(e)
  }
}

async function removeWhitelist(id: string) {
  const list = status.value.whitelist.filter((x) => x !== id)
  try {
    await invoke('set_whitelist', { ids: list })
    status.value.whitelist = list
  } catch (e) {
    msg.value = String(e)
  }
}

onMounted(refresh)
</script>

<template>
  <div class="vbil-settings">
    <div class="section-title">虚拟形象互联（VBIL）</div>

    <div class="row">
      <label class="switch-wrap">
        <span class="label">总开关</span>
        <input type="checkbox" class="switch" :checked="status.enabled" @change="toggleEnabled" />
      </label>

      <label class="mode-wrap">
        <span class="label">响应模式</span>
        <select :value="status.mode" @change="onModeChange">
          <option value="off">关闭（off）</option>
          <option value="rule_only">固定回复（rule_only）</option>
          <option value="ai">AI 对话（ai）</option>
        </select>
      </label>

      <label class="port-wrap">
        <span class="label">端口</span>
        <code class="port">{{ status.port }}</code>
        <button class="op-btn" title="刷新" @click="refresh">刷新</button>
      </label>
    </div>

    <div v-if="msg" class="notice">{{ msg }}</div>

    <!-- 扫描形象 -->
    <div class="block">
      <div class="block-title">发现形象</div>
      <div class="actions">
        <button class="btn primary" :disabled="scanning" @click="doScan">
          {{ scanning ? '扫描中…' : '扫描窗口' }}
        </button>
      </div>
      <table v-if="suspected.length" class="table">
        <thead>
          <tr>
            <th>标题</th>
            <th>进程</th>
            <th>类名</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="a in suspected" :key="a.hwnd">
            <td>{{ a.title || '（无标题）' }}</td>
            <td><code class="code">{{ a.process }}</code></td>
            <td class="class-cell">{{ a.class }}</td>
          </tr>
        </tbody>
      </table>
      <div v-else class="empty">点「扫描窗口」寻找本机正在运行的虚拟形象</div>
    </div>

    <!-- 在线客户端 -->
    <div class="block">
      <div class="block-title">在线客户端</div>
      <table v-if="clients.length" class="table">
        <thead>
          <tr>
            <th>ID</th>
            <th>名称</th>
            <th>能力</th>
            <th>连接时间</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="c in clients" :key="c.id">
            <td><code class="code">{{ c.id }}</code></td>
            <td>{{ c.name || '—' }}</td>
            <td class="caps">{{ c.capabilities.join('、') || '—' }}</td>
            <td>{{ c.connected_at }}</td>
          </tr>
        </tbody>
      </table>
      <div v-else class="empty">暂无客户端连接</div>
    </div>

    <!-- 白名单 -->
    <div class="block">
      <div class="block-title">白名单（留空 = 允许全部）</div>
      <div class="whitelist-add">
        <input v-model="newWhitelistId" placeholder="客户端 ID" @keyup.enter="addWhitelist" />
        <button class="btn primary" @click="addWhitelist">添加</button>
      </div>
      <div v-if="status.whitelist.length" class="whitelist-tags">
        <span v-for="id in status.whitelist" :key="id" class="tag">
          {{ id }}
          <button class="tag-x" title="删除" @click="removeWhitelist(id)">×</button>
        </span>
      </div>
      <div v-else class="empty">未设置白名单，所有客户端均可接入</div>
    </div>
  </div>
</template>

<style scoped>
.vbil-settings {
  color: var(--text-main, #eee6e7);
}
.section-title {
  font-weight: 600;
  font-size: var(--fs-15);
  margin-bottom: 14px;
}
.row {
  display: flex;
  align-items: center;
  gap: 28px;
  flex-wrap: wrap;
  margin-bottom: 12px;
}
.switch-wrap,
.mode-wrap,
.port-wrap {
  display: flex;
  align-items: center;
  gap: 8px;
}
.label {
  font-size: var(--fs-13);
  color: var(--text-secondary, #9a9294);
}
.switch {
  width: 40px;
  height: 20px;
  accent-color: var(--accent, #ff7a94);
}
select,
input {
  padding: 5px 8px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--input-bg);
  color: var(--text-main);
}
.port {
  background: rgba(128, 128, 128, 0.15);
  padding: 3px 8px;
  border-radius: 6px;
  font-size: var(--fs-12);
}
.notice {
  margin-bottom: 10px;
  padding: 8px 12px;
  border-radius: 8px;
  background: rgba(255, 160, 60, 0.15);
  color: var(--warning);
  font-size: var(--fs-12);
}
.block {
  margin-top: 18px;
}
.block-title {
  font-weight: 600;
  font-size: var(--fs-13);
  margin-bottom: 10px;
  color: var(--text-main);
}
.actions {
  display: flex;
  gap: 8px;
  margin-bottom: 10px;
}
.btn {
  padding: 6px 14px;
  border-radius: 8px;
  border: none;
  cursor: pointer;
  font-size: var(--fs-13);
}
.btn.primary {
  background: var(--accent, #ff7a94);
  color: var(--text-user);
}
.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.op-btn {
  background: none;
  border: none;
  cursor: pointer;
  color: var(--accent, #ff7a94);
  font-size: var(--fs-12);
}
.table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--fs-12);
}
.table th {
  text-align: left;
  padding: 6px 8px;
  color: var(--text-secondary, #9a9294);
  border-bottom: 1px solid var(--border);
  font-weight: 500;
}
.table td {
  padding: 6px 8px;
  border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.08));
  vertical-align: middle;
}
.code {
  background: rgba(128, 128, 128, 0.15);
  padding: 2px 6px;
  border-radius: 5px;
  font-size: var(--fs-11);
  word-break: break-all;
}
.class-cell {
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.caps {
  color: var(--text-secondary, #9a9294);
}
.whitelist-add {
  display: flex;
  gap: 8px;
  margin-bottom: 10px;
}
.whitelist-add input {
  flex: 1;
  max-width: 320px;
}
.whitelist-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.tag {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 3px 10px;
  border-radius: 12px;
  background: rgba(128, 128, 128, 0.15);
  font-size: var(--fs-12);
}
.tag-x {
  background: none;
  border: none;
  cursor: pointer;
  color: var(--text-secondary, #9a9294);
  font-size: var(--fs-14);
  padding: 0;
}
.tag-x:hover {
  color: var(--danger);
}
.empty {
  text-align: center;
  color: var(--text-secondary, #9a9294);
  font-size: var(--fs-12);
  padding: 12px 0;
}
</style>
