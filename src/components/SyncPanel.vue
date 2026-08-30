<!-- 《铃·记忆体》同步面板（AI-8 4.1）
     设备发现 / 手动连接 / 记忆集同步 / 进度条 / 同步历史 / 冲突策略 / 更新检查 -->
<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { useSyncStore } from '../stores/syncStore'
import { useMemoryStore } from '../stores/memoryStore'
import { useSettingStore } from '../stores/settingStore'
import { setSyncPassword } from '../utils/tauri'

const sync = useSyncStore()
const memory = useMemoryStore()
const setting = useSettingStore()

const selectedDevice = ref('')
const selectedSet = ref('default')
const manualIpInput = ref('')
const manualPortInput = ref(54546)
const updateResult = ref('')
const pwd = ref('')
const pwdMsg = ref('')
const policyMsg = ref('')

onMounted(async () => {
  sync.init().catch(() => {})
  await memory.loadSets().catch(() => {})
  sync.memorySets = memory.sets.length > 0 ? memory.sets : ['default']
  selectedSet.value = sync.memorySets[0]
  if (!setting.loaded) await setting.loadConfig().catch(() => {})
})

onUnmounted(() => sync.dispose())

function selectDevice(id: string) {
  selectedDevice.value = id
  if (sync.connectionMode === 'manual') sync.connectionMode = 'auto'
}

function switchMode(mode: 'auto' | 'manual') {
  sync.connectionMode = mode
  if (mode === 'manual' && manualIpInput.value) {
    sync.manualIp = manualIpInput.value
    sync.manualPort = manualPortInput.value
  }
}

async function startSync() {
  if (sync.connectionMode === 'manual') {
    if (!manualIpInput.value.trim()) {
      sync.message = '请先输入目标 IP'
      return
    }
    sync.manualIp = manualIpInput.value.trim()
    sync.manualPort = manualPortInput.value
    // 手动模式：确保设备在列表（连接成功后用返回的设备）
    await sync.manualConnect()
    if (sync.syncStatus === 'error') return
    const dev = sync.devices.find((d) => d.source === 'manual')
    if (!dev) {
      sync.message = '未找到手动设备，请重试'
      return
    }
    await sync.doSync(dev.device_id, selectedSet.value)
  } else {
    if (!selectedDevice.value) {
      sync.message = '请先选择目标设备'
      return
    }
    await sync.doSync(selectedDevice.value, selectedSet.value)
  }
}

async function doCheckUpdate() {
  updateResult.value = '检查中…'
  const res = await sync.checkUpdateNow(true)
  if (res?.has_update && res.version_info) {
    const v = res.version_info
    updateResult.value = `发现新版本 ${v.latest_version}（当前 ${v.current_version}）`
    if (v.release_notes) updateResult.value += `\n更新内容：${v.release_notes.slice(0, 200)}`
    updateResult.value += `\n下载：${v.release_url}`
  } else if (res) {
    updateResult.value = '当前已是最新版本 ✓'
  }
}

async function doSetPassword() {
  if (!pwd.value) return
  try {
    await setSyncPassword(pwd.value)
    pwdMsg.value = '✓ 同步主密码已设置（与配置加密共用同一密钥体系）'
    pwd.value = ''
  } catch (e) {
    pwdMsg.value = `✗ ${e}`
  }
  setTimeout(() => (pwdMsg.value = ''), 4000)
}

async function doSetPolicy(policy: 'newest' | 'local' | 'remote') {
  try {
    await sync.setPolicy(policy)
    policyMsg.value = `✓ 冲突策略已设为：${{ newest: '保留时间戳较新的版本', local: '始终保留本地', remote: '始终保留远程' }[policy]}`
  } catch (e) {
    policyMsg.value = `✗ ${e}`
  }
  setTimeout(() => (policyMsg.value = ''), 4000)
}
</script>

<template>
  <div class="sync-panel">
    <!-- 网络状态横幅 -->
    <div class="net-banner" :class="sync.networkStatus">
      <span>
        {{
          sync.networkStatus === 'online' ? '🟢 网络在线' :
          sync.networkStatus === 'offline' ? '🔴 网络已断开（已切换脚本模式）' :
          '⚪ 网络状态未知'
        }}
      </span>
    </div>

    <!-- 同步功能介绍 -->
    <section class="card intro">
      <div class="card-title">同步功能简介</div>
      <p class="hint">「同步」让多台设备上的铃·记忆体共享同一份记忆（对话历史与记忆集）。</p>
      <ul class="intro-list">
        <li><b>怎么用</b>：两台设备都装应用并<u>设置主密码</u> → 一台点「扫描设备」发现另一台（或手动输 IP）→ 选记忆集 → 点「开始同步」。</li>
        <li><b>前提</b>：双方都已设置并解锁主密码（记忆用 AES-256-GCM 加密传输）。</li>
        <li><b>端口</b>：UDP 54545（设备发现）+ TCP 54546（传输）。被防火墙/路由器隔离时请用「手动连接」。</li>
        <li><b>冲突</b>：同一记忆在两台都改过时，按下方「冲突策略」解决（较新/保留本地/保留远程）。</li>
        <li><b>方向</b>：点「开始同步」＝ 从目标设备<u>拉取</u>记忆到本机；双向互通需两台各点一次。</li>
      </ul>
    </section>

    <!-- 连接模式切换 -->
    <section class="card">
      <div class="card-title">连接方式</div>
      <div class="modes">
        <div class="mode" :class="{ sel: sync.connectionMode === 'auto' }" @click="switchMode('auto')">自动发现</div>
        <div class="mode" :class="{ sel: sync.connectionMode === 'manual' }" @click="switchMode('manual')">手动连接</div>
      </div>
      <p class="hint">UDP 广播可能被防火墙或 AP 隔离阻断，此时请切换「手动连接」直接输入目标 IP。</p>
    </section>

    <!-- 设备发现区 -->
    <section class="card">
      <div class="card-title">局域网设备</div>
      <div class="row">
        <button class="btn primary" :disabled="sync.syncStatus === 'discovering' || sync.syncStatus === 'syncing'" @click="sync.discover()">
          {{ sync.syncStatus === 'discovering' ? '扫描中…' : '扫描设备' }}
        </button>
        <button class="btn ghost" @click="sync.refreshStatus()">刷新状态</button>
      </div>

      <div v-if="sync.connectionMode === 'manual'" class="manual-box">
        <input v-model="manualIpInput" class="input" placeholder="目标 IP（如 192.168.1.100）" />
        <input v-model.number="manualPortInput" type="number" class="input port" placeholder="端口" />
        <button class="btn primary" :disabled="sync.syncStatus === 'discovering'" @click="sync.manualConnect()">连接</button>
      </div>

      <div class="devices">
        <div
          v-for="d in sync.devices"
          :key="d.device_id"
          class="device"
          :class="{ sel: selectedDevice === d.device_id }"
          @click="selectDevice(d.device_id)"
        >
          <span class="dev-name">🖥️ {{ d.device_name }}</span>
          <span class="dev-ip">{{ d.ip }}:{{ d.port }}</span>
          <span class="dev-src">{{ d.source === 'manual' ? '手动' : '自动' }}</span>
          <span class="dev-seen">{{ new Date(d.last_seen).toLocaleTimeString() }}</span>
        </div>
        <p v-if="sync.devices.length === 0" class="hint">暂无设备，点击「扫描设备」或在另一台设备上打开应用。</p>
      </div>
    </section>

    <!-- 同步操作区 -->
    <section class="card">
      <div class="card-title">同步记忆</div>
      <div class="row">
        <span class="label">目标：</span>
        <span class="target-name">
          {{
            sync.connectionMode === 'manual'
              ? (sync.manualIp || '未连接')
              : (sync.devices.find((d) => d.device_id === selectedDevice)?.device_name || '未选择')
          }}
        </span>
      </div>
      <div class="row">
        <span class="label">记忆集：</span>
        <select v-model="selectedSet" class="input">
          <option v-for="s in sync.memorySets" :key="s" :value="s">{{ s }}</option>
        </select>
      </div>
      <div class="row">
        <button class="btn primary big" :disabled="sync.syncStatus === 'syncing' || sync.syncStatus === 'discovering'" @click="startSync">
          {{ sync.syncStatus === 'syncing' ? '同步中…' : '开始同步' }}
        </button>
      </div>

      <!-- 进度条 -->
      <div v-if="sync.syncStatus === 'syncing' || sync.syncStatus === 'done'" class="progress-wrap">
        <div class="progress-bar"><div class="progress-fill" :style="{ width: `${sync.syncProgress * 100}%` }"></div></div>
        <span class="progress-text">{{ Math.round(sync.syncProgress * 100) }}%（{{ sync.message }}）</span>
      </div>
      <p v-if="sync.message && sync.syncStatus !== 'syncing' && sync.syncStatus !== 'done'" class="msg">{{ sync.message }}</p>
    </section>

    <!-- 同步历史 -->
    <section class="card">
      <div class="card-title">📜 同步历史</div>
      <div v-if="sync.history.length === 0" class="hint">暂无同步记录</div>
      <div v-for="(h, i) in sync.history" :key="i" class="history-item">
        <span class="hist-time">{{ new Date(h.time).toLocaleString() }}</span>
        <span class="hist-dev">→ {{ h.device }}</span>
        <span class="hist-set">[{{ h.set_name }}]</span>
        <span class="hist-count">{{ h.synced_count }} 条</span>
        <span class="hist-result" :class="h.success ? 'ok' : 'fail'">{{ h.success ? '成功' : '失败' }}</span>
      </div>
    </section>

    <!-- 加密与冲突策略 -->
    <section class="card">
      <div class="card-title">🔐 同步加密（与配置主密码共用密钥体系）</div>
      <p class="hint">
        主密码状态：{{ setting.hasMasterPassword ? (setting.unlocked ? '已设置 · 已解锁 ✅（可直接同步）' : '已设置 · 未解锁 ⚠️（请先在「模型」页解锁）') : '未设置 ⚠️（同步前需设置主密码）' }}
      </p>
      <div class="row">
        <input v-model="pwd" type="password" class="input" placeholder="设置/修改同步主密码" />
        <button class="btn primary" @click="doSetPassword">设置</button>
      </div>
      <div v-if="pwdMsg" class="msg">{{ pwdMsg }}</div>

      <div class="row" style="margin-top: 10px">
        <span class="label">冲突解决策略：</span>
        <button class="btn ghost" :class="{ on: sync.conflictPolicy === 'newest' }" @click="doSetPolicy('newest')">较新优先</button>
        <button class="btn ghost" :class="{ on: sync.conflictPolicy === 'local' }" @click="doSetPolicy('local')">保留本地</button>
        <button class="btn ghost" :class="{ on: sync.conflictPolicy === 'remote' }" @click="doSetPolicy('remote')">保留远程</button>
      </div>
      <div v-if="policyMsg" class="msg">{{ policyMsg }}</div>
    </section>

    <!-- 更新检查 -->
    <section class="card">
      <div class="card-title">🚀 版本更新检查</div>
      <div class="row">
        <button class="btn ghost" @click="doCheckUpdate">检查更新</button>
        <span class="hint" style="margin: 0">启动时自动检查 + 每 24 小时一次；GitHub 不可达时静默降级</span>
      </div>
      <pre v-if="updateResult" class="update-result">{{ updateResult }}</pre>
    </section>
  </div>
</template>

<style scoped>
.sync-panel {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.net-banner {
  padding: 8px 12px;
  border-radius: 10px;
  font-size: var(--fs-13);
  margin-bottom: 8px;
}
.net-banner.online { background: var(--success-bg); color: var(--success); }
.net-banner.offline { background: var(--danger-bg); color: #f85149; }
.net-banner.unknown { background: rgba(128, 128, 128, 0.12); color: var(--text-secondary); }
.card {
  background: var(--bg-bar, rgba(34, 32, 36, 0.85));
  border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
  border-radius: 14px;
  padding: 14px 16px;
  margin-bottom: 10px;
}
.card-title { font-weight: 600; font-size: var(--fs-14); margin-bottom: 8px; }
.hint { font-size: var(--fs-12); color: var(--text-secondary); margin: 0 0 8px; line-height: 1.6; }
.intro-list {
  margin: 4px 0 0;
  padding-left: 18px;
  font-size: var(--fs-12);
  color: var(--text-secondary);
  line-height: 1.9;
}
.intro-list li { margin-bottom: 2px; }
.intro-list b { color: var(--text-main); }
.row { display: flex; align-items: center; gap: 10px; margin-bottom: 8px; flex-wrap: wrap; }
.label { font-size: var(--fs-13); color: var(--text-secondary); }
.modes { display: flex; gap: 8px; margin-bottom: 10px; }
.mode {
  padding: 7px 14px; border-radius: 10px; cursor: pointer; border: 1px solid var(--border);
  font-size: var(--fs-13); background: transparent;
}
.mode.sel { border-color: var(--accent); background: var(--accent); color: var(--text-user); }
.manual-box { display: flex; gap: 8px; margin-bottom: 10px; flex-wrap: wrap; }
.input {
  padding: 7px 10px; border-radius: 8px; border: 1px solid var(--border);
  background: var(--input-bg); color: var(--text-main); font-size: var(--fs-13); flex: 1; min-width: 160px;
}
.input.port { flex: 0 0 90px; min-width: 90px; }
.devices { display: flex; flex-direction: column; gap: 6px; margin-top: 6px; }
.device {
  display: flex; align-items: center; gap: 10px; padding: 8px 12px;
  border-radius: 10px; border: 1px solid var(--border); cursor: pointer; font-size: var(--fs-13);
  transition: all 0.15s;
}
.device:hover { background: rgba(128, 128, 128, 0.1); }
.device.sel { border-color: var(--accent); background: rgba(255, 122, 148, 0.12); }
.dev-name { font-weight: 600; flex: 1; }
.dev-ip { color: var(--text-secondary); font-family: monospace; }
.dev-src { font-size: var(--fs-11); padding: 2px 8px; border-radius: 10px; background: rgba(128,128,128,0.15); }
.dev-seen { font-size: var(--fs-11); color: var(--text-secondary); }
.btn { padding: 6px 14px; border-radius: 8px; border: none; cursor: pointer; font-size: var(--fs-13); }
.btn.primary { background: var(--accent, #ff7a94); color: var(--text-user); }
.btn.ghost { background: rgba(128, 128, 128, 0.18); color: var(--text-main); }
.btn.big { padding: 9px 24px; font-size: var(--fs-14); }
.btn.on { border: 1px solid var(--accent); color: var(--accent); }
.btn:disabled { opacity: 0.5; cursor: not-allowed; }
.target-name { font-weight: 600; color: var(--accent); }
.progress-wrap { margin-top: 8px; }
.progress-bar {
  height: 8px; border-radius: 4px; background: rgba(128,128,128,0.2); overflow: hidden;
}
.progress-fill { height: 100%; background: linear-gradient(90deg, var(--accent), color-mix(in srgb, var(--accent) 60%, #fff)); transition: width 0.2s; }
.progress-text { font-size: var(--fs-12); color: var(--text-secondary); display: block; margin-top: 4px; }
.msg { font-size: var(--fs-12); margin-top: 6px; color: var(--accent, #ff7a94); }
.history-item {
  display: flex; align-items: center; gap: 10px; font-size: var(--fs-12); padding: 5px 0;
  border-bottom: 1px dashed rgba(128,128,128,0.15);
}
.hist-time { color: var(--text-secondary); }
.hist-dev { font-weight: 600; }
.hist-set { color: var(--text-secondary); }
.hist-count { color: var(--accent); }
.hist-result.ok { color: var(--success); }
.hist-result.fail { color: var(--danger, #f85149); }
.update-result {
  white-space: pre-wrap; font-size: var(--fs-12); color: var(--text-secondary);
  background: rgba(128,128,128,0.08); border-radius: 8px; padding: 8px; margin: 6px 0 0;
}
</style>
