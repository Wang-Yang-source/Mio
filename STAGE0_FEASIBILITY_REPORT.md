# Mio 阶段 0：原版 Pi 内存基线、热点拆解与无缝兼容可行性评估报告

- **评估日期**：2026-10-21
- **工作目录与分支**：`/Users/waya/Projects/Mio`（分支 `codex/rust-kernel-handoff`，基线提交 `1c62e8c670fecbab04e2119a5f8c16af74a2bbd4`）
- **实测硬件与工具链**：
  - 平台：macOS 26.3 (`arm64`, Apple M4)
  - Node.js：`v25.9.0` / npm：`11.12.1`
  - Rust：`rustc 1.94.1` / `cargo 1.94.1`
  - 模型目录快照：`sha256-ce2b35e558368354315ceab5a0bc1d50e53d405e8f261ea019a491591bb0a758`（通过 `packages/ai/scripts/hydrate-model-catalog.ts` 水合）
- **门禁回归状态**：
  - `npm run check`（Biome + 四包 `tsc --noEmit` + 入口模块图门禁 `check-entry-graphs.mjs`）：**100% 通过**
  - `cargo fmt --check` / `cargo check` / `cargo clippy -- -D warnings` / `cargo test`（`native/Cargo.toml`）：**100% 通过**
  - `packages/agent` 单元回归（`agent.test.ts` + `agent-loop.test.ts`）：**31/31 通过**
  - `packages/coding-agent` 扩展与会话回归（8 个测试文件）：**71/71 通过**

---

## 一、 核心结论先行：Mio 还有没有必要做全量 Rust 内核重写？

**结论：在「必须保留真实 Node 宿主、100% 原样无缝兼容原版 Pi TS/JS/npm 插件」的硬约束下，不建议进行「全量 Rust 内核重写（将 Agent 循环、会话树、上下文与协议层全部迁往 Rust）」，建议缩小范围为「纯 TypeScript 启动模块图瘦身 + 按需可选的独立 Rust 重算子（如全文检索/图像/AST）」。**

核心证据如下：

1. **原版 Pi 的内存大头在「Node/V8 基础开销 + 顶层静态导入的 TUI/Theme/Schema 模块图」，不在 Agent 内核与会话状态**：
   - 空 `node -e ""` 进程基线即占 **39.42 MB RSS**（4.38 MB HeapUsed）；挂载 `--experimental-strip-types` 与 `source-resolver.ts` 后为 **48.55 MB RSS**。
   - 导入 `packages/coding-agent/src/main.ts` 后跃升至 **110.22 MB RSS**（`peakFootprint` ~127 MB），其中仅 `modes/interactive/theme/theme.ts` 单模块（顶层静态导入 `@sinclair/typebox/value`、`chalk` 并编译校验 52 个 JSON 主题 Schema）就拉高了 **+37.75 MB RSS**，`interactive-mode.ts`（拉入 `cli-highlight`/`highlight.js` 全量语言包与 `marked`）拉高了 **+50.30 MB RSS**。
   - 相反，`@mariozechner/pi-agent-core`（含 `agent-loop.ts`）整个包仅占 **+2.23 MB RSS**（`+0.40 MB HeapUsed`）。
2. **多轮长会话与大工具输出在现有 TypeScript 内核中内存增长极小，且无内存泄漏**：
   - 从导入 `main.ts`（110.22 MB RSS）到创建完整 `AgentSessionRuntime` 空会话（115.69 MB RSS），仅增加 **+5.47 MB RSS**。
   - 50 轮对话（含 50 次 4 KB 工具输出，共 151 条消息）相比 1 轮对话，V8 `HeapUsed` 仅从 **21.84 MB** 增至 **25.00 MB**（**+3.16 MB**）。
   - 200 轮长会话持久化 + 分叉（Scenario S5，预热后 5 次中位数）：GC 后 `HeapUsed` 仅 **45.87 MB**（对比空会话 37.20 MB 仅增 **+8.67 MB**），`peakFootprint` 中位数 **194.65 MB**。
   - 20 次扩展热重载（Scenario S6）：等待内置 `llama` 扩展后台探测异步任务 settle 后，20 次 `reload()` 循环的 `deltaHeap` 仅为 **+0.01 MB**（磁盘 `.ts` 扩展经 `jiti` 热重载 20 次也仅增长 **+1.0 MB Heap / +1.0 MB RSS**，约 50 KB/次）。
3. **上游 Pi 代码库已经实施了严密的懒加载治理**：
   - `scripts/check-entry-graphs.mjs` 已在 CI 门禁层禁止入口静态导入 `models.generated`（3.3 MB）、`@anthropic-ai/sdk`、`openai`、`@google/genai`、`@aws-sdk/*`、`three`、`@silvia-odwyer/photon-node` 等重依赖。
   - 96 个 Provider 模型定义 JSON（共 13 MB）已改为按需读取。
4. **若强行把 Agent 循环与消息历史迁入同进程 Rust（Node-API），在无缝兼容约束下反而会增加内存**：
   - 原版 Pi 插件契约（`ExtensionAPI`）允许插件在 `context`、`tool_call`、`tool_result`、`before_provider_request` 等 20+ 个钩子中直接同步读取并原地修改 JS 消息对象引用，且通过 `ctx.sessionManager.getEntries()` / `getBranch()` 同步获取会话树，通过 `ctx.ui.custom()` 直接挂载 `@mariozechner/pi-tui` 实例。
   - 只要保留 Node 宿主以支持上述语义，Node 侧就必须持有完整的 JS 消息对象与模块图（~110 MB 基线一分不少）；此时若 Rust 内核再持有一份状态并通过 N-API 跨边界同步，不仅省不下 Node 的模块图内存，还会因跨语言双份持有与序列化拷贝导致总 RSS 上升。

---

## 二、 隔离离线环境实测数据汇总

所有测试均在隔离临时目录（`HOME`、`XDG_CONFIG_HOME`、`PI_CODING_AGENT_DIR` 指向临时空目录，禁用联网与真实付费模型，使用离线 `faux` Provider）下执行。原始记录保存在 `/Users/waya/.hermes/cache/scratch/scenario-benchmark-results.json`。

### 2.1 七大标准场景基准（预热后 5 次独立子进程运行中位数与范围，macOS arm64）

| 场景编号与说明 | Wall 耗时中位数 (范围) | Max RSS 中位数 | macOS `peakFootprint` 中位数 | GC 后 RSS 中位数 | GC 后 V8 `HeapUsed` 中位数 |
| --- | --- | --- | --- | --- | --- |
| **S1**：CLI `--version` 冷启动 | 149.9 ms (148.2–153.1) | 103.33 MB | **62.25 MB** | 62.21 MB | **15.08 MB** |
| **S2**：创建空 `AgentSessionRuntime` | 420.6 ms (403.6–423.9) | 165.25 MB | **127.76 MB** | 127.20 MB | **37.20 MB** |
| **S3**：20 轮流式响应（faux streaming） | 442.1 ms (437.4–450.7) | 199.45 MB | **164.94 MB** | 164.42 MB | **40.08 MB** |
| **S4**：大体积工具输出（10 轮 × 64 KB） | 445.3 ms (438.9–453.2) | 193.94 MB | **158.19 MB** | 157.16 MB | **39.51 MB** |
| **S5**：200 轮长会话持久化 + 分叉 (`fork`) | 507.4 ms (500.0–517.8) | 226.63 MB | **194.65 MB** | 194.60 MB | **45.87 MB** |
| **S6**：扩展 `reload()` 连续 20 次循环 | 495.4 ms (488.0–507.2) | 206.39 MB | **172.44 MB** | 172.19 MB | **49.45 MB** *(注)* |
| **S7**：同进程 10 个并发会话实例 | 488.5 ms (482.1–496.3) | 203.89 MB | **169.86 MB** | 169.86 MB | **41.65 MB** |
| **RPC E2E**：独立子进程 `--mode rpc` 就绪 | 390.4 ms (389.9–394.6) | 174.63 MB | **134.58 MB** | — | — |

> **注（S6 Reload 拆解）**：`S6` 中紧循环连续触发 20 次 `reload()` 时 `HeapUsed` 为 49.45 MB，经 `run-reload-settle.ts` 与 `run-reload-which-builtin.ts` 定位，来自内置 `llama` 扩展后台异步探测未完成时的瞬态堆积；当每次 `reload()` 等待微任务/计时器 settle 后，20 次重载的 `HeapUsed` 从 `38.96 MB` 到 `38.97 MB`（**净增仅 +0.01 MB**）。

### 2.2 模块级分层增量拆解（`run-module-breakdown.ts`，4 次中位数）

| 阶段 | RSS (MB) | ΔRSS (MB) | V8 HeapUsed (MB) | V8 HeapTotal (MB) | 说明 |
| --- | --- | --- | --- | --- | --- |
| `01-empty-node` | 39.42 | — | 4.38 | 5.78 | Node v25.9.0 空进程基线 |
| `02-with-source-resolver` | 48.55 | +9.13 | 5.14 | 6.95 | 挂载 TS 源码即时解析器 |
| `03-import-ai` (`@mariozechner/pi-ai`) | 57.12 | +8.57 | 6.32 | 11.41 | AI 核心协议与懒加载注册表 |
| `04-import-agent` (`@mariozechner/pi-agent-core`) | 59.35 | **+2.23** | 6.72 | 11.73 | **Agent 状态机与循环仅占 2.23 MB** |
| `05-import-tui` (`@mariozechner/pi-tui`) | 57.76 | — | 6.46 | 9.61 | 终端 UI 基础库 |
| `06-import-coding-agent-sdk` (`core/sdk.ts`) | 99.62 | **+40.27** | 15.86 | 36.11 | 拉入 Session/Tools/Extensions/Theme |
| `07-import-coding-agent-main` (`main.ts`) | 110.22 | +10.60 | 20.11 | 41.27 | 拉入 InteractiveMode 与 CLI 入口 |
| `08-empty-session-in-memory` | 115.69 | +5.47 | 20.99 | 44.36 | 实例化 `AgentSessionRuntime` |
| `09-single-tool-turn-bash` | 117.64 | +1.95 | 21.84 | 44.86 | 执行 1 次真实 bash 工具调用 |
| `10-multi-turn-10-turns-4kb` | 119.30 | +1.66 | 22.71 | 45.11 | 10 轮对话 + 40 KB 工具输出 |
| `11-multi-turn-50-turns-4kb` | 126.72 | +7.42 | 25.00 | 48.36 | 50 轮对话 + 200 KB 工具输出 |

### 2.3 启动路径热点子模块与第三方依赖排行（基于 48.55 MB 基线单独导入）

1. **`modes/interactive/theme/theme.ts`**：单模块导入即达 **86.25 MB RSS（ΔRSS +37.75 MB，ΔHeapUsed +6.64 MB）** —— 根因是顶层静态导入 `@sinclair/typebox/value`（`Value.Check`）并在加载主题时对大型 `ThemeSchema` 做同步校验；即使在 Headless / Print / RPC 模式下也会被 `sdk.ts` 链路间接触发。
2. **`utils/syntax-highlight.ts` (`cli-highlight` / `highlight.js`)**：单模块 **ΔRSS +14.95 MB（ΔHeapUsed +3.24 MB）** —— 启动即静态加载全量语法高亮语言表。
3. **`core/export-html/index.ts` (`marked` + `theme.ts`)**：单模块 **ΔRSS +30.35 MB** —— 仅在用户执行 `/export` 导出 HTML 时才需要，却在核心启动图中被静态引用。
4. **`core/extensions/loader.ts` (`@mariozechner/jiti`)**：单模块 **ΔRSS +24.75 MB**（其中 `jiti` 自身约 +4.91 MB，其余为虚拟模块注入依赖）。

---

## 三、 当前 Rust 原型与外部参考项目核查

### 3.1 当前仓库 `native/` 原型审计
- `native/kernel/src/lib.rs`（195 行）与 `native/node/src/lib.rs`（45 行）目前仅实现了回合阶段枚举（`Idle` / `RunningPrompt` / `ExecutingTools` / `Stopping` / `Cancelled`）和工具批次规划（`plan_tool_batch`），并通过 JSON 字符串在 N-API 边界传递。
- `packages/agent/src/loop-driver.ts` 仅定义了 `LoopDriver` 接口，`agent-loop.ts` **完全未消费** `loopDriverFactory`。
- **关键缺陷**：该原型触及的 `agent-loop` 调度状态在原版中仅占 `< 1 MB` 内存，且若后续通过 JSON 往返传递 `AgentMessage[]`，会破坏原版插件在 `context` 钩子中对消息对象的原地修改语义与对象身份（Object Identity）。

### 3.2 外部 Rust 参考项目核实对比

| 参考项目 | 真实架构与插件机制（源码核实） | 是否满足「无缝加载原版 Pi TS/npm 插件」 | 可借鉴点 |
| --- | --- | --- | --- |
| **`can1357/oh-my-pi` (OMP)** | 上层仍为 **TypeScript/Bun**，`agent-loop.ts` 主循环仍是 TS；Rust 仅封装在 `crates/pi-natives`（通过 N-API 提供 `grep`/`glob`/`ps`/`html`/`highlight`/`image`/`clipboard` 等计算密集型工具） | 部分兼容（宿主改为 Bun，且移除了部分 Node 专属路径；其内存与原版 Pi 的差异来自模块裁剪与工具下沉，而非 Rust Agent 内核） | **最值得借鉴**：保持 TS 主循环与插件宿主不动，仅用 Rust N-API 替换高频 I/O 与大内存工具（如语法高亮、全文搜索、AST/Diff） |
| **`c4pt0r/pie`** | 纯 Rust 重写（`crates/ai`, `agent`, `coding-agent`, `tui`）；`docs/ds4.md` 中的 DS4 指本地模型推理服务的 KV Cache 前缀命中优化 | **不兼容**（无 Node 宿主，无法加载原版 Pi `.ts`/npm 插件与 `@mariozechner/pi-tui` 组件） | 请求历史稳定性设计（避免系统提示词动态时间戳破坏服务端前缀缓存） |
| **`jshachm/pi-rs`** | 纯 Rust CLI/TUI；`src/extensions/loader.rs` 仅读取 `extension.json` 元数据并维护 `enabled` 布尔值 | **不兼容**（源码中无任何执行原版 TS/JS/npm 插件的运行时逻辑；README 宣称的 `<15MB` 基于无 JS 宿主的纯 Rust 空进程，不能与带 Node 插件宿主的系统直接横向排名） | 无直接复用价值 |
| **`yannelli/pi-rust`** | Rust + QuickJS + 部分 Node API shim (`crates/pi-js/src/unsupported.rs`) | **不兼容**（QuickJS 无法加载原生 npm `.node` addon，且缺失 `async_hooks`、完整 `fs`/`crypto`/`stream` 语义） | 证明了 QuickJS 路线无法满足本项目的无缝兼容硬约束 |
| **`Dicklesworthstone/pi_agent_rust`** | 独立 Rust Agent；LICENSE 含额外附加条款（rider） | **不兼容**（产品方向已偏离原版 Pi 替代品，且许可证有污染风险） | **禁止直接移植代码** |
| **`bigfish1913/pi-rust`** | Rust SDK + Rust `cdylib` C ABI 插件 | **不兼容**（完全不支持 TS/npm 扩展） | 内部工具实现细节 |

---

## 四、 三条候选路线对比与决策建议

| 维度 | 路线 A：纯 TypeScript 启动图瘦身与懒加载治理（推荐首选） | 路线 B：OMP 式混合架构（路线 A + 热点工具下沉 Rust N-API） | 路线 C：全量 Rust 内核迁移（Node 仅作插件与 TUI 外壳） |
| --- | --- | --- | --- |
| **核心做法** | 1. 延迟加载 `export-html` (`marked`)、`syntax-highlight` (`cli-highlight`)、非交互模式下的 `InteractiveMode`/`theme.ts`；<br>2. 用轻量校验替代 `theme.ts` 顶层 `@sinclair/typebox/value` 编译；<br>3. 修复 `llama` 扩展 reload 未取消的后台探测 | 在路线 A 基础上，参考 `oh-my-pi` 将 `cli-highlight`（+15MB）、大文件 `grep`/`find`/diff、长会话全文检索下沉到同进程 `native/node` (Rust N-API) | 将 `agent-loop`、`SessionManager`、上下文管理与全部内置 Provider 迁入 Rust，通过 N-API 句柄与回调桥接 Node 插件 |
| **预期内存收益** | 冷启动与 RPC/SDK 空会话直接降低 **25–45 MB RSS**（从 ~127 MB 降至 **~85–95 MB**） | 在路线 A 基础上再降 **10–15 MB RSS**，且显著降低大仓库搜索/高亮时的峰值堆分配与 GC 抖动 | **几乎无额外内存收益（甚至可能因 JS/Rust 双份消息镜像而倒挂）**，因为 Node + V8 + TS 插件宿主本身就占 ~80–90 MB 底数 |
| **原版插件兼容性** | **100% 零风险原样兼容**（所有对象引用、TUI、同步 getter 毫无变化） | **100% 原样兼容**（公开 SDK 与插件钩子完全不变，仅替换内部工具实现） | **高风险**（20+ 个同步/异步钩子、对象身份相等性 `===`、`SessionManager` 同步树查询极易在跨语言边界出现细微语义退化） |
| **工程周期与维护成本** | **极低**（1–2 个阶段即可完成并验收） | **中等**（按工具逐个替换，边界清晰，随时可回退） | **极高**（需重写数万行协议/会话/循环代码并维护复杂的 N-API 双向句柄表） |

---

## 五、 未完成项与平台限制说明

1. **Linux PSS 实测标记为未完成**：当前执行环境为 `wayamac`（macOS 26.3 arm64），已使用 macOS 原生 `/usr/bin/footprint`（记录 `peakFootprint` dirty+compressed memory）与 `resourceUsage().maxRSS`、V8 Heap 指标完成每场景预热后 5 次测量；Linux 专属 `/proc/<pid>/smaps_rollup` PSS 指标需在 Linux 宿主上另行采集。
2. **真实付费模型长连接测量未执行**：按任务约束，阶段 0 全部使用离线隔离环境与 `faux` 流式 Provider 完成确定性复现，未调用外部付费 API。

---

## 六、 等待用户决策的下一步选项

请老大根据上述实测证据选择后续推进方向：
1. **选项 1（推荐：走路线 A + B）**：停止「将 `agent-loop` / 状态机迁往 Rust」的无收益重写；改为在当前分支先做 **TypeScript 启动图瘦身**（把 `theme.ts` TypeBox 校验、`cli-highlight`、`export-html` 等总计 ~40 MB RSS 的静态加载改为按需懒加载），随后参考 `oh-my-pi` 仅将高内存/高 CPU 工具（语法高亮、文件搜索/diff）下沉到 Rust N-API。
2. **选项 2（走路线 A 后封板）**：仅执行纯 TypeScript 内存瘦身与 `reload` 异步清理，拿到实测 25–45 MB RSS 降幅并保持 100% 原生兼容后即收官。
3. **选项 3（坚持原计划走路线 C）**：即便内存收益有限，仍按 `TODO.md` 阶段 1–5 继续推进完整 Rust 内核迁移。
