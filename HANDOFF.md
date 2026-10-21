# Hermes 交接：Mio 内存优化与三阶段架构路线

## 用户要求与最新路线决策（2026-10-21 更新）

在当前 Pi fork 上降低单实例与多并发 Agent 内存占用，同时**100% 无缝兼容未修改的原版 Pi 插件**（硬约束见 [AGENT.md](AGENT.md)）。

在完成阶段 0 实测评估（见 [STAGE0_FEASIBILITY_REPORT.md](STAGE0_FEASIBILITY_REPORT.md)）后，用户已正式作出路线决策并更新需求（详见 [PRD.md](PRD.md) 与 [TODO.md](TODO.md)）：
1. **停止将 `agent-loop` 或完整 Agent 状态机重写为 Rust**；现有 `native/` 下的 Rust 骨架与 `packages/agent/src/loop-driver.ts` 原样保留为历史与阶段 3 可复用候选，不擅自删除。
2. **阶段 1（当前实施）**：TypeScript 启动模块图瘦身与 `scripts/check-entry-graphs.mjs` 门禁补齐；对 `theme.ts`/TypeBox 校验、`cli-highlight`、`export-html`/`marked`、非交互路径 `InteractiveMode` 实施按需懒加载，保持公开同步/异步 API 契约与插件兼容。
3. **阶段 2（仅规划，阶段 1 完成后推进）**：单 Node 宿主多会话池（Multi-Agent Daemon / Multiplexer）与轻量 CLI/TUI 客户端，目标支持 32+（扩展验证 64）并发会话；严格区分共享只读层与每会话隔离层，对存在进程级全局副作用或不可重入原生 addon 的插件提供独立子进程回退并如实计入内存口径。
4. **阶段 3（仅规划）**：`native/` Rust N-API 有界共享线程池热点工具引擎，仅替换有实测证据的高并发热点（文件搜索 `grep`/`find`、大文件 I/O、Diff、输出截断等），保留 TS 回退且不重放副作用。
5. **待验证目标声明**：单进程节省 `35–45 MB`、每会话增量约 `4.6 MB`、32/64 会话总占用 `< 300 MB` 均为待测目标/估算值，非既定实测保证。

仓库：<https://github.com/Wang-Yang-source/Mio>

交接分支：`codex/rust-kernel-handoff`。先读根目录 `AGENTS.md`、`AGENT.md`、`PRD.md`、`TODO.md`、`README.md` 和本文件；遵守项目检查及版本管理要求，不自动提交或发布后续功能。

## 当前文件与实际行为

- `TODO.md`：完整迁移步骤和未完成验收。
- `native/`：独立 Cargo workspace。`kernel` 是无依赖回合调度状态机；`node` 是 napi-rs 类绑定，协议版本 1。
- `packages/agent/src/loop-driver.ts`：动作/反馈类型和同步驱动接口。
- `packages/agent/src/agent.ts`、`types.ts`：传递实验性的 `loopDriverFactory` 参数。

**关键限制：`agent-loop.ts` 尚未使用这个参数；没有原生模块加载器、后端选择、打包脚本或闭环运行。默认和实际执行均仍为原来的 TypeScript。**

Rust 原型只保存调度状态，不保存消息正文，也不调用模型或工具。尚未验证 JS 与原生模块的真实加载、异常/取消桥接和释放语义。内存基准、兼容矩阵、差分测试均未实现。

本地旧项目位于 `Mio-Backup/Mio`，是独立仓库，仅供参考，未随本次交接提交。
不从备份目录加载依赖或凭据；无需依赖备份即可继续当前 Pi 工作。

## 已执行验证

环境：Linux，Node 26.8.2、npm 11.19.1、Rust/Cargo 1.98.1。

- `npm ci --ignore-scripts`：成功；已有 autoevals 包管理器 engine 警告、依赖弃用提示、3 项 high 漏洞，未自动升级依赖。
- `cargo fmt --manifest-path native/Cargo.toml --all --check`：通过。
- `cargo check --manifest-path native/Cargo.toml --all-targets --locked`：通过。
- `cargo test --manifest-path native/Cargo.toml --locked`：纯内核 4 项通过；不包括 Node 加载或 Agent 闭环。
- `cargo clippy --manifest-path native/Cargo.toml --all-targets --locked -- -D warnings`：通过。
- 在 `packages/agent` 执行 `node ../../node_modules/vitest/dist/cli.js --run test/agent.test.ts test/agent-loop.test.ts`：78 项通过，验证的是现有 TypeScript 行为。

首次 Agent 测试因缺少生成的模型 JSON 而失败。随后按 `nix/model-catalog.json` 中固定 revision 下载目录，验证 SHA-256，再运行 `packages/ai/scripts/hydrate-model-catalog.ts`，复跑通过。未调用真实模型或写入模型生成源码。

原工作目录的 `npm run check` 被备份中 `.cache/codex-src/package.json` 的非固定 prettier 版本阻断。验证副本只包含本次源码快照，不包含旧备份。固定目录还缺少 `stream.test.ts` 引用的 NVIDIA 模型，导致一次类型检查失败。下载最新公开目录并核验 SHA-256 后，验证副本的 **`npm run check` 全部通过**（格式、依赖、入口、安装锁、类型、浏览器 smoke）。不要为通过检查修改旧备份。

通过检查使用的模型目录 revision：`sha256-ce2b35e558368354315ceab5a0bc1d50e53d405e8f261ea019a491591bb0a758`。可从 `https://pi.dev/api/models/revisions/<revision>?types=chat,image,classifier` 下载，核验散列后执行 `node packages/ai/scripts/hydrate-model-catalog.ts <下载文件>`。这些 JSON/类型数据是忽略文件；本次未改动 `nix/model-catalog.json` 或模型生成源码。

## 当前阶段与执行顺序

阶段 0（原版内存基线、热点拆解与参考项目核查）已在 `t_32802225` 中完成，产出了 [STAGE0_FEASIBILITY_REPORT.md](STAGE0_FEASIBILITY_REPORT.md) 与原始基准记录 `/Users/waya/.hermes/cache/scratch/scenario-benchmark-results.json`。

当前按用户最新授权推进**阶段 1**（拆分为 4 个串行 Kanban 任务切片）：
1. **切片 1/4（`t_3d6f4941`，已完成文档重构）**：创建 [PRD.md](PRD.md)，重构 [TODO.md](TODO.md)，同步 [AGENT.md](AGENT.md) 与本文件。阶段 1 代码与实测项保持未完成状态。
2. **切片 2/4（`t_fcb99872`，替代归档卡 `t_ccda7a87`，已完成门禁与变更前基线）**：核查 CLI / Print / RPC / SDK 启动 eager 模块图，补齐 `scripts/check-entry-graphs.mjs` 门禁，修复 scratch 模块解析上下文并采集固定环境变更前 11 组探针×5 次有效基线（存于 `/Users/waya/.hermes/handoffs/mio-stage1-pre-baseline-20261021.json` 与 `/Users/waya/.hermes/handoffs/mio-entry-delivery-20261021.md`）。
3. **切片 3/4（`t_2348d011`，替代归档卡 `t_8a8382ac`，已完成按需加载）**：`InteractiveMode` 与 `export-html`/`marked` 改为异步边界 `await import()`，`theme-schema.ts` 改为仅在交互或非内置主题时按需加载；`check-entry-graphs.mjs` 改为最终预算（cli 381 / rpc-entry 380 / rpc-mode 60 / sdk 309 / print 8）并完成注入回流探针。`utils/syntax-highlight.ts`（同步公开 API）与 `marked`（pi-tui 根 barrel）因打包安全保留 eager，原因见 `/Users/waya/.hermes/handoffs/mio-lazy-delivery-20261021.md`。
4. **切片 4/4（`t_9c59244c`，替代归档卡 `t_51b9f553`，已完成阶段 1 验收）**：同环境 5 次实测（11 探针 + 1 工作负载探针）与逐项前后对比见 `/Users/waya/.hermes/handoffs/mio-stage1-post-baseline-20261021.json`。**RSS 实测仅降 −1.97 MB（125.87→123.90 MB），未达 35–45 MB 目标**：`typebox`/`highlight.js`/`marked` 仍 eager，被移除模块的成本仍由其他路径支付。功能验收：`npm run check` exit 0；vitest 22/23 文件通过（`rpc.test.ts` 缺凭据跳过）、TUI 189 例全通过；离线 faux 下 print 与 RPC（JSONL 子进程）E2E 通过，含 HTML 导出。真实模型验证未完成（无凭据）。另发现运行时回流：带扩展时 `virtual-modules.ts` → SDK barrel → `modes/index.ts` → `interactive-mode.ts` 仍被拉入。详见 `/Users/waya/.hermes/handoffs/mio-stage1-delivery-20261021.md`。
5. **总验收（`t_b8b8beb8`，已完成）**：文档路线验收通过；阶段 1 实现与兼容验收通过，内存目标未达标（CLI RSS −1.97 MB，复核重跑 −2.53 MB）。未完成项保持未完成：`S0-4`、`S1-LAZY-2`、`S1-LAZY-3` 的 `marked` 部分、`S1-LAZY-4` 运行时回流、`S1-VERIFY-3` 真实模型验证。详见 `/Users/waya/.hermes/handoffs/mio-final-acceptance-20261021.md`。

## 已有 Rust Pi：2026-10-21 初查

这些结论来自仓库 README 与具体源码，未下载运行或测量性能。

| 项目 | 与目标的关系 | 已确认的兼容边界 |
| --- | --- | --- |
| [can1357/oh-my-pi](https://github.com/can1357/oh-my-pi) | 优先参考 Rust N-API 边界、工具实现及加载/分发 | 上层为 TypeScript/Bun，Agent 主循环仍为 TS；不能据此推定原版插件全兼容 |
| [c4pt0r/pie](https://github.com/c4pt0r/pie) | 参考 Rust 实现、稳定请求历史与模型前缀缓存优化 | 未核实原版 TS/npm 插件加载能力；模型侧 KV 缓存与客户端进程内存分开测量 |
| [jshachm/pi-rs](https://github.com/jshachm/pi-rs) | 参考 Rust TUI、会话与工具分层 | 当前扩展 loader 读取 extension.json 元数据，未见执行 Pi TS/npm 插件的逻辑；性能数字未实测 |
| [yannelli/pi-rust](https://github.com/yannelli/pi-rust) | 参考全栈 Rust 移植的内部实现 | 使用 QuickJS 与 Node 兼容模块；不能加载原生 npm addon，并有明确的 API 行为差异 |
| [Dicklesworthstone/pi_agent_rust](https://github.com/Dicklesworthstone/pi_agent_rust) | Rust 原生 Agent，有自己的扩展与资源控制实现 | 当前 README 明确不再追求严格替代 TypeScript Pi，不能当作插件完全兼容方案 |
| [bigfish1913/pi-rust](https://github.com/bigfish1913/pi-rust) | 可参考 Rust SDK 分层及工具实现 | 使用 Rust cdylib/C ABI 插件，明确不加载 Pi TypeScript SDK 或 npm 扩展 |

`yannelli/pi-rust` 的 [unsupported.rs](https://github.com/yannelli/pi-rust/blob/main/crates/pi-js/src/unsupported.rs) 列出未实现的 fs/crypto/stream 等 API，以及 async_hooks、子进程、网络和对象行为差异。模块可 import 不等于对应 Node 行为完全一致。其 [v0.5.0](https://github.com/yannelli/pi-rust/releases/tag/v0.5.0) 发布于 2026-10-20。

`Dicklesworthstone/pi_agent_rust` 的 [README](https://github.com/Dicklesworthstone/pi_agent_rust/blob/main/README.md) 已改变产品方向；[LICENSE](https://github.com/Dicklesworthstone/pi_agent_rust/blob/main/LICENSE) 带额外限制，不应按普通 MIT 项目直接移植。

以上项目仅供 Rust 内部实现参考；保持当前 Pi 工程与真实 Node 插件宿主。测试集合用于复现，不能变成兼容白名单；不得因参考项目的兼容缺口降低本项目要求。

补充证据：[OMP 原生模块](https://github.com/can1357/oh-my-pi/blob/main/docs/native-crates.md)、[N-API 架构](https://github.com/can1357/oh-my-pi/blob/main/docs/natives-architecture.md)、[TS 主循环](https://github.com/can1357/oh-my-pi/blob/main/packages/agent/src/agent-loop.ts)；[pie 的 DS4 说明](https://github.com/c4pt0r/pie/blob/main/docs/ds4.md) 中 DS4 指本地模型服务，并非序列化算法名称；[pi-rs 扩展 loader](https://github.com/jshachm/pi-rs/blob/main/src/extensions/loader.rs) 目前只体现元数据加载。

## 交付状态

Mac 已拉取交接分支至 `/Users/waya/Projects/Mio`。Hermes 默认看板已创建任务 `t_32802225`，先做上述阶段 0，任务正文保存于 Mac 的 `/Users/waya/.hermes/handoffs/mio-rust-20261021.md`。运行状态以 `hermes kanban --board default show t_32802225` 为准。

原公网端口直连超时；`wayamac-ts` 已验证可用。本地 `wayamac` 已改为经 Ylris 公网 SSH 跳板访问其本地 FRP 转发端口，并实测成功登录 Mac。没有修改服务端防火墙或宣称公网端口直连已恢复。
