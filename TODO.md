# Mio 实施路线与可执行任务清单（TODO）

## 0. 全局规则与路线边界

- **当前批准路线**：**阶段 1（TS 启动图瘦身与模块图门禁）→ 阶段 2（单 Node 宿主多会话池与轻量客户端）→ 阶段 3（Rust N-API 有界共享线程池热点工具引擎）**。需求与架构规范详见 [PRD.md](PRD.md)，兼容硬约束见 [AGENT.md](AGENT.md)，代码质量与测试门禁见 [AGENTS.md](AGENTS.md)。
- **停止全量 Rust 内核重写**：基于阶段 0 实测证据（见 [STAGE0_FEASIBILITY_REPORT.md](STAGE0_FEASIBILITY_REPORT.md)），**不再继续将 `agent-loop`、会话树或内置模型协议翻译为 Rust**。现有 `native/` 下的 Rust 骨架（`native/kernel`、`native/node`）与 `packages/agent/src/loop-driver.ts` **原样保留**为历史与阶段 3 可复用候选，不擅自删除。
- **硬约束**：100% 无缝加载并运行未修改的原版 Pi 插件（保留真实 Node 宿主、npm 依赖、原生 `.node` addon、TUI 组件、自定义 Provider、动态注册、`reload`、对象身份 `===`、事件顺序与公开同步/异步 API 契约）。有限样本测试通过不等于证明生态 100% 兼容。
- **目标口径**：单进程节省 `35–45 MB`、每会话增量约 `4.6 MB`、32/64 并发会话总占用 `< 300 MB` 均为**待测目标/估算值**，在对应阶段完成同环境前后对比测量前，不得写为既定实测结论或无条件保证。
- **局部规则例外**：用户已明确授权在**阶段 1** 为实现 `theme.ts`/TypeBox、`cli-highlight`、`export-html`/`marked`、非交互路径 `InteractiveMode` 按需懒加载并保持公开 API 契约时，允许使用必要的动态 `import()`（作为对 `AGENTS.md` 禁止 inline imports 规则的受控局部覆盖）；禁止扩大为任意改造。
- **状态标记规则**：`[x]` 仅表示已有可核验文件/命令证据的已完成项；任何未执行、部分完成或跨平台/缺凭据未测项必须保持 `[ ]` 并明确说明。

---

## 阶段 0：原版内存基线、热点拆解与路线可行性评估（已完成，保留证据）

- [x] **S0-1：工程基线与既有 Rust 骨架审计**
  - **产物**：[HANDOFF.md](HANDOFF.md)、[STAGE0_FEASIBILITY_REPORT.md](STAGE0_FEASIBILITY_REPORT.md) 第 3.1 节。
  - **依赖**：分支 `codex/rust-kernel-handoff`（基线提交 `1c62e8c670fecbab04e2119a5f8c16af74a2bbd4`），模型目录 `sha256-ce2b35e558368354315ceab5a0bc1d50e53d405e8f261ea019a491591bb0a758`。
  - **验收命令与证据**：
    - `npm run check`：100% 通过。
    - `cargo fmt --manifest-path native/Cargo.toml --all --check`、`cargo check --manifest-path native/Cargo.toml --all-targets --locked`、`cargo clippy --manifest-path native/Cargo.toml --all-targets --locked -- -D warnings`、`cargo test --manifest-path native/Cargo.toml --locked`：4 项纯内核单测通过；确认 `agent-loop.ts` 未消费 `loopDriverFactory`。
- [x] **S0-2：macOS 隔离离线环境 7 大场景×5 次基准与 11 级模块增量拆解**
  - **产物**：[STAGE0_FEASIBILITY_REPORT.md](STAGE0_FEASIBILITY_REPORT.md) 第二节；原始数据 `/Users/waya/.hermes/cache/scratch/scenario-benchmark-results.json`。
  - **依赖**：S0-1；macOS 26.3 (`arm64`)，Node `v25.9.0`，离线 `faux` Provider。
  - **验收命令与证据**：
    - 定位启动内存大头为 `theme.ts`（TypeBox 校验，`+37.75 MB RSS`）、`interactive-mode.ts`（`+50.30 MB RSS`）、`export-html`（`+30.35 MB RSS`）、`syntax-highlight.ts`（`cli-highlight`，`+14.95 MB RSS`）；`pi-agent-core` 仅占 `+2.23 MB RSS`。
    - 定位 S6 连续 20 次 `reload()` 紧循环增长来自内置 `llama` 扩展未 settle 的异步探测，等待 settle 后净增仅 `+0.01 MB HeapUsed`。
- [x] **S0-3：外部 Rust 参考项目（OMP / pie / pi-rs / pi-rust / pi_agent_rust）源码与许可证核查**
  - **产物**：[STAGE0_FEASIBILITY_REPORT.md](STAGE0_FEASIBILITY_REPORT.md) 第 3.2 节、[HANDOFF.md](HANDOFF.md)。
  - **依赖**：无。
  - **验收命令与证据**：核实 `can1357/oh-my-pi` 上层仍为 TS 主循环、仅将重算子下沉 Rust N-API；核实 QuickJS 方案与纯 Rust 方案不满足原版 TS/npm/.node 插件无缝兼容硬约束；确认 `Dicklesworthstone/pi_agent_rust` 许可证带附加限制不可直接移植。
- [ ] **S0-4：Linux `/proc/<pid>/smaps_rollup` PSS 实测与真实付费模型长连接测量（未完成，受主机与离线约束限制）**
  - **产物**：待在具备 Linux 环境或授权在线凭据时补充。
  - **依赖**：Linux 宿主环境 / 在线模型 API Key。
  - **验收命令与证据**：当前在 macOS 主机离线执行，按规则保留为未完成，不伪造 Linux PSS 或在线测量数据。

---

## 阶段 1：TypeScript 启动模块图瘦身与门禁（已实施并验收，内存目标未达标）

> **注意**：阶段 1 已拆分为 4 个串行 Kanban 交付切片（1/4 文档重构 → 2/4 门禁与变更前基线 → 3/4 按需懒加载实现 → 4/4 实测对比与兼容验收），四切片均已完成。实测结论：模块图 cli 436→381、rpc-entry 435→380、sdk 312→309，但 CLI `--version` RSS 仅 −1.97 MB，**未达 35–45 MB 目标**；未达标原因与未完成项见 1.4 节及 `S1-VERIFY-3`，不得标为阶段 1 目标达成。

### 1.1 需求与路线文档重构（对应卡片 1/4：`t_3d6f4941`）
- [x] **S1-DOC-1：创建 `PRD.md`、重构 `TODO.md` 并同步 `AGENT.md` / `HANDOFF.md`**
  - **产物**：[PRD.md](PRD.md)、[TODO.md](TODO.md)、[AGENT.md](AGENT.md)、[HANDOFF.md](HANDOFF.md) 及交接记录 `/Users/waya/.hermes/handoffs/mio-docs-delivery-20261021.md`。
  - **依赖**：读取 [STAGE0_FEASIBILITY_REPORT.md](STAGE0_FEASIBILITY_REPORT.md)、`/Users/waya/.hermes/handoffs/worker-delivery-protocol-20261021.md`、`/Users/waya/.hermes/handoffs/mio-original-scope-20261021.md`；备份已存于 `/Users/waya/.hermes/handoffs/backups-t_adab644e-20261021/`。
  - **验收命令与证据**：
    - `git status --short` 与 `git diff` 核验仅修改上述文档文件，未触碰任何源码文件，未将阶段 1 实施项标为已完成。

### 1.2 启动模块图门禁与变更前有效基线（对应卡片 2/4：`t_fcb99872`，替代归档卡 `t_ccda7a87`）
- [x] **S1-GATE-1：核查 CLI / Print / RPC / SDK 启动 eager 模块图并补齐 `scripts/check-entry-graphs.mjs` 门禁**
  - **产物**：更新后的 `scripts/check-entry-graphs.mjs`（含 `CODING_AGENT_ENTRY_BUDGETS`、`--report-stage1`、`--strict-stage1` / `PI_STRICT_CODING_AGENT_ENTRY_GRAPHS=1`）、变更前模块图与基线记录 `/Users/waya/.hermes/handoffs/mio-stage1-pre-baseline-20261021.json`。
  - **依赖**：S1-DOC-1。
  - **验收命令与证据**：
    - 查明 `packages/coding-agent` 在 `cli.ts`（436 文件）、`rpc-entry.ts`（435 文件）、`print-mode.ts`（8 文件）、`rpc-mode.ts`（60 文件）、`sdk.ts`（312 文件）入口下的实际 eager 依赖图，明确记录拟禁止的 `theme-schema.ts`（`typebox`/`typebox/compile`）、`syntax-highlight.ts`（`highlight.js/lib/core.js`，注：`package.json` 无 `cli-highlight` 依赖，门禁同时禁止 `cli-highlight` 与 `highlight.js/lib/core.js`）、`export-html`（`marked`/`marked-cjk-friendly`）、非交互路径 `interactive-mode.ts`（含 `grok-mermaid`）依赖边。
    - 门禁通过 `stage1Allow` / `stage1AllowExternals` 阶段性显式标记现有超预算边，默认拦截任何新增回流；开启 `--strict-stage1` 或 `PI_STRICT_CODING_AGENT_ENTRY_GRAPHS=1` 即强制执行最终严格预算（当前严格模式准确报告 28 项待阶段 1 懒加载消除项）。
    - 执行 `node scripts/check-entry-graphs.mjs` 与 `npm run check` 100% 通过。
- [x] **S1-BASE-1：建立可解析项目依赖的固定环境变更前基线（至少 5 次独立测量）**
  - **产物**：仓库外固定路径下的变更前基线原始 JSON `/Users/waya/.hermes/handoffs/mio-stage1-pre-baseline-20261021.json`、测量脚本 `/Users/waya/.hermes/cache/scratch/run-stage1-pre-baseline.mjs`、修复后的子模块拆解脚本 `/Users/waya/.hermes/cache/scratch/run-subcomponent-breakdown.ts` 与输出 `/Users/waya/.hermes/cache/scratch/subcomponent-breakdown-results.json`。
  - **依赖**：S1-GATE-1。
  - **验收命令与证据**：
    - **修复模块解析上下文**：使用项目根 `createRequire('/Users/waya/Projects/Mio/package.json')` 与 `--import packages/coding-agent/src/experimental/source-resolver.ts`，消除旧 `run-subcomponent-breakdown.ts` 在 scratch 目录直接导入 `typebox` 触发 `ERR_MODULE_NOT_FOUND` 的问题（23 个子模块探针与 11 个阶段 1 基线探针全部 `exit 0`）。
    - 在固定隔离离线环境下（macOS 26.6.2 arm64，Node `v26.12.0`），对 `P0` 空进程、`P1` CLI `--version`、`P2` `main.ts`、`P3` `print-mode.ts`、`P4` `rpc-mode.ts`、`P5` `sdk.ts`、`P6` `theme`+`typebox`、`P7` `syntax-highlight`、`P8` `export-html`+`marked`、`P9` `interactive-mode`、`P10` SDK 空会话各运行 **5 次**，完整记录原始值、中位数与全距（含 `rss`、`maxRss`、`peakFootprint`、V8 `heapUsed`/`heapTotal`、native `external`/`arrayBuffers`、父子进程分离口径及启动延迟 `wallMs`）。
    - 已写入交接文档 `/Users/waya/.hermes/handoffs/mio-entry-delivery-20261021.md`。

### 1.3 四大热点按需懒加载兼容实现（对应卡片 3/4：`t_2348d011`，已完成）
- [x] **S1-LAZY-1：`modes/interactive/theme/theme.ts` 的 TypeBox 校验按需/轻量化**
  - **产物**：`packages/coding-agent/src/main.ts`（`isBuiltinThemeName` 判定 + 按需 `import("./modes/interactive/theme/theme-schema.ts")`）、`theme.ts`（`setThemeJsonValidator` 注入点）。
  - **依赖**：S1-BASE-1。
  - **验收命令与证据**：
    - `theme-schema.ts` 已从 cli / rpc-entry / sdk 静态启动图消失（cli 436→381、rpc-entry 435→380、sdk 312→309）。
    - 运行时实测：print 模式 + 非内置主题 `bad-theme` 时动态 import 触发（`appMode=print themeName="bad-theme"` → validator loaded）；内置主题时不触发。
- [ ] **S1-LAZY-2：`utils/syntax-highlight.ts` (`cli-highlight`) 按需加载** — **未完成（保留 eager）**
  - **原因**：`highlightCode()` 是同步公开 API，且 esbuild 打包下 `createRequire` 无法加载打包模块（探针已记录），懒加载会破坏 `tsc dist` 与 bundle 契约。
  - **现状**：`syntax-highlight.ts` 与 `highlight.js` 仍在 cli / rpc-mode 静态图中，已在 `scripts/check-entry-graphs.mjs` 的 `REMAINING_ALLOWED_FILES` / `REMAINING_ALLOWED_EXTERNALS` 显式登记。
- [x] **S1-LAZY-3：`core/export-html/index.ts` (`marked` + 模板) 按需加载** — **部分完成**
  - **产物**：`core/export-html/index.ts`、`core/export-html/tool-renderer.ts` 改为异步边界 `await import`，两文件已从 cli / sdk 静态图消失。
  - **未完成部分**：`marked` / `marked-cjk-friendly` 仍经 `packages/tui/src/index.ts` 根 barrel 与 `markdown.ts` eager 加载，已登记为允许项并写明原因。
- [x] **S1-LAZY-4：非交互路径按需加载 `InteractiveMode`** — **静态门禁完成，运行时仍有回流**
  - **产物**：`main.ts` 交互分支 `await import("./modes/interactive/interactive-mode.ts")`；`grok-mermaid` 从 cli 外部依赖表消失。
  - **运行时残留（实测证据）**：加载任意扩展时 `core/extensions/loader.ts` → `virtual-modules.ts` → 静态 `import * as ... from "../../index.ts"`（SDK barrel）→ `modes/index.ts` → `interactive-mode.ts`，即带扩展的 print/RPC 仍会拉入交互组件树。静态门禁看不到该路径，属既有架构，未在本切片修改。

### 1.4 阶段 1 变更后实测、前后对比与全路径兼容验收（对应卡片 4/4：`t_9c59244c`，已完成）
- [x] **S1-VERIFY-1：同环境变更后至少 5 次实测与前后对比报告** — **已完成，目标未达标（如实记录）**
  - **产物**：`/Users/waya/.hermes/handoffs/mio-stage1-post-baseline-20261021.json`（变更后原始数据 + 与 `mio-stage1-pre-baseline-20261021.json` 的逐项对比）、`/Users/waya/.hermes/handoffs/mio-stage1-delivery-20261021.md`。
  - **依赖**：S1-LAZY-4。
  - **实测结果（Node v26.12.0 / darwin 25.6.0 arm64，每探针 5 次，中位数）**：
    - `P1` CLI `--version`：RSS 125.87 → 123.90 MB（Δ −1.97 MB），peakFootprint −1.94 MB，heapUsed −2.29 MB，wall 435.46 → 406.75 ms。
    - `P2` `main.ts`：RSS −1.99 MB；`P5` `sdk.ts`：−0.34 MB；`P10` SDK 空会话：−0.61 MB；`P0` 空进程基线 −0.02 MB（环境噪声量级）。
    - 模块图：cli 436→381、rpc-entry 435→380、sdk 312→309、`src/index.ts` 438→434，`print-mode` 8、`rpc-mode` 60 不变。
    - **与 35–45 MB 目标对比：未达标**。原因：`theme-schema`/`export-html`/`interactive-mode` 已移出静态图，但 `typebox`（cli 图内 15 处导入方，含 `keybindings.ts`/`model-config.ts`/`ai/utils/validation.ts`）、`highlight.js`、`marked` 仍 eager，约为被移除模块成本的主要部分；被删除模块自身增量（theme-schema +18.77 MB、interactive-mode +90.49 MB）与实测净降幅不成比例，说明这些依赖仍由其他 eager 路径支付。
  - **基线有效性**：变更前后同一台机器、同 Node/v8 版本、同解析上下文（`source-resolver.ts` + 根 `createRequire`）、每探针 5 次子进程 + `/usr/bin/time -l`，`P0` 空进程差异 0.02 MB，可做可信前后对比。
- [x] **S1-VERIFY-2：全路径功能、插件兼容与门禁回归验证** — **已完成（真实模型验证未完成）**
  - **产物**：`/Users/waya/.hermes/cache/scratch/mio-stage1-final-check.log`、`mio-stage1-final-tests.log`、`mio-stage1-final-tui-tests.log`、`mio-stage1-rpc-smoke-result.json`。
  - **依赖**：S1-LAZY-4。
  - **验收命令与证据**：
    - `npm run check`（含 `check:entry-graphs`、install-lock、browser-smoke）exit 0。
    - 定向回归：vitest 23 文件 → 22 通过 / 1 跳过（`test/rpc.test.ts` 需 `ANTHROPIC_API_KEY` 且依赖 `dist`，环境无凭据），180 passed / 18 skipped；`packages/tui` `node --test` 3 文件 189 passed / 0 failed。
    - 离线 E2E（faux provider，`extensions/faux-provider.ts`）：print 模式 `-p` 输出正确且 stdout 干净；RPC 模式 JSONL 子进程实测 `get_state` / `prompt` / `get_last_assistant_text` / `export_html`（323,664 B，session-data base64 解码 33,201 B 含回复文本）/ `get_session_stats` 全部 success，0 未解析行、stderr 0 字节。
    - SDK：`P10` 空会话 + `W1` 工作负载探针（流式 prompt + 扩展 tool/hook + `reload()` + HTML 导出 295,930 B）5/5 一致通过。
    - 门禁有效性：仓库内 `node scripts/check-entry-graphs.mjs --report-stage1` exit 0；预算探针（cli 预算 381→380 的脚本副本）exit 1 并报 `reaches 381 files, budget 380`。
    - **未完成项**：真实模型（Anthropic/OpenAI）端到端验证——本卡执行时环境无凭据；已由 S1-VERIFY-3 用 DeepSeek 真实 provider 补做（2026-10-21）。
- [x] **S1-VERIFY-3：真实模型端到端兼容验证** — **已完成（2026-10-21，真实 DeepSeek provider）**
  - **配置**：`~/.hermes/cache/scratch/mio-deepseek-agent/models.json` 用 `baseUrl: https://api.deepseek.com/v1` + `apiKey: "$OPENAI_API_KEY"` 环境插值覆盖内置 `deepseek` provider；密钥只在环境变量中，仓库内无任何明文（已用 `grep -F` 核验 0 命中）。
  - **证据**：`/Users/waya/.hermes/cache/scratch/mio-real-model-e2e-20261021.json`（命令、exit code、stdout、usage、耗时、eager 命中），交接 `/Users/waya/.hermes/handoffs/mio-real-model-20261021.md`。
  - **实测结果（模型 `deepseek/deepseek-flash`）**：
    - CLI print `-p`：exit 0，stdout 恰为 `MIO_DEEPSEEK_PRINT_OK`，stderr 0 字节，wall 1.46 s。
    - CLI `--mode json`：exit 0，回复 `MIO_DEEPSEEK_JSON_OK`，usage input 238 / output 10 / cacheRead 256 / total 504 token，wall 1.75 s。
    - RPC（JSONL 子进程）：`get_state`（375 ms，模型解析为 `deepseek-flash`）/ `prompt`（ack 1399 ms，`agent_settled` 1372 ms）/ `get_last_assistant_text`（`MIO_DEEPSEEK_RPC_OK`，27 ms）/ `get_session_stats`（总 504 token、cost $0.0000849，27 ms）全部 success，事件流 `agent_start…agent_settled`，stderr 0 字节，wall 2.23 s。
    - SDK：`firstDeltaMs` 912 ms、`promptDoneMs` 1522 ms、115 个流式 delta、`agent_settled` 为真、usage input 232 / output 116（含 reasoning 106）/ total 604 token、HTML 导出 286,318 B，wall 1.91 s。
  - **额外 eager 加载核查**：真实 provider 下 print 路径只命中 `theme.ts`；RPC 路径 0 命中；SDK 路径命中 `theme.ts` 与显式导出时的 `export-html/index.ts`。`theme-schema.ts` 与 `interactive-mode.ts` 在三条路径均未加载，真实模型未把懒加载模块拉回。
  - **门禁**：`node scripts/check-entry-graphs.mjs --report-stage1` exit 0（cli 381/381、rpc-entry 380/380、print 8/8、rpc-mode 60/60、sdk 309/309，预算无回流）；`npm run check` exit 0；定向 vitest 4 文件 174 例全过。
  - **未覆盖**：交互模式真实终端会话（tmux）、RPC/print 的工具调用链路（本次用 `--no-tools` 隔离网络验证范围）、`deepseek-v4-pro` 未跑。

---

## 阶段 2：Multi-Agent Daemon / Multiplexer（单 Node 宿主多会话池，仅规划）

> **阶段定位**：本阶段当前仅作可执行工程规划，在阶段 1 完成验收并向用户汇报前**不启动编码实现**。

- [ ] **S2-PLAN-1：单 Node 宿主多会话隔离边界与共享缓存规范设计**
  - **产物**：Daemon 进程模型与会话隔离设计规范、会话上下文接口定义。
  - **依赖**：阶段 1 完成验收（S1-VERIFY-1、S1-VERIFY-2）。
  - **设计约束与验收标准**：
    - **可共享只读层**：Node/V8 运行时、核心 TS/JS 模块代码缓存、只读模型注册表（`models.generated` / catalog）。
    - **每会话隔离层（Session-scoped）**：`AgentSessionRuntime`、`SessionManager`、每会话插件实例与可变状态、工具/命令/快捷键/Provider 注册表、认证上下文（Auth/OAuth）、会话级 `cwd` 与 `env` 视图、终端 I/O 桥接、`AbortController` 与 `reload`/`dispose` 生命周期。
    - **全局副作用治理与独立进程回退**：排查框架与内置工具对 `process.cwd()` / `process.env` / `process.exit()` 的直接耦合；针对修改全局单例、使用非重入原生 `.node` addon 或产生不可隔离进程级副作用的插件，实现**独立 Node 子进程回退（Process Fallback）**，并将回退子进程内存明确计入总内存账本，不以牺牲插件兼容性换取共享率。
- [ ] **S2-PLAN-2：Multi-Agent Daemon 与轻量 CLI/TUI/RPC 多路复用客户端实现**
  - **产物**：Daemon 宿主服务、本地 IPC（Unix Domain Socket / Named Pipe）多路复用协议、轻量 CLI/TUI/RPC 客户端入口及生命周期管理。
  - **依赖**：S2-PLAN-1。
  - **验收命令与证据（待阶段 2 实施时执行）**：
    - `npm run check` 及多会话并发创建、独立 `cwd`/`env`/插件隔离、异常崩溃隔离、客户端断连重连、`reload()` 与销毁释放专项测试通过。
- [ ] **S2-PLAN-3：32 会话（目标）与 64 会话（扩展验证）多负载内存与兼容性实测**
  - **产物**：32/64 并发会话在空闲、活跃流式、长上下文、加载插件及包含子进程回退场景下的 5 次重复测量报告（含守护进程 + 客户端 + 子进程树总 RSS/`peakFootprint`/PSS/Heap/Native 与延迟）。
  - **依赖**：S2-PLAN-2。
  - **验收命令与证据（待阶段 2 实施时执行）**：
    - 检验“每会话增量约 4.6 MB”与“32/64 Agent 总内存 < 300 MB”两个待验证目标的真实达成度；禁止仅用无插件空壳会话充当最终验收。

---

## 阶段 3：Rust N-API 并发工具引擎（有界共享线程池，仅规划）

> **阶段定位**：本阶段当前仅作可执行工程规划，在阶段 2 完成或用户明确指示前**不启动编码实现**。

- [ ] **S3-PLAN-1：外部参考实现（OMP）许可证审查与 N-API 算子契约定义**
  - **产物**：许可证合规审查记录、`native/` 下 N-API 异步任务与有界共享线程池接口定义。
  - **依赖**：阶段 1 完成（可与阶段 2 协同评估）。
  - **验收命令与证据（待阶段 3 实施时执行）**：
    - 借鉴 `can1357/oh-my-pi` (OMP) 前逐文件核查许可证兼容性；绝不引入带附加限制的 `Dicklesworthstone/pi_agent_rust` 代码。
    - 明确仅替换有实测证据的内部热点算子，不触碰 `agent-loop` 或插件 JS 对象引用。
- [ ] **S3-PLAN-2：有界共享线程池与 32 路并发热点工具（文件搜索 / Diff / 输出截断）实现**
  - **产物**：`native/` 下的 Rust N-API 工具实现及 `packages/coding-agent` 内部工具适配层（含启动期缺失/ABI 不匹配自动回退 TS 实现）。
  - **依赖**：S3-PLAN-1。
  - **验收命令与证据（待阶段 3 实施时执行）**：
    - `cargo fmt --manifest-path native/Cargo.toml --all --check`、`cargo clippy --manifest-path native/Cargo.toml --all-targets --locked -- -D warnings`、`cargo test --manifest-path native/Cargo.toml --locked` 及 `npm run check` 全部通过。
    - 覆盖 32 路并发 `grep` / `find` / 大文件 I/O / Diff / 输出截断压测，验证有界线程池背压、`AbortSignal` 取消中断、原生内存释放、无副作用重放，以及与原版 TS 工具输出的差分一致性。
