# Codex 启动提示词（复制以下全文给 Codex）

---

先读本文件，再按顺序执行后端开发任务。不要凭文件名猜测任何功能行为，改动前必须先读源码确认。

## 一、背景

本项目是「Speed Workbench」—— 一款快速构建 AI 产品原型的工作台/产品工厂。底座是 AionUi（Electron 前端，TypeScript）+ AionCore（Rust 后端，Axum+Tokio+SQLite，24 个 crate）。目标是：用户输入一句话想法，多 Agent 团队并行开发出可运行的 AI 产品原型。

本次只做**后端开发**，前端 UI 设计暂定，前端 demo 采用现有样式，不改前端代码。前后端联调用现有界面验证。

## 二、动手前必读（顺序执行，不要跳）

1. `AionCore/AGENTS.md` —— 后端规则（分层、日志、「禁止凭名字猜行为」）
2. `AionCore/ARCHITECTURE.zh-CN.md` —— 后端分层与新增功能步骤
3. `docs/PRD/产品工厂PRD.md` —— 产品需求（了解全貌即可，不逐字实现）
4. `docs/PRD/后端开发规划.md` —— **本次开发的权威规划**（含每个改动的精确文件、行号、改动内容、验证命令、验收标准）
5. `docs/PRD/研发流程与审核规范.md` —— 三栏审核标准

其中《后端开发规划》是唯一权威，其余是背景。冲突时以《后端开发规划》为准。

## 三、任务（按顺序执行，每步独立提交、独立验证、独立过三栏审核）

**0. 改动四（两个 skill）—— 已完成，你只需验证**
   - `AionCore/crates/aionui-app/assets/builtin-skills/product-blueprint/SKILL.md` ✅ 已写好
   - `AionCore/crates/aionui-app/assets/builtin-skills/product-sop/SKILL.md` ✅ 已写好
   - 你只做一件事：`cargo build` 验证这两个 skill 被 `include_dir!` 编译期发现并嵌入（无需改任何代码、无需改内容）。

**1. 改动一：review 门禁**（TaskStatus 加 InReview，防 Agent 糊弄）
**2. 改动二：隔离工作区**（每任务独立目录，多 Agent 不冲突）
**3. 改动三：用量/成本追踪**（记录每任务 token/成本）

改动一/二/三的精确文件、改动内容、影响面、验证命令、验收标准，全部见《后端开发规划》。

## 四、铁律（违反即打回）

1. 禁止凭文件名或经验猜测行为，改任何逻辑前先读源码确认（AionCore 明确要求）。
2. 改完必须运行验证命令，全绿才交付，不能只改不验。
3. 不删除/修改 `LICENSE`、`NOTICE`、上游版权头。
4. 不破坏现有功能（团队/看板/会话/定时/MCP 的回归测试必须过）。
5. 遵守后端分层：领域 crate 不得依赖上层；不在 `aionui-app` 里塞业务逻辑。
6. 每个改动完成后，提交三栏审核表（技术实现完整性 / 功能可用性 / AI 调用成本控制），任一栏不过不得进入下一个改动。

## 五、验证命令（在 AionCore 目录）

```bash
cargo check                            # 快速编译检查
cargo test -p aionui-team              # 改动一/二
cargo test -p aionui-db                # 改动一/三 migration
cargo test -p aionui-session           # 改动三
cargo build                            # 改动四（skill 编译嵌入）
```

前端（本次基本不动，仅当改动涉及前端类型定义时）：

```bash
cd ../AionUi && node_modules/.bin/tsc --noEmit && bun run test
```

## 六、交付格式（每个改动完成后输出）

1. 改动文件清单（相对路径）
2. 每个改动为什么这样做
3. 运行了哪些验证命令、结果如何
4. 三栏审核表（勾选 + 未勾选项的原因与补救计划）
5. 未完成/需人工确认的事项

## 七、成本控制提醒

- 改动一/二/四是纯状态机/文件系统/静态内容，不应消耗 AI 调用成本（改动四已完成）；
- 改动三自举：验证「用量追踪」本身产生的 AI 调用也要被记录，且不产生冗余调用；
- 不要为了「跑通」反复调用真实模型，能用 mock 验证的环节用 mock，标注「待验」即可。

---

开始执行。先读《后端开发规划》，`cargo build` 验证改动四已就位，然后从「改动一」做起。
