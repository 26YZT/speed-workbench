# 本轮核心开发与内部技术验证证据

更新：2026-10-06。本轮核心开发和内测验证完成，未正式发布；费用未知、客户签收及正式发布条件另计。当前状态[STATUS](../../STATUS.md)，机器快照[verification.json](verification.json)。

## 源码与包身份

最新source_commit `0d35b57032cb719f6d2c9a86b606166257358d08`；backend_source_commit `1e690924a4fd7953afcc1c019a22833a4ba3a0fa`。Rust树`d5850744ec35956b469c4345b76052e89ca55c29`两提交一致，后端SHA256 `64e862e81b2723ba48b3356c04f351ad48ad7caae0d1da3e11f0d855d389841f`。R2本地包manifest0d35b57，后续文档HEAD另计；旧包/旧提交仅历史身份。原API升级回执记录1e源码，是重包前的时间边界，最终桌面包记录0d。

## 最终工程验证

[workspace-final-verification.json](workspace-final-verification.json) / [workspace-final-summary.txt](workspace-final-summary.txt)：`cargo test -j 1 --workspace --offline`终端exit0；**9571通过、0失败、53ignored，264 targets含doctests**。跳过项未验证，不把其计入通过。首次磁盘链接失败已恢复，源码/数据/Core/App保留；旧错误仅历史，不再写进行中。

定向Conversation495/Team512/Session611/Agent1027、Factory77（规划9/版本15）、DB Factory15、App53+14+4、UI136，累计7与DB usage9含专项重叠，不追加总数。TS/i18n/fmt与五crate all-targets clippy通过。[桌面隔离回归](desktop-e2e-isolation-regression.json)16新隔离+5storage共21通过；lint0 errors、14旧warnings，实测旧基线rule/message multiset一致，0新增。旧App+Factory1000/32 ignored保留历史身份。

## 三类产品技术验收

[API收口](api-delivery-closeout.md)、[AI独立复核](api-delivery-ai-independent.json)、[根目录浏览器](ai-independent-root-browser.json)、[Core升级](api-delivery-core-upgrade.json)、[最终成本](api-delivery-final-cost.json)、[静态备份](api-delivery-completed-fixture.json)确认：3样例/12任务内部approve后completed；R2升级49→52、13原usage及旧Run/Task所有已有字段保留。

| 产品 | 验证与边界 |
| --- | --- |
| 事项簿 | 4/4、根目录48项/3进程重启/真实浏览器；[早期交付](crud-final-delivery.json)、[独立根目录](crud-independent-root-smoke.json) |
| CSV工具 | 4/4、根目录81项/16进程；[早期交付](cli-final-delivery.json)、[独立进程](cli-independent-root-smoke.json) |
| AI服务 | 4/4、独立59后端/10DOM、fixture无模型HTTP失败恢复与跨进程持久化、组装根目录浏览器；既有唯一真实POST HTTP201/gpt-6-astra、2metadata+1exec、6MCP关闭、内容不同fixture |

真实AI同数据库不同进程读回仅既有脚本记录；原临时DB已删，不能独立再读，也没有第二次真实POST。fixture重启与根目录浏览器另列，不冒充真实模型结果。三完整独立源码副本在`/path/to/workbench/Speed-Workbench-验收产物-2026-10-06`，不含运行DB/认证/任务临时目录。

## 真实模型规划

CLI `[5问](model-cli-interview.json) → [18需求](model-cli-blueprint.json) → [9卡](model-cli-task-graph.json)`；原候选candidate_ready/confirmed=false，[后续报告](model-planning-cost-report.json)3阶段applied、9pending，人工确认/交接后未启动。t6/t7为task_workspace中间Test，t9为唯一project_integration，传递依赖覆盖其余任务。Web[5问候选](model-web-interview.json)未应用。请求gpt-6-astra，蓝图实际usage为gpt-6.1-sol；计费归因取冻结实际回执。

## 版本API与原生只读验收

[版本API](api-delivery-versions.json)33checks/38HTTP、model calls0；同key幂等、payload冲突、stale hash、旧CAS、sealed不改写、working不可激活/未完成Run不能封存均有具体响应；复制path/hash/size一致，旧代码/账本不变。全表2sealed/1working，CRUD v1sealed/v2working。copying中断/平台/跨用户现场负面操作本轮未执行，由既有定向测试覆盖，不计33项。

[桌面包](desktop-package-verification.json) / [版本只读页](desktop-version-verification.json)：3个direct场景empty/P1/version的实际/private/tmp data/log/parent已验证，alias target/device/inode不变；三次正常菜单Quit exit0、所属App/Core进程0，无force signal。empty八表0；P1保留2runs/4planning/9pending/0execution/4unknown usage，Web候选未应用；版本八表行SHA全部相同，4runs/3execution/2products/3versions/3ops/12completedtasks/13null usage。

原生saved/source manifest21文件、source排除3项，hash与code_only服务/数据说明可见；迭代说明只读未提交；v2 editor因handed_off隐藏，仅unit/API与持久字段验，未完整原生编辑。QA未发起模型/开发动作，账本不变；provider网络流量未独立捕获。

包为本地Electron37.10.3/builder26.15.2，managed4908文件基线一致，MCP/三LICENSE/NOTICE/旧包保留，ad-hoc deep strict verify0，未Developer ID签名/公证/发布。

## 隔离事件更正与证据限制

[alias恢复](desktop-path-alias-recovery.json)记录旧E2E重定向共享alias至tmp。旧argv home是symlink文本，不能证明访问日常DB；旧隔离结论被最终直接路径复验替代。guarded恢复指向既有正式Speed Workbench默认目录，未读改数据库/凭证；最初target未采集，不能称精确恢复原目标。0d源码使E2E不再重定向全局目录别名，并隔离日志、跳过真实旧CDP注册表清理；21项回归与三场景target/dev/inode不变复验通过。

## 费用、发布与剩余范围

三completed样例均delivery.ready=false，真实费用null/unknown且无预算；规划金额同样未知。新Draft7.50仅内部API测试预算、无Team/planning/usage，不是价格或实际花费。fixture价格不能代替账单；工作台planning+execution与生成产品业务API分别对账。预算是新Send准入，不是已开始调用的硬上限。

后续为实际费率/金额与最终客户签收、新机器安装升级、正式签名公证、其他语言原生译文、53ignored路径和完整原生v2编辑验收。code_only选择不启动/重启服务、不迁移/回滚业务数据；Windows snapshot unsupported，嵌入数据审manifest，任务目录不是安全沙箱。原始旧回执不改写成最新成功；最终JSON/截图与汇总已持久，完整/private/tmp日志仍具临时性。
