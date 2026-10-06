# 后续维护启动提示词

先读 `docs/HANDOFF.md` 与 `docs/evidence/L2-2026-10-06/README.md`，再读前后端 AGENTS.md 和本次要改的源码。

开发位置为 `.worktrees/product-factory-backend`，当前分支为 `feat/product-factory-frontend`。不要操作主目录快照或其他项目，不 reset/clean。

真实执行、Review、顺序推进与交付接口已经接通。不要按历史交接重新写调度引擎，也不要继续把标题工具当产品工厂内置功能。

先定位当前验收记录中的未通过项，再做最小修复。实际模型结果、fixture 测试、历史样品和发布产物分别记录；金额未知不得按零记账；未完成真实链路不得宣布正式 L2 发布完成。

保持单 Leader 的 L2 范围，避免提前扩展并行/云部署。每个改动运行相关回归测试和类型/编译检查，并更新验收证据。
