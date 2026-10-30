<!-- 《铃·记忆体》同步面板（AI-8 4.1，挂在设置页「同步」分区）
     设备发现 / 手动连接 / 记忆集同步 / 进度 / 同步历史 / 冲突策略 / 更新检查
     样式全部走 src/styles/tokens.css 的 --ui-* 令牌；状态用 AppStatusDot、图标用 TheIcon，无 emoji。 -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import TheIcon from './TheIcon.vue'
import AppButton from './ui/AppButton.vue'
import AppCard from './ui/AppCard.vue'
import AppEmpty from './ui/AppEmpty.vue'
import AppField from './ui/AppField.vue'
import AppInput from './ui/AppInput.vue'
import AppStatusDot from './ui/AppStatusDot.vue'
import AppTabs from './ui/AppTabs.vue'
import { toastErr, toastOk } from '../utils/toast'
import { useSyncStore } from '../stores/syncStore'
import { useMemoryStore } from '../stores/memoryStore'
import { useSettingStore } from '../stores/settingStore'
import { setSyncPassword } from '../utils/tauri'

const sync = useSyncStore()
const memory = useMemoryStore()
const setting = useSettingStore()

/** AppStatusDot 的状态取值 */
type StatusState = 'ok' | 'warn' | 'err' | 'idle'

const selectedDevice = ref('')
const selectedSet = ref('default')
const manualIpInput = ref('')
const manualPortInput = ref(54546)
const updateResult = ref('')
const pwd = ref('')

const MODE_TABS: { id: string; label: string }[] = [
  { id: 'auto', label: '自动发现' },
  { id: 'manual', label: '手动连接' },
]
const POLICY_TABS: { id: string; label: string }[] = [
  { id: 'newest', label: '较新优先' },
  { id: 'local', label: '保留本地' },
  { id: 'remote', label: '保留远程' },
]
const POLICY_LABELS: Record<'newest' | 'local' | 'remote', string> = {
  newest: '保留时间戳较新的版本',
  local: '始终保留本地',
  remote: '始终保留远程',
}

const netState = computed<StatusState>(() =>
  sync.networkStatus === 'online' ? 'ok' : sync.networkStatus === 'offline' ? 'err' : 'idle',
)
const netText = computed(() =>
  sync.networkStatus === 'online'
    ? '网络在线'
    : sync.networkStatus === 'offline'
      ? '网络已断开（已切换到脚本模式）'
      : '网络状态未知',
)
const passwordState = computed<StatusState>(() =>
  setting.hasMasterPassword ? (setting.unlocked ? 'ok' : 'warn') : 'warn',
)
const passwordText = computed(() =>
  setting.hasMasterPassword
    ? setting.unlocked
      ? '已设置并已解锁，可以直接同步'
      : '已设置但未解锁，请先在「模型」页解锁'
    : '未设置，同步前需要先设置主密码',
)

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

/** AppTabs 传来的是 string，这里收窄成连接模式 */
function onModeTab(id: string) {
  switchMode(id === 'manual' ? 'manual' : 'auto')
}

/** 冲突策略标签的唯一数据源是 sync.conflictPolicy，点击即写入 */
function onPolicyTab(id: string) {
  if (id === 'newest' || id === 'local' || id === 'remote') doSetPolicy(id)
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
    updateResult.value = '当前已是最新版本'
  }
}

async function doSetPassword() {
  if (!pwd.value) return
  try {
    await setSyncPassword(pwd.value)
    pwd.value = ''
    toastOk('同步主密码已设置（与配置加密共用同一密钥体系）')
  } catch (e) {
    toastErr(`设置失败：${e}`)
  }
}

async function doSetPolicy(policy: 'newest' | 'local' | 'remote') {
  try {
    await sync.setPolicy(policy)
    toastOk(`冲突策略已设为：${POLICY_LABELS[policy]}`)
  } catch (e) {
    toastErr(`设置失败：${e}`)
  }
}
</script>

<template>
  <div class="page">
    <!-- 网络状态 -->
    <div class="net" :class="sync.networkStatus">
      <AppStatusDot :state="netState" />
      <span class="net-text">{{ netText }}</span>
    </div>

    <!-- 同步功能介绍 -->
    <AppCard title="同步功能简介">
      <p class="hint">「同步」让多台设备上的铃·记忆体共享同一份记忆（对话历史与记忆集）。</p>
      <ul class="intro-list">
        <li><b>怎么用</b>：两台设备都装应用并<u>设置主密码</u> → 一台点「扫描设备」发现另一台（或手动输 IP）→ 选记忆集 → 点「开始同步」。</li>
        <li><b>前提</b>：双方都已设置并解锁主密码（记忆用 AES-256-GCM 加密传输）。</li>
        <li><b>端口</b>：UDP 54545（设备发现）+ TCP 54546（传输）。被防火墙/路由器隔离时请用「手动连接」。</li>
        <li><b>冲突</b>：同一记忆在两台都改过时，按下方「冲突策略」解决（较新/保留本地/保留远程）。</li>
        <li><b>方向</b>：点「开始同步」＝ 从目标设备<u>拉取</u>记忆到本机；双向互通需两台各点一次。</li>
      </ul>
    </AppCard>

    <!-- 连接模式切换 -->
    <AppCard title="连接方式">
      <AppTabs :items="MODE_TABS" :model-value="sync.connectionMode" @update:model-value="onModeTab" />
      <p class="hint">UDP 广播可能被防火墙或 AP 隔离阻断，此时请切换「手动连接」直接输入目标 IP。</p>
    </AppCard>

    <!-- 设备发现区 -->
    <AppCard title="局域网设备">
      <div class="row">
        <AppButton
          variant="primary"
          :disabled="sync.syncStatus === 'discovering' || sync.syncStatus === 'syncing'"
          @click="sync.discover()"
        >
          {{ sync.syncStatus === 'discovering' ? '扫描中…' : '扫描设备' }}
        </AppButton>
        <AppButton @click="sync.refreshStatus()">刷新状态</AppButton>
      </div>

      <div v-if="sync.connectionMode === 'manual'" class="manual">
        <div class="manual-ip">
          <AppInput v-model="manualIpInput" placeholder="目标 IP（如 192.168.1.100）" />
        </div>
        <input v-model.number="manualPortInput" type="number" class="input manual-port" placeholder="端口" />
        <AppButton variant="primary" :disabled="sync.syncStatus === 'discovering'" @click="sync.manualConnect()">
          连接
        </AppButton>
      </div>

      <div class="devices">
        <div
          v-for="d in sync.devices"
          :key="d.device_id"
          class="device"
          :class="{ sel: selectedDevice === d.device_id }"
          @click="selectDevice(d.device_id)"
        >
          <TheIcon name="monitor" :size="14" class="dev-icon" />
          <span class="dev-name">{{ d.device_name }}</span>
          <span class="dev-ip">{{ d.ip }}:{{ d.port }}</span>
          <span class="dev-src">{{ d.source === 'manual' ? '手动' : '自动' }}</span>
          <span class="dev-seen">{{ new Date(d.last_seen).toLocaleTimeString() }}</span>
        </div>
        <AppEmpty
          v-if="sync.devices.length === 0"
          text="暂无设备"
          hint="点击「扫描设备」，或在另一台设备上打开应用。"
        />
      </div>
    </AppCard>

    <!-- 同步操作区 -->
    <AppCard title="同步记忆">
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
      <AppField label="记忆集">
        <select v-model="selectedSet" class="input select">
          <option v-for="s in sync.memorySets" :key="s" :value="s">{{ s }}</option>
        </select>
      </AppField>
      <div class="row">
        <AppButton
          variant="primary"
          :disabled="sync.syncStatus === 'syncing' || sync.syncStatus === 'discovering'"
          @click="startSync"
        >
          {{ sync.syncStatus === 'syncing' ? '同步中…' : '开始同步' }}
        </AppButton>
      </div>

      <!-- 进度条 -->
      <div v-if="sync.syncStatus === 'syncing' || sync.syncStatus === 'done'" class="progress">
        <div class="progress-bar">
          <div class="progress-fill" :style="{ width: `${sync.syncProgress * 100}%` }" />
        </div>
        <p class="progress-text">{{ Math.round(sync.syncProgress * 100) }}%（{{ sync.message }}）</p>
      </div>
      <p v-if="sync.message && sync.syncStatus !== 'syncing' && sync.syncStatus !== 'done'" class="msg">
        {{ sync.message }}
      </p>
    </AppCard>

    <!-- 同步历史 -->
    <AppCard title="同步历史">
      <AppEmpty v-if="sync.history.length === 0" text="暂无同步记录" hint="同步成功一次后，这里会记下时间、设备和条数。" />
      <div v-else class="hist-list">
        <div v-for="(h, i) in sync.history" :key="i" class="hist">
          <span class="hist-time">{{ new Date(h.time).toLocaleString() }}</span>
          <span class="hist-dev">→ {{ h.device }}</span>
          <span class="hist-set">[{{ h.set_name }}]</span>
          <span class="hist-count">{{ h.synced_count }} 条</span>
          <AppStatusDot :state="h.success ? 'ok' : 'err'" :text="h.success ? '成功' : '失败'" />
        </div>
      </div>
    </AppCard>

    <!-- 加密与冲突策略 -->
    <AppCard title="同步加密" hint="与配置主密码共用密钥体系">
      <p class="status-line">
        <span class="label">主密码状态</span>
        <AppStatusDot :state="passwordState" />
        <span class="status-text">{{ passwordText }}</span>
      </p>
      <div class="row">
        <div class="grow">
          <AppInput v-model="pwd" type="password" placeholder="设置/修改同步主密码" />
        </div>
        <AppButton variant="primary" @click="doSetPassword">设置</AppButton>
      </div>

      <AppField label="冲突解决策略" hint="同一记忆在两台设备都改过时，按这个策略决定保留哪一份">
        <AppTabs :items="POLICY_TABS" :model-value="sync.conflictPolicy" @update:model-value="onPolicyTab" />
      </AppField>
    </AppCard>

    <!-- 更新检查 -->
    <AppCard title="版本更新检查" hint="启动时自动检查，之后每 24 小时一次">
      <div class="row">
        <AppButton @click="doCheckUpdate">检查更新</AppButton>
        <span class="hint inline">GitHub 不可达时静默降级，不影响使用。</span>
      </div>
      <pre v-if="updateResult" class="update-result">{{ updateResult }}</pre>
    </AppCard>
  </div>
</template>

<style scoped>
.page {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-3);
  padding-bottom: var(--ui-sp-4);
}

/* ---------- 网络状态 ---------- */
.net {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-sm);
  background: var(--ui-panel);
  color: var(--ui-text-2);
  font-size: var(--ui-fs-12);
}
.net.online {
  border-color: color-mix(in srgb, var(--ui-ok) 32%, transparent);
  background: color-mix(in srgb, var(--ui-ok) 10%, transparent);
}
.net.offline {
  border-color: color-mix(in srgb, var(--ui-err) 32%, transparent);
  background: color-mix(in srgb, var(--ui-err) 10%, transparent);
}
.net-text {
  min-width: 0;
  overflow-wrap: anywhere;
}

/* ---------- 通用文本 ---------- */
.hint {
  margin: 0 0 var(--ui-sp-2);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text-2);
  overflow-wrap: anywhere;
}
.hint.inline {
  margin: 0;
}
.intro-list {
  margin: 0;
  padding-left: var(--ui-sp-5);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  color: var(--ui-text-2);
}
.intro-list li {
  margin-bottom: var(--ui-sp-1);
  overflow-wrap: anywhere;
}
.intro-list b {
  color: var(--ui-text);
}
.label {
  flex: none;
  font-size: var(--ui-fs-13);
  color: var(--ui-text-2);
}
.row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
  margin-bottom: var(--ui-sp-2);
  min-width: 0;
}
.grow {
  flex: 1 1 200px;
  min-width: 0;
}

/* ---------- 设备区 ---------- */
.manual {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
  margin-bottom: var(--ui-sp-2);
  min-width: 0;
}
.manual-ip {
  flex: 1 1 200px;
  min-width: 0;
}
.manual-port {
  flex: 0 1 96px;
  min-width: 80px;
}
.input {
  min-width: 0;
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-3);
  border: 1px solid var(--ui-border-strong);
  border-radius: var(--ui-r-sm);
  background: var(--ui-raised);
  color: var(--ui-text);
  font-size: var(--ui-fs-13);
}
.input:focus {
  border-color: var(--ui-accent);
}
.select {
  width: auto;
  max-width: 100%;
}
.devices {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-2);
  margin-top: var(--ui-sp-2);
  min-width: 0;
}
.device {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
  min-width: 0;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-sm);
  background: var(--ui-panel);
  font-size: var(--ui-fs-12);
  cursor: pointer;
  transition: background var(--ui-dur-1) var(--ui-ease), border-color var(--ui-dur-1) var(--ui-ease);
}
.device:hover {
  background: var(--ui-hover);
}
.device.sel {
  border-color: var(--ui-accent-line);
  background: var(--ui-accent-soft);
}
.dev-icon {
  flex: none;
  color: var(--ui-text-2);
}
.dev-name {
  flex: 1 1 120px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--ui-text);
  font-weight: 600;
}
.dev-ip {
  color: var(--ui-text-2);
  font-family: var(--font-mono);
}
.dev-src {
  flex: none;
  padding: 1px var(--ui-sp-2);
  border-radius: var(--ui-r-pill);
  background: var(--ui-hover);
  color: var(--ui-text-2);
  font-size: var(--ui-fs-11);
}
.dev-seen {
  color: var(--ui-text-3);
  font-size: var(--ui-fs-11);
}

/* ---------- 同步区 ---------- */
.target-name {
  min-width: 0;
  color: var(--ui-accent);
  font-weight: 600;
  overflow-wrap: anywhere;
}
.progress {
  margin-top: var(--ui-sp-2);
}
.progress-bar {
  height: 6px;
  border-radius: var(--ui-r-pill);
  background: var(--ui-hover);
  overflow: hidden;
}
.progress-fill {
  height: 100%;
  border-radius: var(--ui-r-pill);
  background: linear-gradient(90deg, var(--ui-accent), color-mix(in srgb, var(--ui-accent) 60%, var(--ui-canvas)));
  transition: width var(--ui-dur-2) var(--ui-ease);
}
.progress-text {
  margin: var(--ui-sp-1) 0 0;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  overflow-wrap: anywhere;
}
.msg {
  margin: var(--ui-sp-2) 0 0;
  color: var(--ui-accent);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  overflow-wrap: anywhere;
}

/* ---------- 同步历史 ---------- */
.hist-list {
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.hist {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
  min-width: 0;
  padding: var(--ui-sp-2) 0;
  border-bottom: 1px dashed var(--ui-border);
  font-size: var(--ui-fs-12);
}
.hist:last-child {
  padding-bottom: 0;
  border-bottom: 0;
}
.hist-time {
  color: var(--ui-text-2);
}
.hist-dev {
  min-width: 0;
  color: var(--ui-text);
  font-weight: 600;
  overflow-wrap: anywhere;
}
.hist-set {
  color: var(--ui-text-3);
}
.hist-count {
  color: var(--ui-accent);
}

/* ---------- 加密区 ---------- */
.status-line {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ui-sp-2);
  margin: 0 0 var(--ui-sp-2);
  min-width: 0;
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
}
.status-text {
  min-width: 0;
  overflow-wrap: anywhere;
}

/* ---------- 更新检查 ---------- */
.update-result {
  margin: var(--ui-sp-2) 0 0;
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-radius: var(--ui-r-sm);
  background: var(--ui-hover);
  color: var(--ui-text-2);
  font-family: var(--font-mono);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
</style>
