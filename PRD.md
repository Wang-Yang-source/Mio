# Mio 产品需求与技术路线文档（PRD）

- **状态**：已批准并已验收阶段 1（阶段 0 已完成；阶段 1 已实施并实测，**内存目标未达标**：CLI `--version` RSS 仅 −1.97 MB，低于 35–45 MB 目标；阶段 2/3 仍为规划阶段）
- **更新日期**：2026-10-21
- **工作区与基线**：`/Users/waya/Projects/Mio`（分支 `codex/rust-kernel-handoff`，基线提交 `1c62e8c670fecbab04e2119a5f8c16af74a2bbd4`）
- **关联文档**：[AGENT.md](AGENT.md)、[AGENTS.md](AGENTS.md)、[TODO.md](TODO.md)、[HANDOFF.md](HANDOFF.md)、[STAGE0_FEASIBILITY_REPORT.md](STAGE0_FEASIBILITY_REPORT.md)

---

## 1. 背景与阶段 0 实证结论

Mio 基于 Pi coding agent 演进，核心诉求是在**无缝加载并运行未修改原版 Pi 插件**的前提下，大幅降低单实例与多 Agent 并发运行时的内存占用。

在阶段 0（见 [STAGE0_FEASIBILITY_REPORT.md](STAGE0_FEASIBILITY_REPORT.md)）中，通过对 macOS 26.3 (`arm64`) 隔离离线环境下的 7 大标准场景（每场景预热后 5 次测量）和 11 级模块分层拆解，得出以下实证结论：

1. **原版 Pi 的内存大头在「Node/V8 基础开销 + 顶层静态导入的 TUI/Theme/Schema/Highlight/HTML 模块图」，不在 Agent 循环状态机**：
   - 空 `node -e ""` 进程基线为 `39.42 MB RSS`（`4.38 MB HeapUsed`），挂载源码解析器后为 `48.55 MB RSS`。
   - `@mariozechner/pi-agent-core`（含 `agent-loop.ts`）仅占 `+2.23 MB RSS`（`+0.40 MB HeapUsed`）。
   - `packages/coding-agent` 启动导入 `main.ts` 跃升至 `110.22 MB RSS`（`peakFootprint` 约 `127.76 MB`），热点集中在顶层静态拉入的 `modes/interactive/theme/theme.ts`（顶层 `@sinclair/typebox/value` 同步校验 52 个主题 JSON，`+37.75 MB RSS`）、`interactive-mode.ts`（`+50.30 MB RSS`）、`utils/syntax-highlight.ts`（`cli-highlight`/`highlight.js`，`+14.95 MB RSS`）及 `core/export-html/index.ts`（`marked` + `theme.ts`，`+30.35 MB RSS`）。
2. **不继续将 `agent-loop` 翻译为 Rust，不直接做完整 Rust 内核重写**：
   - 原版插件契约允许扩展在 20+ 个钩子中直接同步读写并原地修改 JS 消息对象引用（验证 `===` 对象身份），并同步查询 `SessionManager` 会话树与挂载 `@mariozechner/pi-tui` 组件。
   - 只要保留真实 Node 宿主，将 `agent-loop` 迁入同进程 Rust 不仅无法消除 Node 模块图基线，反而会因 JS/Rust 双份消息镜像与跨 N-API 边界同步增加内存与兼容风险。
   - 现有 `native/` 下的 Rust 骨架（`native/kernel`、`native/node`）及 `packages/agent/src/loop-driver.ts` 保留为历史/可复用候选，不擅自删除，但当前不继续接入 `agent-loop.ts`。

基于阶段 0 证据，用户已正式决策采用以下三阶段演进路线：**阶段 1（TS 启动图瘦身与模块图门禁）→ 阶段 2（单 Node 宿主多会话守护进程与轻量客户端）→ 阶段 3（Rust N-API 有界共享线程池工具引擎）**。

---

## 2. 目标用户与核心使用场景

### 2.1 目标用户
- 在单机（macOS / Linux 工作站或开发服务器）上同时运行数十个并发编码 Agent 的重度开发者与自动化调度系统（如 Kanban 多 worker 编排、并行 PR 审查、多工作树回归测试、后台巡检守护进程）。
- 依赖现有 Pi 扩展生态（官方与第三方 TS/JS 插件、npm 依赖、原生 `.node` addon、自定义 Provider、自定义 TUI 组件与主题）且不接受重写或迁移插件的用户。

### 2.2 核心使用场景
1. **单机 32+ 并发 Agent 编排（核心场景，64 并发为扩展验证目标）**：
   - 用户或调度器同时拉起至少 32 个（扩展目标 64 个）独立会话（覆盖不同项目目录/worktree、不同模型/Provider、不同插件组合），执行代码检索、文件修改、测试运行或长上下文对话。
2. **Headless / Print / RPC / SDK 非交互自动化调用**：
   - 脚本、CI 或上层编排器通过 `pi -p`、`pi --mode rpc` 或公开 TypeScript SDK 高频调用单次或流式会话，无需加载交互式 TUI、全量语法高亮语言表或 HTML 导出渲染器。
3. **交互式 CLI / TUI 多终端接入**：
   - 开发者在多个终端或 tmux 窗格中打开交互界面，按需加载主题、语法高亮和 HTML 导出功能，交互体验、快捷键与原版完全一致。

---

## 3. 硬约束与插件兼容契约

### 3.1 无缝兼容未修改原版 Pi 插件（第一优先级硬约束）
- **原样加载与运行**：在相同 Pi 基线版本、平台和 Node 环境下，原版可正常加载和运行的插件（含源码、npm 依赖、包名、配置、安装/发现方式）必须原样工作，不得要求用户改写、转换、重新打包插件或迁移至第二套插件协议。
- **真实 Node 宿主**：保留真实 Node.js 运行时及现有 TS/JS 插件加载机制（含 `@mariozechner/jiti`）。插件依赖的 Node 内置模块、npm 包及原版支持的原生 `.node` addon 必须继续工作；禁止使用 QuickJS 或不完整的 Node API 模拟层替换宿主。
- **完整语义保持**：
  - 保持插件生命周期、事件触发顺序、同步/异步钩子契约、异常传播和取消（`AbortSignal`）行为。
  - 保持 JS 对象身份（Object Identity，`===`）与原地可变引用语义（如 `context`、`tool_call`、`tool_result`、`before_provider_request` 等钩子中的消息与状态修改）。
  - 保持工具、命令、快捷键、TUI 组件（`@mariozechner/pi-tui`）、自定义 Provider、动态注册和 `reload()` 行为。
- **公开 API 契约不变**：`packages/coding-agent` 及各子包公开 API 的同步/异步签名与返回值语义保持不变。
- **样本验证边界**：冻结插件样本仅用于自动化回归复现，不能把兼容目标缩减为白名单；有限插件样本通过测试不等于证明全生态 100% 兼容。

### 3.2 局部规则覆盖说明（仅限阶段 1 按需懒加载）
- 根级 `AGENTS.md` 原有“No inline imports (`await import()`)”规则，经用户在本路线中明确授权：**仅在阶段 1 为切断启动期 eager 依赖链（`theme.ts`/TypeBox、`cli-highlight`、`export-html`/`marked`、非交互路径 `InteractiveMode`）且必须保持公开 API 契约时，允许使用必要的动态 `import()` 或延迟求值**。
- 该例外严格限定在上述四个按需加载目标内，不得扩大为任意模块的动态导入改造，且不得引入动态类型导入（`import("pkg").Type`）。

---

## 4. 三阶段总体架构路线

### 阶段 1：TypeScript 启动模块图瘦身与门禁（当前实施阶段）
- **目标**：在不改变任何公开同步/异步 API 契约与插件行为的前提下，切断 `packages/coding-agent` 启动路径上的高内存静态依赖，并在 `scripts/check-entry-graphs.mjs` 中建立门禁防止回流。
- **四大瘦身切点**：
  1. **`theme.ts` 的 TypeBox 校验按需/轻量化**：消除 `modes/interactive/theme/theme.ts` 在模块顶层静态拉入 `@sinclair/typebox/value` 并在非交互启动时同步编译校验全部内置主题 Schema 的开销，保持主题校验语义与公开同步/异步接口兼容。
  2. **`cli-highlight`（`utils/syntax-highlight.ts`）按需加载**：仅在实际触发代码高亮渲染时加载 `cli-highlight` / `highlight.js` 语言表；若保持同步公开函数签名，需确保未触发或按需初始化路径不破坏调用方契约。
  3. **`export-html` / `marked`（`core/export-html/index.ts`）按需加载**：仅在执行 HTML 导出（如 `/export` 命令或导出 API 调用）时动态加载 `marked` 与导出模板逻辑，从核心会话与启动入口图中移除。
  4. **非交互路径上的 `InteractiveMode` 按需加载**：在 CLI `--version`/`--help`、Print 模式（`-p`）、RPC 模式（`--mode rpc`）及纯 SDK 会话创建路径上，不静态加载 `InteractiveMode` 及其重型 TUI 视图树；仅在进入交互式 TUI 模式时加载。
- **门禁建设**：扩展 `scripts/check-entry-graphs.mjs`，覆盖 CLI、Print/RPC、SDK 等入口的 eager 依赖图预算与禁止边（forbid edges），并通过注入禁止依赖的探针测试证明门禁能可靠阻断回流。

### 阶段 2：Multi-Agent Daemon / Multiplexer（单 Node 宿主多会话池，仅规划）
- **目标**：在阶段 1 完成后，构建单 Node.js（V8）宿主多会话池守护进程与轻量 CLI/TUI 客户端，使多个并发 Agent 共享同一份 Node 运行时、基础模块字节码与只读模型目录，避免每个会话重复支付 `~40–80 MB` 的独立进程底数。
- **共享与每会话隔离边界（核心设计约束）**：
  - **可安全共享层（只读/不可变）**：Node/V8 基础运行时、已加载的核心 TS/JS 模块代码、只读内建模型目录快照（`models.generated` / catalog）、无状态工具纯函数。
  - **必须每会话严格隔离层（Session-scoped）**：
    - `AgentSessionRuntime` / `SessionManager` 会话树与历史条目；
    - 插件实例（Extension instances）、每会话可变状态、命令/工具/钩子/快捷键注册表（Registries）、自定义 Provider 实例；
    - 认证凭据上下文（Auth/OAuth token 解析与刷新状态）；
    - 会话级工作目录（`cwd`）、环境变量视图（`env`）、终端/TUI I/O 通道、取消控制器（`AbortController`）与生命周期钩子（`reload` / `dispose`）。
- **进程级全局副作用与隔离风险（必须显式处理，禁止假定天然 100% 兼容）**：
  - 原版插件可能依赖或修改进程级全局状态：`process.chdir()`、`process.env` 原地写入、`process.exit()`、未捕获异常处理器、`globalThis` 单例、第三方 npm 包模块级可变缓存（Module-level singletons）以及非上下文感知的原生 `.node` addon。
  - **隔离与回退策略**：
    1. 在内置工具与框架层显式传递会话级 `cwd`/`env`/`AbortSignal`，禁止框架代码依赖 `process.cwd()`。
    2. 通过 `AsyncLocalStorage` 或代理层拦截可虚拟化的上下文访问，并对 `process.exit` 等危险调用做守护拦截。
    3. **独立子进程回退（Process Fallback）**：对于经检测或声明包含不可隔离进程级副作用、非重入原生 addon 或强制修改全局单例的插件/会话，宿主必须支持将其回退至独立 Node 子进程运行。
    4. **回退成本透明化**：任何触发独立子进程回退的会话，其完整子进程树内存（RSS/footprint）必须如实计入 32/64 会话总内存口径，明确记录兼容性与内存目标的取舍，严禁为凑内存指标而静默降级插件能力或隐藏子进程开销。

### 阶段 3：Rust N-API 并发工具引擎（有界共享线程池，仅规划）
- **目标**：在 `native/` 现有 Cargo workspace 基础上，借鉴 `can1357/oh-my-pi`（OMP）的 N-API 算子边界（须先完成许可证审查），使用 Rust N-API + 有界共享线程池替换高并发下实测存在 CPU/内存峰值的内部重算子。
- **范围与边界**：
  - **聚焦实测热点**：覆盖 32 路并发下的文件搜索（`grep` / `find` / `glob`）、大文件 I/O、Diff 计算、大输出截断（truncation）与可选的语法高亮等有实测证据的热点。
  - **并发控制与资源安全**：采用进程内有界共享线程池（Bounded shared thread pool），防止 32/64 会话同时触发工具时产生线程爆炸；具备明确的队列背压（Backpressure）、`AbortSignal` 取消中断、及时释放原生缓冲内存，且不在失败后重放副作用。
  - **语义一致与 TS 回退**：`read` / `write` / `edit` / `grep` / `find` / `ls` / `bash` 等内置工具保持与原版 TS 实现完全一致的参数、返回结构、错误处理与插件拦截语义；当原生模块未编译、ABI 不匹配或加载失败时，在启动阶段明确诊断并无感回退至 TypeScript 实现。

---

## 5. 内存与性能口径定义（待验证目标 vs 实测基线）

### 5.1 待验证目标（Target / Hypothesis，非既定实测结论）
用户提出的以下数值均为**待验证的工程目标/估算值**，在对应阶段完成真实测量前，**不得作为实测结论或无条件保证**：
- **阶段 1 目标**：单进程启动/非交互/空会话相比阶段 0 基线节省约 **35–45 MB** 内存。
- **阶段 2 目标**：单 Node 宿主内每新增一个会话的平均增量约 **4.6 MB**；**32 个（及扩展目标 64 个）并发 Agent 会话的总内存占用 < 300 MB**。

### 5.2 严格测量口径
所有阶段的内存与性能报告必须遵循以下统一口径，禁止只报有利指标或只测空壳会话：
1. **单位明确**：报告中必须明确区分十进制 `MB`（$10^6$ bytes）与二进制 `MiB`（$1024^2$ bytes），或直接附上原始字节数。阶段 0 现有脚本中 `MB` 按 `bytes / 1024 / 1024`（即 MiB 口径）计算，后续对比须保持同一换算公式并显式注明。
2. **多维内存指标**：
   - **进程驻留集（RSS）**：`process.memoryUsage().rss` 及 `process.resourceUsage().maxRSS`。
   - **平台物理内存指标**：macOS 记录 `/usr/bin/footprint` 的 `peakFootprint`（dirty + compressed）；Linux（若在 Linux 环境执行）记录 `/proc/<pid>/smaps_rollup` 的 `PSS`（Proportional Set Size）。跨平台未执行的项必须明确标为“未完成”。
   - **V8 堆与原生内存**：同时记录 `heapUsed`、`heapTotal`、`external`、`arrayBuffers`（涵盖原生 Buffer 与 N-API 分配）。
   - **进程树口径（Process Tree）**：包含守护进程、轻量 CLI/TUI 客户端进程、因隔离回退产生的独立 Node 子进程、以及工具启动的外部子进程（如 `rg`/`bash`）。
3. **负载梯度（禁止只测空壳会话）**：
   - 必须分别测量并报告：**冷启动/非交互退出**、**空闲会话（Idle）**、**活跃流式会话（Active Streaming）**、**长上下文/大工具输出（Long Context / Large Tool Output）**、以及**加载代表性插件/主题/高亮/导出后的负载**。
4. **统计可复现性**：
   - 固定代码提交、未提交改动状态、Node/OS 版本与模型目录 revision；
   - 同一环境下**变更前与变更后各至少独立运行 5 次**，保留完整原始 JSON/日志记录路径，报告**中位数（Median）与全距（Min–Max Range）**，并同时记录启动/首字节/命令完成延迟（Wall time）。
   - **基线有效性校验**：测量脚本必须确保被测模块在项目真实解析上下文中成功加载（例如修复旧 scratch 脚本因路径问题触发 `ERR_MODULE_NOT_FOUND` 导致漏算依赖内存的缺陷；不能把失败导入的 RSS 当作有效基线）。

---

## 6. 范围与非目标（Non-Goals）

### 本项目范围内（In Scope）
- **当前立即执行（阶段 1）**：重构 `PRD.md` / `TODO.md` 并同步 `AGENT.md` / `HANDOFF.md`；补齐 `check-entry-graphs.mjs` 入口依赖图门禁；实施 `theme.ts`/TypeBox、`cli-highlight`、`export-html`/`marked`、非交互 `InteractiveMode` 按需懒加载；完成阶段 1 前后 5 次基准对比与全路径兼容回归。
- **后续规划（阶段 2 & 3，阶段 1 验收汇报后再推进）**：单 Node 宿主多会话池 + 轻量客户端架构设计与实现；`native/` Rust N-API 有界共享线程池热点工具实现。

### 明确非目标（Out of Scope / Non-Goals）
1. **不继续将 `agent-loop` 或完整 Agent 状态机、会话管理器、全部内置 Provider 协议重写为 Rust**。
2. **不删除现有 `native/` Rust 骨架代码**（保留在仓库中作为阶段 3 及历史参考）。
3. **不使用 QuickJS、Deno 或不完整沙箱替代真实 Node.js 插件宿主**，也不要求现有 Pi 插件做任何源码或打包修改。
4. **不在本阶段自动執行 `git commit`、`git push`、修改锁文件（除非明确授权）或发布 npm 包**。
5. **不在缺少 Linux 环境或真实付费 API 凭据时伪造对应测试数据**；无法执行的跨平台或在线测试一律如实标记为“未完成”。

---

## 7. 风险登记与缓解措施

| 风险项 | 影响描述 | 缓解与验收措施 |
| --- | --- | --- |
| **懒加载破坏同步 API 或首次调用时序** | `theme.ts` 或 `syntax-highlight.ts` 改为按需加载后，若有外部调用方或插件依赖同步返回，改为异步会破坏公开 API 契约。 | 保持公开同步函数签名不变：对同步 API 采用首次调用同步惰性初始化（lazy synchronous init）或预热钩子，仅在异步入口使用 `await import()`；通过类型检查与现有单元测试验证。 |
| **门禁规则假阳性或漏检动态回流** | 静态导入门禁被绕过，或后续代码修改重新在顶层拉入 `marked` / `cli-highlight` / `theme.ts`。 | 在 `scripts/check-entry-graphs.mjs` 中精确配置各入口禁止模块/边；验收时必须做一次“注入禁止依赖 → 验证 `npm run check` 报错拦截 → 恢复代码”的探针验证。 |
| **测量脚本模块解析失效导致虚假内存基线** | 从仓库外目录（如 `/tmp` 或 `~/.hermes/cache/scratch`）直接运行脚本导入 `@sinclair/typebox` 等包时抛出 `ERR_MODULE_NOT_FOUND`，把报错退出的低 RSS 误记为基线。 | 所有测量入口必须通过仓库根目录上下文或显式 `createRequire` / `--import` 解析，并在结果 JSON 中校验加载成功标志与模块数量。 |
| **阶段 2 单宿主内插件全局状态污染** | 多会话共享同一 Node 进程时，插件调用 `process.chdir()`、修改 `process.env` 或模块级单例导致跨会话串扰。 | 阶段 2 严格划分共享只读层与每会话隔离层；对无法隔离的插件提供独立子进程回退，并将回退子进程内存纳入 32/64 会话总账。 |
| **阶段 3 N-API 线程池争用与取消泄漏** | 32 路并发执行文件搜索或 Diff 时，无界线程或未响应 `AbortSignal` 导致 CPU 饱和与原生内存驻留。 | 阶段 3 强制使用有界共享线程池、队列背压与取消令牌检查，并在壓测中验证取消后原生内存释放。 |
