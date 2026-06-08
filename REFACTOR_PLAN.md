# 重构计划

## 目标

每个文件、每个函数只做一件事。每个 commit 只做一件事。

## 执行进度

- [x] Phase 1.1 session_ops → tree.rs + preview.rs + delete_ops.rs
- [x] Phase 1.2 ui_popup → ui_popup_select + ui_popup_view + ui_popup_confirm
- [x] Phase 1.3 extraction → session_title.rs
- [ ] Phase 2.1 handler.rs 大方法拆分
- [ ] Phase 2.2 App 字段重组 (browse/search/io)
- [ ] Phase 2.3 discovery → discovery_walk + discovery_parse
- [ ] Phase 2.4 Agent 抽象层 — 统一 agent trait
- [ ] Phase 3.1 handler 方法签名优化

## Phase 1：低风险拆分 ✅ 已完成

session_ops.rs 2022→1337, ui_popup.rs 2393→175, extraction.rs→session_title.rs

## Phase 2：中风险重构（部分签名变更）

### 2.1 handler.rs 拆分大方法

当前 `handle_key` 是 230 行的 match。拆成：

- `handle_none_mode_key` — InputMode::None 下的按键
- `handle_input_mode_key` — 各种 InputMode 输入
- `handle_scrollback_key` — scrollback 搜索

每个方法控制在 50 行以内。

### 2.2 App struct 字段重组

把扁平字段组织成子结构体：

```rust
struct App {
    view: AppView,         // 已有
    ptys: PtyManager,      // 已有
    sessions: SessionStore,// 已有
    popup: PopupState,     // 已有
    chains: ChainState,    // 已有
    // 新分组：
    browse: BrowseState,   // browse_dir + browse_entries + browse_state
    search: SearchState,   // search_index + search_results + search_result_state
    io: IoState,           // last_refresh + timeline_events + agent_recommendations
}
```

### 2.3 discovery.rs 职责分离

| 新文件 | 内容 |
|--------|------|
| `discovery.rs` (保留) | `discover_sessions`, `discover_sessions_by_ids`, `discover_sessions_cached` — 公共 API |
| `discovery_walk.rs` | `collect_*`, `walk_*`, `find_session_jsonl*` — 文件遍历 |
| `discovery_parse.rs` | `parse_session_from_path` + 从 extraction 移来的解析函数 |

### 2.4 Agent 抽象层

**问题：** Agent 相关代码散落在 6+ 个文件，每个新 agent 需要改 switch-case：

| 位置 | 当前做法 |
|------|---------|
| `types/agent.rs` | 硬编码枚举 + `build_*_cmd()` + `sessions_dir()` |
| `session.rs` | spawn 时按 agent 建命令 |
| `discovery.rs` | 按 agent 目录扫描 JSONL |
| `extraction.rs` | 按 agent 解析 JSONL |
| `ui.rs` | `Agent::Claude => color_claude` 重复 6 次 |
| `handler_select.rs` | agent 选择弹窗 |

**方案：在 `types/agent.rs` 中统一封装 agent 特性：**

```rust
impl Agent {
    /// 该 agent 的 session 文件扩展名/目录模式
    fn session_pattern(&self) -> &str;
    
    /// 该 agent 的 JSONL 解析函数
    fn parse_session(&self, jsonl: &Path) -> Option<Session>;
    
    /// 该 agent 的侧边栏图标颜色
    fn theme_color(&self, theme: &Theme) -> Color;
    
    /// 检查该 agent 是否已安装
    fn is_installed(&self) -> bool;
}
```

**具体改动：**

1. `ui.rs` 中 6 处 `Agent::* => theme.agent_*` 替换为 `agent.theme_color(&self.view.theme)`
2. `discovery.rs` 中 `collect_*_jsonl` 三个函数合并为一个，按 `agent.session_pattern()` 遍历
3. `extraction.rs` 中 `parse_gsd_session`/`parse_codex_session` 通过 agent 分发
4. `handler_select.rs` 中 agent 列表从 `detect_agents()` 动态获取，而非硬编码

**收益：** 添加新 agent 只需改 `types/agent.rs` 一处 + 注册枚举变体。

**风险：** 中。需要改 discovery 和 extraction 的接口，但这些路径有测试覆盖。

## Phase 3：高风险重构（核心循环变更）

### 3.1 handler 方法接收具体参数

把 `fn handle_key(&mut self, key)` 改为接收需要的具体状态，而非整个 `&mut self`。

**需要：**
- 全面的集成测试覆盖
- 手动回归测试清单
- 每个 refactor 后跑全量测试

## 执行原则

1. **一个 commit 只做一件事** — 一个文件拆分 = 一个 commit
2. **先测试后重构** — 每个 phase 开始前确认 356 tests 全绿
3. **可随时停下** — 每个 commit 后代码都可编译、可运行、可发布
4. **不改行为** — 纯结构重组，不修 bug、不加功能

## 执行顺序

```
Phase 2.1  handler.rs 大方法拆分
Phase 2.2  App 字段重组 (browse/search/io)
Phase 2.3  discovery → discovery_walk + discovery_parse
Phase 2.4  Agent 抽象层
Phase 3.1  handler 方法签名优化
```

每一步：改代码 → cargo build → cargo test → commit。
