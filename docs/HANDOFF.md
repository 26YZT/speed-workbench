# Speed Workbench 开发交接

更新：2026-10-06。以 [STATUS](STATUS.md)、当前源码和 [本轮证据](evidence/L2-closeout-2026-10-06/README.md) 为准。旧聊天、旧测试和旧包保留历史身份。

## 目录与源码身份

- 开发：`/path/to/speed-workbench`，分支`feat/product-factory-frontend`。
- 最新验收source_commit `0d35b57032cb719f6d2c9a86b606166257358d08`；backend_source_commit `1e690924a4fd7953afcc1c019a22833a4ba3a0fa`。Rust树`d5850744ec35956b469c4345b76052e89ca55c29`相同，Core SHA256 `64e862e81b2723ba48b3356c04f351ad48ad7caae0d1da3e11f0d855d389841f`；R2包manifest0d35b57。后续文档HEAD另计，旧1468229/54864c4及旧包保留历史身份。
- QA包：`AionUi/out/closeout-r2/mac-arm64/AionUi.app`；正确binary在`Contents/Resources/bundled-aioncore/darwin-arm64/aioncore`。旧包中的旧binary不能当成R2。
- 三产品源码副本：`/path/to/workbench/Speed-Workbench-验收产物-2026-10-06`；不含运行数据库/认证/任务临时目录。
- 全景：`/path/to/workbench/Speed-Workbench-第一性原理与开发状态-2026-10-06.html`。

本轮核心开发和内测验证完成；未正式发布。不得reset/clean、覆盖旧目录或直写DB伪造完成；不自动推送/发布。

## 当前可用主链路

想法 → 规则草稿或显式模型规划 → 候选审阅/应用 → 用户确认需求与蓝图/任务 → 单 Leader 顺序执行 → Review → 显式继续/恢复 → 唯一终结任务集成根目录 → 启动/停止/重开验收 → 费用核查。

模型规划使用独立会话与工作目录，经正常 ConversationService 创建和实际 Send 入口执行，准备阶段不 warmup。回合必须 completed 且 output_complete，最终规范文本整体严格 JSON 校验；不读部分 stdout，不以规则草稿冒充模型成功。访谈最多5问；v2蓝图有需求ID；任务有requirement_ids与execution_scope。模型候选不自确认。

v1保持draft历史兼容，v2为model来源。中间任务（包括Test）只在task_workspace；唯一终结集成任务才可使用project_integration，并通过依赖覆盖所有其他任务。普通Task目录隔离是执行范围约定，不是安全沙箱。

## 规划接口与恢复

接口前缀 `/api/product-factory/runs/{id}`，成功 `ApiResponse<T>`，失败为带稳定code的错误；变更请求需登录owner和CSRF。

| 请求 | 契约 |
| --- | --- |
| POST `/planning` | phase=interview/blueprint/task_graph；expected_plan_revision、idempotency_key、assistant_id、model。空model表示沿用助手/后端默认，不是伪模型default |
| GET `/planning`、GET `/planning/{attempt_id}` | 回读attempt列表/状态/候选，不触发新模型调用 |
| POST `/planning/{attempt_id}/apply` | expected_plan_revision CAS；应用候选后仍需用户确认 |
| POST `/planning/{attempt_id}/cancel` | 显式取消；保留旧attempt与已发生费用 |

预约状态持久化；candidate_ready可审阅，applied表示已应用。failed/cancelled/uncertain保留事实。响应丢失先GET；uncertain不自动重发。用户明确新轮才用新幂等key。公共Send门禁验证持久的owner/run/conversation/appTurn绑定，任何replay或skill continuation也重新检查预算；本地准入拒绝与已真实调用但缺回执分别计量。

关键稳定错误包括 `USER_MODEL_SEND_ADMISSION_REJECTED`、`PLANNING_UNCERTAIN_ATTEMPT`、`PLANNING_CONVERSATION_NOT_BOUND`、`PLANNING_ATTEMPT_NOT_RUNNING`、`PLANNING_APP_TURN_NOT_BOUND`；以实际接口返回为准，不解析内部日志推断成功。

## 用量、费用与代码版本

- 账本每次实际Send记录独立attempt，app turn保持真实回合身份；重试/continuation不覆盖旧attempt。模型以冻结的运行回执为准，picker/request不替代实际model。
- native累计报告按高水位计算增量，重启沿用持久baseline；晚到用量保留原attempt/task/model。费用未知保留未知，显式重估按CAS更新；历史缺分桶不自动补价。
- planning+execution共同计入产品预算，Factory绑定Team使用动态费用贡献。预算Some且未知时拒绝新Send；预算None可以继续但金额仍unknown且交付财务检查不通过。仓储读取错误拒绝准入。
- 工作台报告只覆盖规划与开发；生成产品自己的业务API账单另外核查。当前真实费用均unknown、无预算、delivery.ready=false，费率信息仍待用户提供，fixture仅测试算法。
- 版本是code-only：manifest与hash、独立代码克隆、owner/CAS及幂等预约；copying/failed/sealed不能当普通可写版本。sealed关联Team新Send也受门禁。
- active版本选择只选代码路径，不启动/重启服务、不迁移或回滚业务数据。旧代码目录、运行账本与业务数据库保留；Windows快照尚不支持。常见数据/凭证过滤不能证明所有嵌入数据都已分离，使用前审manifest。

## 接线入口

- Factory：`AionCore/crates/aionui-product-factory/src/` 的 planning、planning_validation、task_draft、handoff、execution、review、delivery 与 versions 模块。
- 组合层：`AionCore/crates/aionui-app/src/router/` 的 state、product_factory_planning、conversation_send_admission、team_conversation_adapters。
- 共享Send：Conversation；计量：AI Agent/Session与DB usage仓储；费用贡献：Team预算port。
- UI：`AionUi/packages/desktop/src/renderer/pages/product-factory/`；API DTO位于aionui-api-types，迁移050/051/052由DB统一执行，不手工改用户数据库。

## 最终验证与交付事实

Rust全workspace命令`cargo test -j 1 --workspace --offline`，exit0、9571passed/0failed/53ignored、264 targets含doctests。53ignored未验证；专项与整组重叠。首次磁盘不足已恢复，旧失败保留历史。最终持久[JSON](evidence/L2-closeout-2026-10-06/workspace-final-verification.json) / [targets汇总](evidence/L2-closeout-2026-10-06/workspace-final-summary.txt)。本轮已完成检查，新增源码变化才重跑相应范围。

UI136、隔离16+storage5共21、TS/i18n/fmt、五crate all-targets clippy通过；lint0 errors/14旧warnings，与旧源码rule/message multiset一致，0新增。详见[隔离回归](evidence/L2-closeout-2026-10-06/desktop-e2e-isolation-regression.json)。

三个样例/12任务内部技术approve后completed，13旧usage不变且ready=false。CRUD根目录48/3进程、CLI81/16进程；AI独立59后端/10DOM、无模型HTTP失败恢复/跨进程持久化及根目录夹具浏览器通过。既有唯一真实POST HTTP201，实际gpt-6-astra（2metadata+1model_exec，6MCP关闭），内容不同fixture；真实同DB重开仅脚本记录，临时DB已删，不能独立再读，不再追加真实POST。R2 API升级49→52保持3Run/12Task/13Usage既有字段。版本33checks/38HTTP，全表2sealed/1working；CRUD v1sealed/v2working，新Draft7.50仅内测预算，无Team/planning/usage。见[API收口](evidence/L2-closeout-2026-10-06/api-delivery-closeout.md)。

## 原生直接路径QA与恢复边界

最终[原生包记录](evidence/L2-closeout-2026-10-06/desktop-package-verification.json)及[版本只读记录](evidence/L2-closeout-2026-10-06/desktop-version-verification.json)：Electron37.10.3/builder26.15.2、ad-hoc deep strict verify0，managed4908文件基线/MCP/三LICENSE/NOTICE/旧包保留。empty/P1/version实际/private/tmp data/log与parent归属验证；两个global alias target/dev/inode不变；三个正常菜单Quit exit0、所属App/Core进程0，未force signal。

空白八表0；P1保留2runs/4planning/9pending/0execution/4unknown usages，不应用Web候选、不启动开发；版本八表行SHA完全相同，4runs/3execution/2products/3versions/3ops/12completedtasks/13null usages。saved/source manifest21文件、source排除3项；只读迭代说明未提交。v2 editor在handed_off不渲染，仅持久字段/unit/API验，不能称完整原生编辑验收。QA未发起模型/开发动作、账本无新增；未独立捕获provider网络流量。

旧E2E曾重定向共享alias到tmp，argv home别名不能用于判断日常DB访问。E2E不再重定向全局目录别名，并隔离日志、跳过真实旧CDP注册表清理；[guarded恢复](evidence/L2-closeout-2026-10-06/desktop-path-alias-recovery.json)以既有正式默认目录为依据，不读改数据库/凭证。最初target未采集，不能说精确恢复原目标。后续QA必须使用显式data/log目录，不能重新改global alias；只停止记录归属的自建进程，不操作日常用户进程。

本轮自建26980、26990及原生QA均已安全结束，无活模型任务；仅保留源码/数据/证据，日常用户进程不操作。主agent已浏览器验证同一全景搜索/状态筛选与无横向溢出。

## 后续条件

- 实际费率/金额核查与最终客户签收；真实费用unknown、delivery.ready=false，fixture不得补账。
- 新机器安装/升级、Developer ID签名/公证、非技术用户验收、其他语言原生译文。
- 53ignored路径及完整原生v2编辑器操作验收；当前本地QA包未公开发布。

code_only版本选择不自动启动/重启服务、不迁移或回滚业务数据；Windows快照unsupported。任务目录不是安全沙箱；嵌入数据需审manifest。工作台planning+execution账单与产品自己的业务API账单分别对账。产品内多Agent并行、云部署、通用Git合并非本轮已验收范围。
