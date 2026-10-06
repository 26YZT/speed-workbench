# Codex 开发说明（喂给 Codex 的任务书）

> 本文件是给 AI 编程助手（Codex / Claude Code / Cursor 等）的任务说明。
> 使用方式：把本文件放到项目根目录，然后对助手说"先读本文件，再执行任务 A/B/C"。

---

## 〇、任务 A 定制参数（本次已确定的值，直接按此执行）

| 项 | 值 |
|---|---|
| 产品名（APP_DISPLAY_NAME） | Speed Workbench |
| 简称（APP_MONOGRAM） | SW |
| 内置助手名（BUTLER_DISPLAY_NAME） | Speed 助手 |
| bundleId（appId） | com.speedworkbench.app |
| 产品定位（description 文案） | 一款快速构建 AI 产品原型的工作台/产品工厂 |
| 图标 | 本次【不改】，改动清单中标注"图标待 AI 生成后替换"，禁止用占位图 |

补充要求：
1. `AionUi/package.json` 的 `description` 改为上面"产品定位"的文案。
2. 用户可见文案里的"产品经理工作台"改为"Speed Workbench"，但 `LICENSE`、`NOTICE`、`docs/AUTHORS.md` 里的上游归属声明**不改**。
3. `UPSTREAM_UPDATE_ENABLED` 保持 `false`；`electron-builder.yml` 的 `publishAutoUpdate` 设为 `false`。

---

## 一、项目背景（先理解，再动手）

这是一个**二次开发项目**，原始代码是"产品经理工作台"，由两个子项目组成：

```
工作台product-manager-workbench-main/
├── AionUi/     # Electron 桌面端 + React 前端（TypeScript）
├── AionCore/   # Rust 后端（Axum + Tokio + SQLite）
├── docs/       # 文档、修改记录
├── LICENSE     # Apache-2.0（不可删除）
└── NOTICE      # 上游归属声明（不可删除）
```

**性质**：这是对上游开源项目 AionUi/AionCore 的定制分支。你要做的是**品牌与界面改造**，不是重写架构。

**法律底线（必须遵守）**：
- 不允许删除或修改 `LICENSE`、`NOTICE`、`AionUi/LICENSE`、`AionCore/LICENSE` 中的版权声明。
- 可以改品牌名/图标/文案，但必须保留"基于 AionUi/AionCore"的归属。

---

## 二、动手前必读（顺序执行）

1. `AionUi/AGENTS.md` —— 前端编码规范（组件库、i18n、命名、目录结构）
2. `AionCore/AGENTS.md` —— 后端规则（分层、日志、**"禁止凭名字猜行为"**）
3. `AionCore/ARCHITECTURE.zh-CN.md` —— 后端分层与新增功能步骤（改后端前必读）
4. `docs/MODIFICATIONS.md` —— 了解哪些文件是"定制改动"，别误伤

**核心铁律（AionCore 明确要求）**：
- 任何"某功能存在/不存在、某字段是什么含义"的判断，**必须先读源码文件确认**，禁止凭文件名字或经验猜测。
- 改动后必须运行验证命令，不能只改不验。

---

## 三、验证命令（改完必须跑，否则不算完成）

```bash
# 前端（在 AionUi 目录）
bun run test                 # 全部单测
bun run lint                 # oxlint 静态检查
node_modules/.bin/tsc --noEmit   # 类型检查

# 后端（在 AionCore 目录）
cargo check                  # 快速编译检查
cargo test -p aionui-system  # 改到 aionui-system 时跑这个
cargo build                  # 完整编译（较慢）
```

**原则**：改前端优先跑 `tsc --noEmit` + `bun run test`；改后端优先 `cargo check`。全绿才交付。

---

## 四、前端编码规范（摘自 AGENTS.md，必须遵守）

1. **UI 组件**：只用 `@arco-design/web-react`，禁止裸写 `<button>/<input>/<select>`。
2. **图标**：只用 `@icon-park/react`。
3. **样式**：优先 UnoCSS 工具类；复杂样式用 CSS Modules（`Xxx.module.css`）；颜色用语义 token，禁止硬编码色值。
4. **文案**：所有用户可见文字**必须走 i18n key**，禁止硬编码字符串。语言配置在 `packages/desktop/src/common/config/i18n-config.json`。
5. **TypeScript**：严格模式，禁止 `any`，用 `type` 不用 `interface`。
6. **进程隔离**：`process/`（主进程）禁碰 DOM；`renderer/`（渲染进程）禁碰 Node API；跨进程走 `preload/` 的 IPC bridge。
7. **目录**：单个目录直接子项 ≤ 10 个。

---

## 五、任务 A：品牌改造（第一优先，全部集中在这几处）

> 目标：把"产品经理工作台"换成用户给的新名字。改动要**最小化、可回滚**。

### 必改文件与字段

| 文件 | 改什么 |
|---|---|
| `AionUi/packages/desktop/src/common/branding.ts` | `APP_DISPLAY_NAME`（产品名）、`APP_MONOGRAM`（简称）、`BUTLER_DISPLAY_NAME`（内置助手名）；`UPSTREAM_UPDATE_ENABLED` 保持 `false` |
| `AionUi/packages/desktop/electron-builder.yml` | `appId`（改成用户给的 bundleId）、`productName`；`publish` 段若没有自建更新服务器，把 `publishAutoUpdate` 设为 `false` |
| `AionUi/package.json` | `name`、`description`、`productName`、`author` |
| `AionUi/packages/desktop/src/renderer/index.html` | `<title>` 与相关 meta |

### 需要系统性搜索替换的地方（用 grep 找，别漏）

```bash
cd AionUi
# 找所有出现旧名字的地方（排除 node_modules）
grep -rn "产品经理工作台\|AionUi\|Aion UI\|AionUI" packages/ public/ resources/ --include="*.ts" --include="*.tsx" --include="*.json" --include="*.html" --include="*.yml" | grep -v node_modules
```

按结果逐处判断：**用户可见文案**要改，**技术标识符**（如 `aionui-browser`、`aionui-` 前缀的 id、协议名、`com.aionui.app` 之外的兼容字段）**不要改**——它们是兼容契约。

### 图标资源（二进制，需重新生成后替换）

- `AionUi/resources/` 下的 `app.icns`、`app.ico`、`app.png`、`app_dev.png`、`icon.png`
- `AionUi/public/pwa/icon-*.png`、`AionUi/public/manifest.webmanifest`
- `AionUi/mobile/assets/images/icon.png`
- `AionUi/packages/desktop/src/renderer/assets/logos/brand/app.png`

> 没有新图标素材时，先只改文本品牌，图标标注"待替换"，不要用低质量占位图破坏仓库。

### i18n 文案

- 中文默认语言相关：`packages/desktop/src/renderer/services/i18n/startupLanguage.ts`、`packages/desktop/src/common/config/i18n-config.json`
- 各语言文案：`packages/desktop/src/renderer/services/i18n/locales/*/common.json`
- 改完必须跑：`node scripts/generate-i18n-types.js && node scripts/check-i18n.js`

### 完成标准（任务 A）

- [ ] grep 旧品牌名，仅剩"归属声明/技术标识符"合理残留
- [ ] `bun run test` + `tsc --noEmit` 全绿
- [ ] `electron-builder.yml` 的 appId/productName/publish 是新值
- [ ] 未改动 LICENSE / NOTICE

---

## 六、换皮地图（UI / 主题 / 布局改造，改界面前必读）

> 本项目的主题体系是**集中式 CSS 变量**设计：改一个文件里的变量，全局颜色/主题都会跟着变。
> **铁律：永远改语义变量，不要在某一个组件里硬编码色值。**

### 6.1 颜色 / 主题体系（换皮的第一战场）

| 文件 | 职责 | 什么时候改它 |
|---|---|---|
| `packages/desktop/src/renderer/styles/themes/default-color-scheme.css` | **颜色总控**：浅色(`:root`)与深色两套 CSS 变量，含品牌色板 `--aou-1`~`--aou-10`、`--brand`、`--primary`、背景/文字/边框/语义色 | 改主题色、品牌色、深浅色，主要改这里 |
| `packages/desktop/src/renderer/styles/colors.ts` | 上述变量的 **TS 常量清单**（供代码类型安全引用） | 新增变量时同步 |
| `uno.config.ts` | **原子类 → CSS 变量的映射**（`bg-brand`、`text-t-primary` 等） | 新增原子类时改 |
| `packages/desktop/src/renderer/styles/arco-override.css` | Arco 组件库外观覆盖 | 改按钮/弹窗/输入框等 Arco 组件样式 |
| `packages/desktop/src/renderer/theme/builtinThemes.ts` | 内置主题（Light/Dark/跟随系统） | 增删主题时改 |
| `packages/desktop/src/renderer/styles/MIGRATION.md` | **用法手册**（变量/原子类示例） | 不确定用哪个类时先读它 |

**换品牌色实操**：在 `default-color-scheme.css` 里改 `--aou-*`、`--brand`、`--brand-light`、`--brand-hover`（浅色与深色两段都要改），全局自动生效，不用逐个组件改。

### 6.2 布局 / 外壳（改结构）

| 想改的 | 去这里 |
|---|---|
| 整体布局框架 | `packages/desktop/src/renderer/components/layout/Layout.tsx` |
| 侧边栏导航（入口/顺序） | `components/layout/Sider/`（顺序在 `siderOrder.ts`，样式在 `Sider.module.css`） |
| 标题栏/窗口按钮 | `components/layout/Titlebar/` |
| 路由（加新页面） | `components/layout/Router.tsx` |
| 窗口标题 | `components/layout/DocumentTitle.tsx` |
| 某个页面 | `packages/desktop/src/renderer/pages/<页面>/` |
| 全局布局样式 | `packages/desktop/src/renderer/styles/layout.css` |

### 6.3 换皮任务映射表（想做什么 → 改哪里）

| 需求 | 正确做法 | 错误做法 |
|---|---|---|
| 换主题色/品牌色 | 改 `default-color-scheme.css` 的变量 | 在组件里写死 `#xxx` |
| 改某个组件的圆角/间距 | 组件对应的 `*.module.css` + `:global()` | 全局乱加 CSS |
| 改文案 | 改 i18n `locales/*/common.json` | 硬编码字符串 |
| 加/减侧边栏入口 | 改 `Sider/siderOrder.ts` + 入口组件 | 在 Layout 里堆按钮 |
| 加新页面 | `pages/` 建目录 + `Router.tsx` 注册 + Sider 入口 + i18n | 直接在旧页里塞 |
| 换图标/字体/间距 | 资源目录 + 主题变量 | 散落各处 |

### 6.4 换皮流程（每次改 UI 都走这个闭环）

1. 先读 `MIGRATION.md` 确认有没有现成变量/原子类。
2. 改语义变量或组件样式，不硬编码。
3. 跑 `node_modules/.bin/tsc --noEmit` + `bun run test` 验证。
4. 说明改动点与截图（或让用户看界面确认）。

> 具体的 UI 视觉设计（配色方案、布局稿）由用户单独提供，Codex 按上面的地图落地即可。

---

## 七、禁止事项（违反即打回）

- ❌ 删除/篡改 LICENSE、NOTICE 及上游版权头
- ❌ 凭文件名"猜"某功能的行为，不读源码就下结论
- ❌ 用户可见文案硬编码（必须 i18n）
- ❌ 前端用原生 HTML 元素替代 Arco 组件
- ❌ 改后端时破坏分层（领域 crate 不得依赖上层）
- ❌ 改完不跑验证命令就声称完成

---

## 八、交付格式

每次任务完成后，输出：
1. 改动文件清单（相对路径）
2. 每个改动的"为什么"
3. 运行了哪些验证命令、结果如何
4. 未完成/需人工确认的事项（如"图标素材待用户提供"）
