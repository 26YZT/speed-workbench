# Speed Workbench 当前开发状态

更新：2026-10-06。**本轮核心开发与内部技术验证已完成，尚未正式发布。** 已完成开发闭环、真实模型规划、费用可靠性、代码版本及本地原生QA；费用核定与正式发布条件另列，不用进度百分比推算完成度。

## 从第一性原理看产品

用户需要把想法变成自己能够掌握的本地产品。价值同时满足四个条件：**可运行**（独立启动、停止和重开）、**可验收**（需求对应产物与检查证据）、**可恢复**（中断与版本可回读，未经确认不重复执行）、**费用可核查**（实际模型、用量与价格来源能对账）。聊天结束、任务completed、财务核定和正式发布分别记录。

当前主链路：想法 → 规则草稿或真实模型规划候选 → 用户应用并确认 → 单Leader顺序开发 → 隔离任务目录 → Review与显式继续 → 唯一终结卡集成根目录 → 本地验收与费用核查 → 代码快照与新版本。

## 验收源码与本地包

- 最新验收源码 `0d35b57032cb719f6d2c9a86b606166257358d08`；Rust后端来源 `1e690924a4fd7953afcc1c019a22833a4ba3a0fa`，两提交Rust树相同：`d5850744ec35956b469c4345b76052e89ca55c29`。
- R2本地包 `AionUi/out/closeout-r2/mac-arm64/AionUi.app`，manifest source_commit为0d35b57，Core SHA256 `64e862e81b2723ba48b3356c04f351ad48ad7caae0d1da3e11f0d855d389841f`。
- source_commit指验收源码；后续文档提交HEAD另计。历史1468229/54864c4及旧包保留历史身份，不当成最新版本。

## 已实现与已验证

| 能力 | 实现与验证范围 |
| --- | --- |
| 开发闭环 | 持久交接、owner隔离、单Leader派发、Review、显式恢复与顺序推进；三个样例各4/4、共12任务内部技术approve并completed |
| 真实模型规划 | 先预约、独立会话、完整最终文本严格JSON、候选revision CAS、取消/不确定状态恢复；CLI5问→18需求→9卡已应用/确认/交接，9 pending未启动；Web5问候选未应用 |
| 任务权限 | v2需求引用/显式scope；v1中间Test隔离；唯一终结集成卡覆盖全部产物。真实图2张中间Test隔离、1张终结卡根目录；v2编辑器仅unit/API验证，handed_off原生页隐藏编辑器，未声称完整原生编辑验收 |
| 用量与预算 | 每次实际Send独立attempt；冻结实际模型回执、晚到累计高水位、重启baseline、共享准入、规划+开发预算及重估CAS；定向回归通过，真实金额仍未知 |
| 代码版本 | code_only manifest/hash、独立克隆、父版本、幂等/owner/active CAS、no-follow与不可变门禁；实际API33检查/38请求通过；全表2sealed/1working，CRUD v1sealed/v2working；原生只读页通过 |

## 最终工程验证

| 范围 | 结果 |
| --- | --- |
| Rust全workspace | **9571通过 / 0失败 / 53跳过**；264 targets含doctests；exit0。53跳过项仍未验证 |
| 受影响定向检查 | Conversation495、Team512、Session611、Agent1027；Factory77（规划9/版本15）、DB Factory15；App53+规划14+版本4；专项与整组重叠，不相加 |
| UI与静态检查 | Product Factory136通过；隔离16+storage5共21通过；TS/i18n/fmt与五crate all-targets clippy通过；lint0 errors、14既有warnings，与旧基线rule/message multiset一致，0新增 |
| 持久证据 | [最终workspace记录](evidence/L2-closeout-2026-10-06/workspace-final-verification.json)、[目标汇总](evidence/L2-closeout-2026-10-06/workspace-final-summary.txt)；首次磁盘链接失败已恢复，旧失败保留历史 |

## 三类真实产品

- **本地事项簿**：4/4任务，根目录48项、3个独立进程重启、真实浏览器CRUD通过。
- **CSV账目汇总器**：4/4任务，根目录81项、16个独立进程重开通过。
- **AI需求摘要服务**：4/4任务；独立59后端、10DOM逻辑、无模型HTTP失败恢复/跨进程持久化及组装根目录夹具浏览器通过。既有唯一真实业务POST HTTP201，实际gpt-6-astra，2metadata+1model_exec、6MCP关闭，内容区别于fixture。真实同DB重启读回仅有既有脚本记录；临时真实DB已删，不能独立再读，也未新增真实POST。

三个完整独立源码副本在 `/path/to/workbench/Speed-Workbench-验收产物-2026-10-06`，不含运行数据库/认证/任务临时目录。R2 API自然升级49→52，3旧Run/12Task/13Usage既有字段保留；三completed样例均ready=false，因为实际费用未知。版本新Draft预算7.50美元仅内部测试，无Team/planning/新增usage，不是价格或花费。

## 原生QA与隔离纠正

空白、规划、版本三个场景以实际/private/tmp data/log路径和正确parent归属复验通过；全局两个alias的target/device/inode全程不变；三次正常菜单Quit均exit0、所属App/Core进程0，无force signal。空白八表为0；规划保留2runs/4planning/9pending/0execution/4unknown usages；版本八表行SHA全部不变（4runs/3execution/2products/3versions/3ops/12completedtasks/13null usage）。saved/source manifest均21文件，source排除3项；迭代说明只读，未提交或启动模型。

旧E2E曾把共享alias指向临时目录，旧argv home别名不能证明访问日常数据库，旧隔离结论已被直接路径复验替代。E2E不再重定向全局目录别名，并隔离日志、跳过真实旧CDP注册表清理；alias以guarded方式恢复到既有正式Speed Workbench默认目录。最初target未采集，不能说精确恢复原目标。见[恢复边界](evidence/L2-closeout-2026-10-06/desktop-path-alias-recovery.json)、[21项隔离回归](evidence/L2-closeout-2026-10-06/desktop-e2e-isolation-regression.json)。

本地Electron37.10.3/builder26.15.2；managed4908文件基线一致，MCP/三LICENSE/NOTICE及旧包保留，ad-hoc deep strict verify0。原生QA未发起模型/开发动作、账本未变；未独立捕获provider网络流量。全景搜索/筛选与无横向溢出由主agent浏览器检查通过。

## 后续条件与产品边界

1. 核验用户实际服务费率与金额，完成财务对账和最终客户签收。真实样例与规划无预算，费用unknown、delivery.ready=false；fixture价格不能代替账单。工作台报告覆盖规划+开发，生成产品业务调用另外对账。
2. 正式分发需新机器安装/升级、Developer ID签名/公证、非技术用户验收及其他语言原生译文；本地ad-hoc QA包未公开发布。
3. v2任务编辑器的完整原生操作验收、53 ignored路径尚未完成；不将其算作已验证。

预算控制新实际Send，不是已开始调用的硬上限。版本仅code_only路径选择，不自动启动/重启服务，不迁移或回滚业务数据；Windows快照unsupported。常见数据/凭证过滤有范围，嵌入数据仍须审manifest。任务目录不是安全沙箱。产品内多Agent并行、自动云部署与通用Git合并不属于本轮已验收能力。

开发目录`.worktrees/product-factory-backend`，分支`feat/product-factory-frontend`；只本地提交，不自动推送/发布，不改日常用户数据库。详见[HANDOFF](HANDOFF.md)与[证据说明](evidence/L2-closeout-2026-10-06/README.md)。
