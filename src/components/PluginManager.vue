<!-- 《铃·记忆体》插件面板（抽屉内嵌版）
     标题与关闭由抽屉标签条负责，这里只负责内容：
     插件列表（启用/禁用、权限、卸载）/ 安装 / 终端命令 / 插件市场 -->
<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue'
import { usePluginStore } from '../stores/pluginStore'
import { PERMISSION_LABELS } from '../types'
import type { Plugin } from '../types'
import PluginStore from './PluginStore.vue'
import TheIcon from './TheIcon.vue'
import AppButton from './ui/AppButton.vue'
import AppCard from './ui/AppCard.vue'
import AppEmpty from './ui/AppEmpty.vue'
import AppInput from './ui/AppInput.vue'
import AppStatusDot from './ui/AppStatusDot.vue'
import AppSwitch from './ui/AppSwitch.vue'
import AppTabs from './ui/AppTabs.vue'
import { confirmDialog } from '../utils/dialog'
import { toastOk, toastErr } from '../utils/toast'

const store = usePluginStore()

// —— 插件分类 ——
function isTerminal(p: Plugin) {
  return p.manifest.main.trim() === ''
}
const normalPlugins = computed(() => store.plugins.filter((p) => !isTerminal(p)))
const terminalPlugins = computed(() => store.plugins.filter((p) => isTerminal(p)))

// 当前 tab：插件 / 终端命令 / 市场
const tab = ref<'plugins' | 'terminal' | 'store'>('plugins')
// 内容滚动容器：切页/开表单/弹权限确认时回到顶部，避免表单落在视口外
const scrollEl = ref<HTMLDivElement | null>(null)
function scrollToTop() {
  nextTick(() => {
    if (scrollEl.value) scrollEl.value.scrollTop = 0
  })
}
function setTab(id: string) {
  if (id === 'plugins' || id === 'terminal' || id === 'store') {
    tab.value = id
    scrollToTop()
  }
}
const tabItems = computed(() => [
  { id: 'plugins', label: '插件', count: normalPlugins.value.length },
  { id: 'terminal', label: '终端命令', count: terminalPlugins.value.length },
  { id: 'store', label: '市场' },
])

// —— 安装 ——
const showInstall = ref(false)
const installSource = ref('')
const installError = ref('')

function toggleInstall() {
  showInstall.value = !showInstall.value
  if (showInstall.value) scrollToTop()
  else installError.value = ''
}

async function onInstall() {
  const src = installSource.value.trim()
  if (!src) {
    installError.value = '请输入本地路径或 Git URL'
    return
  }
  // 拒绝明显含 shell 元字符的输入（防命令注入；后端需同步校验）
  if (/[;&|`$<>]/.test(src)) {
    installError.value = '路径包含非法字符，请检查后重试'
    return
  }
  installError.value = ''
  try {
    const newPlugin = await store.install(src)
    showInstall.value = false
    installSource.value = ''
    // 安装成功 → 弹权限确认单（用刚安装的插件对象）
    if (newPlugin && newPlugin.manifest.permissions?.length > 0) {
      openPermConfirm(newPlugin)
    } else {
      toastOk('安装成功，请先启用插件')
    }
  } catch (e) {
    installError.value = String(e)
  }
}

// —— 安装后权限确认单（安全关键，默认全不勾，最小权限原则） ——
const permConfirm = ref<Plugin | null>(null)
const permChecked = ref<Set<string>>(new Set())
const permConfirmBusy = ref(false)
const permConfirmMsg = ref('')

function openPermConfirm(p: Plugin) {
  permChecked.value = new Set()
  permConfirmMsg.value = ''
  permConfirm.value = p
  scrollToTop()
}
function togglePermCheck(perm: string) {
  const s = new Set(permChecked.value)
  if (s.has(perm)) s.delete(perm)
  else s.add(perm)
  permChecked.value = s
}
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

// —— 权限 ——
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
    toastErr(String(e))
  }
}
// 权限展开的插件 id
const permExpanded = ref<Set<string>>(new Set())
function togglePermExpand(id: string) {
  const s = new Set(permExpanded.value)
  if (s.has(id)) s.delete(id)
  else s.add(id)
  permExpanded.value = s
}

// 插件是否为 Hermes 兼容
function isHermes(p: Plugin) {
  return p.manifest.hermes_compatible
}
// 技能动作类型标签（纯文字，不用 emoji）
function actionLabel(action: string): string {
  if (action.startsWith('command:')) return '终端命令'
  if (action.startsWith('builtin:')) return '内置动作'
  return 'JS 技能'
}

// —— 卸载 / 启用禁用 ——
async function onUninstall(p: Plugin) {
  const ok = await confirmDialog(`确定卸载插件「${p.name}」吗？\n（插件目录将被删除）`, {
    danger: true,
    confirmText: '卸载',
  })
  if (!ok) return
  try {
    await store.uninstall(p.id)
  } catch (e) {
    toastErr(String(e))
  }
}
async function onToggle(p: Plugin) {
  try {
    await store.toggle(p)
  } catch (e) {
    toastErr(String(e))
  }
}

// —— 终端命令 ——
const showTermForm = ref(false)
const termName = ref('')
const termCommand = ref('')
const termDescription = ref('')
const termError = ref('')

function toggleTermForm() {
  showTermForm.value = !showTermForm.value
  if (showTermForm.value) scrollToTop()
  else termError.value = ''
}
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
    toastOk('终端命令已添加。system 权限默认禁用，需要时在下方手动开启')
  } catch (e) {
    termError.value = String(e)
  }
}

onMounted(() => {
  store.loadPlugins()
})
</script>

<template>
  <section class="plugin-panel">
    <!-- 顶部：分页标签（轻量操作条在下方） -->
    <div class="panel-tabs">
      <AppTabs :items="tabItems" :model-value="tab" @update:model-value="setTab" />
    </div>

    <!-- 轻量操作条：随 tab 变化 -->
    <div class="action-bar">
      <template v-if="tab === 'plugins'">
        <AppButton variant="ghost" size="md" @click="toggleInstall">
          <TheIcon :name="showInstall ? 'close' : 'add'" :size="14" />
          {{ showInstall ? '取消安装' : '安装插件' }}
        </AppButton>
        <AppButton
          variant="text"
          size="md"
          title="刷新插件列表"
          aria-label="刷新插件列表"
          @click="store.loadPlugins()"
        >
          <TheIcon name="refresh" :size="14" />
          刷新
        </AppButton>
      </template>
      <template v-else-if="tab === 'terminal'">
        <AppButton variant="ghost" size="md" @click="toggleTermForm">
          <TheIcon :name="showTermForm ? 'close' : 'add'" :size="14" />
          {{ showTermForm ? '收起表单' : '添加终端命令' }}
        </AppButton>
      </template>
      <span v-else class="bar-hint">市场里的插件装好后会出现在「插件」页</span>
    </div>

    <div ref="scrollEl" class="scroll">
      <div v-if="store.errorMsg" class="error">{{ store.errorMsg }}</div>
      <div v-if="store.isLoading" class="hint">加载中…</div>

      <!-- ============ 插件 ============ -->
      <template v-if="tab === 'plugins'">
        <!-- 安装表单：流内展开，不再用固定遮罩 -->
        <AppCard v-if="showInstall" title="安装插件" hint="本地目录或 Git 仓库">
          <div class="form">
            <AppInput
              v-model="installSource"
              placeholder="C:\path\to\plugin 或 https://github.com/xxx/plugin.git"
              @keyup.enter="onInstall"
            />
            <div v-if="installError" class="error">{{ installError }}</div>
            <div class="form-actions">
              <AppButton variant="text" size="sm" @click="toggleInstall">取消</AppButton>
              <AppButton variant="primary" size="sm" @click="onInstall">安装</AppButton>
            </div>
          </div>
        </AppCard>

        <!-- 安装后的权限确认单 -->
        <AppCard v-if="permConfirm" title="插件权限确认" :hint="permConfirm.name">
          <p class="form-tip">默认全不勾选（最小权限）。未授权的功能会提示权限不足。</p>
          <div class="perm-list">
            <label
              v-for="perm in permConfirm.manifest.permissions"
              :key="perm"
              class="perm-item"
            >
              <input
                type="checkbox"
                :checked="permChecked.has(perm)"
                @change="togglePermCheck(perm)"
              />
              <span class="perm-name">{{ PERMISSION_LABELS[perm] || perm }}</span>
            </label>
          </div>
          <div v-if="permConfirmMsg" class="error">{{ permConfirmMsg }}</div>
          <div class="form-actions">
            <AppButton variant="text" size="sm" :disabled="permConfirmBusy" @click="rejectPerms">
              全部拒绝
            </AppButton>
            <AppButton variant="ghost" size="sm" :disabled="permConfirmBusy" @click="confirmPerms">
              {{ permConfirmBusy ? '授权中…' : `确认授权（${permChecked.size}）` }}
            </AppButton>
          </div>
        </AppCard>

        <AppEmpty
          v-if="!store.isLoading && normalPlugins.length === 0 && !showInstall && !permConfirm"
          text="还没有安装插件"
          hint="支持本地插件目录或 Git 仓库"
          action-label="安装插件"
          @action="toggleInstall"
        />

        <div v-else class="list">
          <AppCard v-for="p in normalPlugins" :key="p.id" class="plugin-card">
            <div class="p-head">
              <div class="p-name">
                {{ p.name }}
                <span v-if="isHermes(p)" class="tag">Hermes</span>
              </div>
              <AppSwitch :model-value="p.enabled" @update:model-value="onToggle(p)" />
            </div>
            <div class="meta">v{{ p.version }} · {{ p.author }}</div>
            <p class="desc">{{ p.description || '（无描述）' }}</p>
            <div v-if="p.manifest.skills.length" class="skills">
              <span v-for="s in p.manifest.skills" :key="s.name" class="skill">
                {{ s.name }}
                <span class="skill-act">{{ actionLabel(s.action) }}</span>
              </span>
            </div>

            <!-- 权限展示与配置 -->
            <div class="perms">
              <button
                class="perm-toggle"
                type="button"
                :aria-expanded="permExpanded.has(p.id)"
                @click="togglePermExpand(p.id)"
              >
                <TheIcon name="lock" :size="12" />
                <span class="perm-title">
                  权限 {{ grantedOf(p).length }}/{{ p.manifest.permissions.length }}
                </span>
                <span class="perm-more">{{ permExpanded.has(p.id) ? '收起' : '展开' }}</span>
              </button>
              <div v-if="permExpanded.has(p.id)" class="perm-list">
                <div v-if="p.manifest.permissions.length === 0" class="dim">
                  该插件未声明任何权限（默认无权限）
                </div>
                <label v-for="perm in p.manifest.permissions" :key="perm" class="perm-item">
                  <input type="checkbox" :checked="hasPerm(p, perm)" @change="togglePerm(p, perm)" />
                  <span class="perm-name">{{ PERMISSION_LABELS[perm] || perm }}</span>
                </label>
              </div>
            </div>

            <div class="p-foot">
              <AppStatusDot
                :state="p.enabled ? 'ok' : 'idle'"
                :text="p.enabled ? '已启用' : '已停用'"
              />
              <button class="danger-btn" type="button" @click="onUninstall(p)">卸载</button>
            </div>
          </AppCard>
        </div>
      </template>

      <!-- ============ 终端命令 ============ -->
      <template v-else-if="tab === 'terminal'">
        <AppCard v-if="showTermForm" title="添加终端命令">
          <div class="form">
            <AppInput v-model="termName" placeholder="命令名（如 clean_temp）" />
            <AppInput v-model="termCommand" placeholder="命令内容（如 del /q %TEMP%\*）" />
            <AppInput v-model="termDescription" placeholder="描述（可选）" />
            <div v-if="termError" class="error">{{ termError }}</div>
            <div class="form-actions">
              <AppButton variant="text" size="sm" @click="toggleTermForm">取消</AppButton>
              <AppButton variant="ghost" size="sm" @click="onAddTerminal">保存</AppButton>
            </div>
          </div>
        </AppCard>

        <AppEmpty
          v-if="!store.isLoading && terminalPlugins.length === 0 && !showTermForm"
          text="还没有自定义终端命令"
          hint="把常用命令存成技能，铃需要时自己调用"
          action-label="添加终端命令"
          @action="toggleTermForm"
        />

        <div v-else class="list">
          <AppCard v-for="p in terminalPlugins" :key="p.id" class="plugin-card">
            <div class="p-head">
              <div class="p-name">{{ p.manifest.skills[0]?.name }}</div>
              <AppSwitch :model-value="p.enabled" @update:model-value="onToggle(p)" />
            </div>
            <p class="desc">{{ p.manifest.skills[0]?.description || '（无描述）' }}</p>
            <code class="cmd">{{ p.manifest.skills[0]?.action.replace('command:', '') }}</code>

            <div class="perms">
              <AppStatusDot
                :state="hasPerm(p, 'system') ? 'warn' : 'idle'"
                :text="hasPerm(p, 'system') ? '已授权 system' : '未授权 system，无法执行'"
              />
              <label class="perm-item">
                <input
                  type="checkbox"
                  :checked="hasPerm(p, 'system')"
                  @change="togglePerm(p, 'system')"
                />
                <span class="perm-name">system 系统命令</span>
                <span class="warn-text">高风险，默认禁用</span>
              </label>
            </div>

            <div class="p-foot">
              <AppStatusDot
                :state="p.enabled ? 'ok' : 'idle'"
                :text="p.enabled ? '已启用' : '已停用'"
              />
              <button class="danger-btn" type="button" @click="onUninstall(p)">删除</button>
            </div>
          </AppCard>
        </div>
      </template>

      <!-- ============ 插件市场 ============ -->
      <div v-else class="market">
        <PluginStore @installed="store.loadPlugins()" />
      </div>
    </div>
  </section>
</template>

<style scoped>
/* 流内布局：宽高由抽屉决定，面板自己不再有外框、固定宽度和定位 */
.plugin-panel {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
  color: var(--ui-text);
  background: transparent;
  font-size: var(--ui-fs-13);
}

/* 顶部分页与轻量操作条 */
.panel-tabs {
  flex: none;
  padding: var(--ui-sp-2) var(--ui-sp-2) 0;
}
.action-bar {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-2) var(--ui-sp-3);
  border-bottom: 1px solid var(--ui-border);
}
.bar-hint {
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  line-height: var(--ui-line);
}

/* 内容区自己滚动 */
.scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-3);
  padding: var(--ui-sp-3);
}
/* 滚动区里的块不参与压缩（否则卡片会被挤扁、内容被裁掉） */
.scroll > * {
  flex: none;
}

.list {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-3);
}

/* —— 插件卡片 —— */
.p-head {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-2);
}
.p-head :deep(.sw) {
  flex: none;
  padding: 0;
  gap: 0;
}
.p-name {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: var(--ui-sp-1);
  font-size: var(--ui-fs-14);
  font-weight: 600;
  color: var(--ui-text);
  word-break: break-word;
}
.tag {
  flex: none;
  padding: 0 var(--ui-sp-1);
  border-radius: var(--ui-r-pill);
  background: var(--ui-active);
  color: var(--ui-text-2);
  font-size: var(--ui-fs-11);
  font-weight: 400;
}
.meta {
  margin-top: var(--ui-sp-1);
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  overflow-wrap: anywhere;
}
.desc {
  margin: var(--ui-sp-1) 0 0;
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
  line-height: var(--ui-line);
  word-break: break-word;
}
.skills {
  display: flex;
  flex-wrap: wrap;
  gap: var(--ui-sp-1);
  margin-top: var(--ui-sp-2);
}
.skill {
  max-width: 100%;
  padding: 0 var(--ui-sp-1);
  border-radius: var(--ui-r-sm);
  background: var(--ui-hover);
  color: var(--ui-text-2);
  font-size: var(--ui-fs-11);
  line-height: var(--ui-line);
  overflow-wrap: anywhere;
}
.skill-act {
  margin-left: var(--ui-sp-1);
  color: var(--ui-text-3);
}
.cmd {
  display: block;
  margin-top: var(--ui-sp-2);
  padding: var(--ui-sp-2);
  border-radius: var(--ui-r-sm);
  background: var(--ui-hover);
  color: var(--ui-text-2);
  font-family: ui-monospace, Consolas, monospace;
  font-size: var(--ui-fs-11);
  line-height: var(--ui-line);
  word-break: break-all;
}

/* —— 权限 —— */
.perms {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
  margin-top: var(--ui-sp-3);
  padding-top: var(--ui-sp-2);
  border-top: 1px solid var(--ui-border);
}
.perm-toggle {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-1);
  width: 100%;
  padding: var(--ui-sp-1) 0;
  border: 0;
  border-radius: var(--ui-r-sm);
  background: transparent;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-12);
  text-align: left;
}
.perm-toggle:hover {
  color: var(--ui-text);
}
.perm-title {
  flex: 1;
  min-width: 0;
}
.perm-more {
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
}
.perm-list {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-1);
}
.perm-item {
  display: flex;
  align-items: center;
  gap: var(--ui-sp-1);
  padding: var(--ui-sp-1) var(--ui-sp-2);
  border-radius: var(--ui-r-sm);
  color: var(--ui-text);
  font-size: var(--ui-fs-12);
  line-height: var(--ui-line);
  cursor: pointer;
}
.perm-item:hover {
  background: var(--ui-hover);
}
.perm-item input {
  flex: none;
  margin: 0;
  accent-color: var(--ui-accent);
  cursor: pointer;
}
.perm-name {
  flex: 1;
  min-width: 0;
  word-break: break-word;
}
.warn-text {
  flex: none;
  font-size: var(--ui-fs-11);
  color: var(--ui-warn);
}
.dim {
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  line-height: var(--ui-line);
}

/* —— 卡片底部 —— */
.p-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--ui-sp-2);
  margin-top: var(--ui-sp-3);
  padding-top: var(--ui-sp-2);
  border-top: 1px solid var(--ui-border);
}
.danger-btn {
  border: 0;
  border-radius: var(--ui-r-sm);
  background: transparent;
  color: var(--ui-err);
  padding: var(--ui-sp-1) var(--ui-sp-2);
  font-size: var(--ui-fs-12);
  transition: background var(--ui-dur-1) var(--ui-ease);
}
.danger-btn:hover {
  background: var(--ui-hover);
}

/* —— 表单 —— */
.form {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-2);
}
.form-tip {
  margin: 0;
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  line-height: var(--ui-line);
}
.form-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--ui-sp-2);
}

.error {
  font-size: var(--ui-fs-12);
  color: var(--ui-err);
  line-height: var(--ui-line);
  word-break: break-word;
}
.hint {
  padding: var(--ui-sp-4) 0;
  text-align: center;
  font-size: var(--ui-fs-12);
  color: var(--ui-text-3);
}

/* 市场页由 PluginStore 组件渲染（该组件不在本批次改动范围），
   这里只补齐它在抽屉里的卡片与按钮样式，避免默认白底按钮看不清 */
.market :deep(.store) {
  display: flex;
  flex-direction: column;
  gap: var(--ui-sp-3);
}
.market :deep(.store-card) {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--ui-sp-2);
  padding: var(--ui-sp-3);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-md);
  background: var(--ui-raised);
}
.market :deep(.store-info) {
  min-width: 0;
}
.market :deep(.store-name) {
  font-size: var(--ui-fs-14);
  font-weight: 600;
  color: var(--ui-text);
  overflow-wrap: anywhere;
}
.market :deep(.store-tag) {
  padding: 0 var(--ui-sp-1);
  border-radius: var(--ui-r-pill);
  background: var(--ui-active);
  color: var(--ui-text-2);
  font-size: var(--ui-fs-11);
  font-weight: 400;
}
.market :deep(.store-meta) {
  margin-top: var(--ui-sp-1);
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  opacity: 1;
}
.market :deep(.store-desc) {
  margin-top: var(--ui-sp-1);
  font-size: var(--ui-fs-12);
  color: var(--ui-text-2);
  line-height: var(--ui-line);
  opacity: 1;
  overflow-wrap: anywhere;
}
.market :deep(.store-msg) {
  padding: var(--ui-sp-2);
  border-radius: var(--ui-r-sm);
  background: var(--ui-accent-soft);
  color: var(--ui-text);
  font-size: var(--ui-fs-12);
  white-space: pre-line;
}
.market :deep(.store-tip) {
  margin: 0;
  font-size: var(--ui-fs-11);
  color: var(--ui-text-3);
  line-height: var(--ui-line);
  text-align: center;
}
.market :deep(.primary-btn) {
  height: var(--ui-ctl-h);
  padding: 0 var(--ui-sp-3);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-sm);
  background: var(--ui-raised);
  color: var(--ui-text);
  font-size: var(--ui-fs-12);
  white-space: nowrap;
}
.market :deep(.primary-btn:hover) {
  background: var(--ui-hover);
  border-color: var(--ui-border-strong);
}
.market :deep(.primary-btn.small) {
  padding: 0 var(--ui-sp-2);
}
.market :deep(.more-btn) {
  height: var(--ui-ctl-h);
  border: 1px solid var(--ui-border);
  border-radius: var(--ui-r-sm);
  background: transparent;
  color: var(--ui-text-2);
  font-size: var(--ui-fs-12);
}
.market :deep(.more-btn:hover) {
  background: var(--ui-hover);
  color: var(--ui-text);
}
</style>
