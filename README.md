# Speed Workbench

从产品想法、需求确认到 AI 编码、人工审核和可本地运行的原型。

开源发布与维护：[26YZT](https://github.com/26YZT) · [反馈问题](https://github.com/26YZT/speed-workbench/issues)

基于周承健定制的「产品经理工作台」及 AionUi / AionCore 扩展。原作者、上游及第三方版权声明完整保留，详见 [作者与来源](docs/AUTHORS.md)和 [NOTICE](NOTICE)。

> 当前是可供开发者使用的源码内测版。源码公开不代表已经完成正式桌面分发、签名公证、新机器安装或最终用户验收。

## 当前能做什么

输入产品想法、目标用户、问题、输出形态和约束，选择已配置的助手与模型：

**模型访谈 → 产品蓝图 → 带依赖的任务图 → 人工确认 → 单 Leader 顺序开发 → 逐任务 Review → 集成与交付检查 → 代码快照与新版本。**

| 能力 | 当前范围 |
| --- | --- |
| 真实模型规划 | 生成产品相关问题、可追踪需求与任务依赖；候选需显式应用和确认 |
| 开发与恢复 | 一名已配置 Leader 顺序执行；任务独立目录、人工审核、打回和显式恢复 |
| 费用记录 | 记录规划与开发的实际模型、用量和计价来源；未知金额不会写成零 |
| 代码版本 | 保存选定代码与静态资源、克隆独立工作版本、选择已封存版本 |
| 工作台基础 | 会话、助手、项目、通用 Team、定时任务与模型对比等基础能力 |

产品工厂目前不提供多 Agent 并发生产。通用 Team 的多人能力与开发本项目时使用多个协作 Agent，不代表产品工厂已经接入并行调度。版本选择只切换代码路径，不自动重启服务、迁移或回滚业务数据；Windows 代码快照尚不支持。

## 已验证到哪一步

已完成三类内部技术验收样例：本地事项簿、CSV 账目汇总 CLI、包含真实模型调用的需求摘要服务。它们说明小型本地原型能够完成开发、测试、停止和重开，不证明任意复杂产品都能无人值守交付。

2026-10-06 验收快照：Rust 全工作区 9,571 通过、0 失败、53 忽略；产品工厂前端 136 项，桌面隔离与存储 21 项通过。忽略项未验证。原生 Mac ARM 空白、规划历史与版本页面已验证。实际费用仍未知，样例 `delivery.ready=false`；不能将测试价格当成账单。

[当前状态](docs/STATUS.md) · [开发交接](docs/HANDOFF.md) · [完整验收证据](docs/evidence/L2-closeout-2026-10-06/README.md)

真实模型规划已生成过 5 个问题、18 条需求和 9 张任务卡；这组九任务仅交接，尚未执行完成。不能把它与其他顺序开发样例拼成一条全自动端到端验证。早期 PRD 和上游文档中的阶段描述以当前状态记录为准。

## 从源码运行

需要 Rust 1.95.0（仓库工具链固定）、Node.js 22～24、Bun，以及本机可用并已配置的编码助手或模型服务。原生依赖编译可能需要 Python 3.11+ 和对应平台编译工具。源码不提供模型额度、个人配置、密钥或聊天记录。

macOS / Linux 开发启动：

```bash
git clone https://github.com/26YZT/speed-workbench.git
cd speed-workbench/AionCore
cargo build --locked --bin aioncore

cd ../AionUi
bun install --frozen-lockfile
AIONUI_BACKEND_BIN="../AionCore/target/debug/aioncore" bun run start
```

`AIONUI_BACKEND_BIN` 显式选择本仓库编译的后端，避免误用上游或其他已安装版本。Windows 使用相应的 `aioncore.exe` 路径和 MSVC 工具链；本轮没有完成 Windows 产品工厂全链路验收。

桌面打包还需要完整的本分支后端 bundle（binary + managed-resources）。`bun run package` 只编译前端，不等于生成安装包；默认后端准备流程可能下载上游版本，不能据此宣称包含本分支产品工厂。详见 [源码开发与打包](docs/guides/源码开发与打包.md)。

## 验证与参与开发

保留 AionCore / AionUi 同级目录。本仓库保留上游各模块指南；新用户从本 README 开始，无须分别克隆两个上游 main。

```bash
# 后端：需要 just 和 cargo-nextest
cd AionCore
just check

# 前端
cd ../AionUi
just check
bun run test
```

不安装 nextest 时可在 AionCore 使用 `cargo test --workspace`；推送遵守各模块 AGENTS.md 的完整门禁。根 GitHub CI 显式选择两个工作目录，不自动执行 GPT 评审、部署或发布。规范见 [AionCore 指南](AionCore/AGENTS.md)和 [前端贡献指南](AionUi/CONTRIBUTING.md)。

## 开源来源与许可

感谢 [AionUi](https://github.com/iOfficeAI/AionUi)、[AionCore](https://github.com/iOfficeAI/AionCore)与原始产品经理工作台作者周承健。通用运行时、助手、Team 等能力来自上游，不声明为本项目独立原创。

许可证见 [LICENSE](LICENSE)、[AionUi/LICENSE](AionUi/LICENSE)和 [AionCore/LICENSE](AionCore/LICENSE)。已有 Apache-2.0 条款、源文件版权头、第三方声明及原作者署名保留。26YZT 是本仓库的开源发布维护账号，不以维护身份替换原作者版权。

源码快照、路径脱敏和扫描边界见 [公开范围](docs/guides/源码公开范围.md)。

历史定制文件范围见 [MODIFICATIONS](docs/MODIFICATIONS.md)。请通过本仓库 Issues 反馈，勿提交密钥、个人数据库或私人会话。
